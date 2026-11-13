//! Running a setup: the plan's steps on a worker thread, with progress and a way to stop.
//!
//! [`Handle::start`] plans (so what is already on disk is skipped), then downloads two files at a
//! time while the engine is unpacked as soon as its archive is there, writes the engine's model
//! paths, works out the settings and, if asked, starts the engine and renders a test image.
//! Stopping keeps everything that arrived: starting again continues from it.
//!
//! The worker never touches the app's settings. It returns an [`Outcome`], and the engine
//! applies it to the settings as they are then ([`crate::configure`]).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use photocraft_aigen::{Control, Launch, LocalServer, Settings, Transport};
use serde::Serialize;

use crate::download::{self, Spec};
use crate::fetch::Fetch;
use crate::manifest::Manifest;
use crate::plan::{self, Choices, Layout, Plan, Planned, PlannedModel, RealDisk, Route, Step};
use crate::runtime::{self, Marker, RuntimePlatform};
use crate::state::{self, Phase};
use crate::unpack::{self, Limits};
use crate::vcrt;
use crate::{Error, Result, configure, human_bytes};

/// What the test image shows. Plain, quick to judge, and nothing a model would decline.
pub const TEST_PROMPT: &str = "A red apple on a wooden table by a window, soft morning light, photograph";
/// The test image's size: one megapixel, which every model here renders well.
const TEST_SIZE: u32 = 1024;
/// A fixed seed, so the same model gives the same test image on every machine.
const TEST_SEED: u64 = 2_026_101_300;

/// What a setup needs from the app.
pub struct Env {
    pub fetch: Arc<dyn Fetch>,
    pub transport: Arc<dyn Transport>,
    /// Starts the engine for the test image. `None`: it must already be running.
    pub server: Option<Arc<dyn LocalServer>>,
    /// Where the setup's record is kept ([`crate::state`]).
    pub state_file: Option<PathBuf>,
    pub sleep: fn(Duration),
    pub free_space: fn(&Path) -> Option<u64>,
    /// Windows: where the Microsoft C++ runtime is put beside the engine's Python from (see
    /// [`crate::vcrt`]). `None`: left as the engine archive has it.
    pub vc_runtime: Option<vcrt::Sources>,
}

/// One setup to run.
pub struct Job {
    pub manifest: Manifest,
    pub choices: Choices,
    /// The settings as they are now: what the test image's settings start from.
    pub settings: Settings,
    /// The Hugging Face token, for gated files.
    pub token: Option<String>,
    /// Managed: the port the engine is given (0: the first free one from 8190).
    pub port: u16,
    /// Existing: the server's address, and how it is started when that is known.
    pub url: String,
    pub launch: Option<Launch>,
}

/// The test image, as the engine returned it (PNG).
#[derive(Clone, Debug, PartialEq)]
pub struct TestImage {
    pub bytes: Vec<u8>,
    pub seconds: f64,
    pub model: String,
    pub model_name: String,
    pub width: u32,
    pub height: u32,
}

/// What a finished setup hands the app.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub route: Route,
    /// Where the engine answers, and how it is started.
    pub url: String,
    pub launch: Option<Launch>,
    pub port: u16,
    /// The model files the engine has, by folder.
    pub folders: BTreeMap<String, Vec<String>>,
    /// The manifest models installed whole.
    pub installed: Vec<String>,
    /// The chosen models that were left out, and why.
    pub skipped: Vec<PlannedModel>,
    pub test: Option<TestImage>,
    /// Why there is no test image, when one was asked for. Not a failed install.
    pub test_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StepState {
    Pending,
    Active,
    Done,
    Failed,
    Skipped,
}

pub use crate::plan::LINKED_NOTE;

/// One row of the Install screen.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepProgress {
    pub key: String,
    pub label: String,
    pub state: StepState,
    /// Bytes done and in all (downloads and the unpack; 0 for the rest).
    pub done: u64,
    pub total: u64,
    /// What it is doing, or why it failed or was left out.
    pub note: String,
    pub download: bool,
}

/// A setup's progress, as the UI polls it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub steps: Vec<StepProgress>,
    /// Over all downloads.
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// Bytes a second over the last few seconds.
    pub speed: f64,
    pub eta_seconds: Option<u64>,
    pub message: String,
}

/// How long the speed is averaged over.
const WINDOW: Duration = Duration::from_secs(8);

#[derive(Default)]
struct Inner {
    steps: Vec<StepProgress>,
    /// Bytes that came over the network, and when: what the speed is read from.
    arrived: u64,
    samples: VecDeque<(Instant, u64)>,
    message: String,
    result: Option<Result<Outcome>>,
}

#[derive(Default)]
struct Shared {
    cancel: AtomicBool,
    finished: AtomicBool,
    inner: Mutex<Inner>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn set_steps(&self, plan: &Plan) {
        let steps = plan
            .steps
            .iter()
            .map(|s| StepProgress {
                key: s.step.key(),
                label: s.label.clone(),
                state: if s.done {
                    StepState::Done
                } else if s.skipped.is_some() {
                    StepState::Skipped
                } else {
                    StepState::Pending
                },
                done: if s.done { s.bytes } else { s.have },
                total: s.bytes,
                // A file the user's own models folder has is not downloaded: that says so.
                note: s.skipped.clone().or_else(|| s.linked.as_ref().map(|_| LINKED_NOTE.to_string())).unwrap_or_default(),
                download: s.step.is_download(),
            })
            .collect();
        self.lock().steps = steps;
    }

    fn set(&self, index: usize, state: StepState, note: &str) {
        let mut inner = self.lock();
        if let Some(step) = inner.steps.get_mut(index) {
            step.state = state;
            step.note = note.to_string();
            if state == StepState::Done {
                step.done = step.total;
            }
        }
    }

    fn note(&self, index: usize, note: &str) {
        if let Some(step) = self.lock().steps.get_mut(index) {
            step.note = note.to_string();
        }
    }

    fn state_of(&self, index: usize) -> Option<StepState> {
        self.lock().steps.get(index).map(|s| s.state)
    }

    /// A step has `done` of its bytes. With `network`, the growth counts towards the speed.
    fn bytes(&self, index: usize, done: u64, network: bool) {
        let mut inner = self.lock();
        let Some(step) = inner.steps.get_mut(index) else { return };
        let grew = done.saturating_sub(step.done);
        step.done = done.min(step.total.max(done));
        if network && grew > 0 {
            inner.arrived = inner.arrived.saturating_add(grew);
            let now = Instant::now();
            if inner.samples.back().is_none_or(|(at, _)| now.duration_since(*at) >= Duration::from_millis(250)) {
                let arrived = inner.arrived;
                inner.samples.push_back((now, arrived));
                while inner.samples.front().is_some_and(|(at, _)| now.duration_since(*at) > WINDOW) && inner.samples.len() > 2 {
                    inner.samples.pop_front();
                }
            }
        }
    }

    fn message(&self, text: &str) {
        let mut inner = self.lock();
        if inner.message != text {
            text.clone_into(&mut inner.message);
        }
    }

    fn progress(&self) -> Progress {
        let inner = self.lock();
        let downloads = || inner.steps.iter().filter(|s| s.download && s.state != StepState::Skipped);
        let bytes_total = downloads().fold(0u64, |sum, s| sum.saturating_add(s.total));
        let bytes_done = downloads().fold(0u64, |sum, s| sum.saturating_add(s.done.min(s.total)));
        let speed = match (inner.samples.front(), inner.samples.back()) {
            // A window that ended a while ago is no speed: nothing is arriving now.
            (Some((t0, b0)), Some((t1, b1))) if t1 > t0 && t1.elapsed() < Duration::from_secs(5) => {
                b1.saturating_sub(*b0) as f64 / t1.duration_since(*t0).as_secs_f64()
            }
            _ => 0.0,
        };
        let left = bytes_total.saturating_sub(bytes_done);
        let eta_seconds = (speed > 1.0 && left > 0).then(|| (left as f64 / speed).min(86_400.0 * 30.0) as u64);
        Progress { steps: inner.steps.clone(), bytes_done, bytes_total, speed, eta_seconds, message: inner.message.clone() }
    }
}

/// A setup running on its worker thread.
pub struct Handle {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// What it was started with.
    pub choices: Choices,
}

impl Handle {
    /// Start `job` on a worker thread.
    pub fn start(env: Arc<Env>, job: Job) -> Result<Handle> {
        // Planned once here, so a setup that can't be planned says so at once and the rows are
        // there for the first poll.
        let first = plan::plan(&job.manifest, &job.choices, &RealDisk)?;
        let shared = Arc::new(Shared::default());
        shared.set_steps(&first);
        let choices = job.choices.clone();
        let worker = shared.clone();
        // On disk before anything else is: an app closed in the middle finds a setup to continue.
        record(&env, &job, Phase::Installing, None, "");
        let thread = std::thread::Builder::new()
            .name("photocraft-ai-setup".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&env, &job, &worker)))
                    .unwrap_or_else(|_| Err(Error::Engine("The setup stopped with an internal error. What arrived is kept: continue to try again.".into())));
                match &result {
                    Ok(done) => record(&env, &job, Phase::Done, Some(done), ""),
                    Err(Error::Cancelled) => record(&env, &job, Phase::Paused, None, ""),
                    Err(e) => record(&env, &job, Phase::Failed, None, &e.to_string()),
                }
                worker.lock().result = Some(result);
                worker.finished.store(true, Ordering::Release);
            })
            .map_err(|e| Error::Engine(format!("could not start the setup: {e}")))?;
        Ok(Handle { shared, thread: Some(thread), choices })
    }

    pub fn progress(&self) -> Progress {
        self.shared.progress()
    }

    /// Stop. What arrived is kept; the next start continues from it.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    pub fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }

    /// The result, once: `None` while it runs, and after it has been taken.
    pub fn take(&mut self) -> Option<Result<Outcome>> {
        if !self.finished() {
            return None;
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.shared.lock().result.take()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // The app is closing, or the session is gone: stop writing to disk.
        self.cancel();
    }
}

/// Note in the setup's record how far it got. The record keeps what else it holds (that the
/// wizard was offered); failing to write it is not a reason to fail the setup.
fn record(env: &Env, job: &Job, phase: Phase, done: Option<&Outcome>, error: &str) {
    let Some(path) = &env.state_file else { return };
    let mut state = state::load(path);
    state.phase = phase;
    state.choices = job.choices.clone();
    state.comfy_version = job.manifest.comfy.version.clone();
    state.error = error.to_string();
    if let Some(done) = done {
        state.installed = done.installed.clone();
        state.port = done.port;
    }
    let _ = state::save(path, &state);
}

/// The drive a path is on, where the system says (Windows); elsewhere every path is taken to
/// share one, which only ever asks for more room than needed.
fn drive_of(path: &Path) -> String {
    match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => prefix.as_os_str().to_string_lossy().to_ascii_uppercase(),
        _ => String::new(),
    }
}

/// A folder the setup writes to: there, and writable.
fn writable(dir: &Path, what: &str) -> Result<()> {
    let refuse = |e: std::io::Error| Error::Disk(format!("The {what} folder `{}` can't be written to ({e}). Choose a folder you can write to.", dir.display()));
    std::fs::create_dir_all(dir).map_err(refuse)?;
    let probe = dir.join(".layershop-write-test");
    std::fs::write(&probe, b"ok").map_err(refuse)?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// What can be known to fail before anything is downloaded: a path too long, a folder that
/// can't be written to, a drive without the room.
pub fn preflight(job: &Job, plan: &Plan, layout: &Layout, free_space: fn(&Path) -> Option<u64>) -> Result<()> {
    let managed = job.choices.route == Route::Managed;
    if managed {
        if let Some(why) = plan::path_too_long(job.choices.platform, job.manifest.asset(&job.choices.variant)?, layout) {
            return Err(Error::Config(why));
        }
        writable(&layout.engine_dir, "engine")?;
    }
    if plan.models_bytes > 0 || managed {
        writable(&layout.models_dir, "models")?;
    }
    let room = |dir: &Path, needed: u64, what: &str| match free_space(dir) {
        Some(free) if needed > 0 && free < needed.saturating_add(plan::HEADROOM) => {
            let drive = drive_of(dir);
            let place = if drive.is_empty() { format!("the drive of `{}`", dir.display()) } else { format!("drive {drive}") };
            Err(Error::Disk(format!(
                "Not enough room on {place}: {} is free, and {what} needs {} (with {} to spare). Free some space, choose fewer models, or choose another folder.",
                human_bytes(free),
                human_bytes(needed),
                human_bytes(plan::HEADROOM)
            )))
        }
        _ => Ok(()),
    };
    let engine = if managed { plan.engine_bytes } else { 0 };
    if managed && drive_of(&layout.engine_dir) == drive_of(&layout.models_dir) {
        room(&layout.models_dir, engine.saturating_add(plan.models_bytes), "the engine and the models")
    } else {
        if managed {
            room(&layout.engine_dir, engine, "the engine")?;
        }
        room(&layout.models_dir, plan.models_bytes, "the models")
    }
}

/// Bring one file down (the runtime archive, or a model file).
fn fetch_step(env: &Env, job: &Job, layout: &Layout, shared: &Shared, index: usize, planned: &Planned, stop: &(dyn Fn() -> bool + Sync)) -> Result<()> {
    let (url, size, sha256, dest, token) = match &planned.step {
        Step::FetchRuntime => {
            let asset = job.manifest.asset(&job.choices.variant)?;
            (asset.url.as_str(), asset.size, asset.sha256.as_str(), layout.archive.clone(), None)
        }
        Step::FetchFile(key) => {
            let file = job.manifest.files.get(key).ok_or_else(|| Error::Config(format!("`{key}` is not a model file")))?;
            // The token is only for the files that need it.
            (file.url.as_str(), file.size, file.sha256.as_str(), layout.model_file(key)?, job.token.as_deref().filter(|_| file.gated))
        }
        _ => return Ok(()),
    };
    shared.set(index, StepState::Active, "");
    if planned.recheck {
        shared.note(index, "Checking");
        if download::verify(&dest, size, sha256, stop, &|n| shared.bytes(index, n, false))? {
            return Ok(());
        }
        // Not the file it should be: it is replaced.
        shared.bytes(index, 0, false);
        shared.note(index, "Damaged: downloading again");
    }
    let ctx = download::Ctx { fetch: env.fetch.as_ref(), cancelled: stop, sleep: &env.sleep, free_space: &env.free_space };
    // What arrived before is read again first (its checksum continues from there): on a large
    // file that takes a moment, in which the row would otherwise look stuck.
    let resumed = planned.have;
    if resumed > 0 {
        shared.note(index, "Checking what arrived");
    }
    download::download(&ctx, &Spec { url, size, sha256, token }, &dest, &|n| {
        if resumed > 0 && n > resumed {
            shared.note(index, "");
        }
        shared.bytes(index, n, true);
    })
}

/// Unpack the runtime archive into the engine folder, then delete the archive.
fn apply_executable_permissions(staging: &Path, executables: &[String]) -> Result<()> {
    for relative in executables {
        if !crate::manifest::is_plain_rel(relative) {
            return Err(Error::Archive(format!("The engine manifest names `{relative}` as an executable, but it is not a safe path inside the build.")));
        }
        let path = staging.join(relative);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| Error::Archive(format!("The engine archive has no executable `{relative}`: it is not the build this version of the app installs.")))?;
        if !metadata.is_file() {
            return Err(Error::Archive(format!(
                "The engine archive's declared executable `{relative}` is not a regular file: it is not the build this version of the app installs."
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| Error::Disk(format!("Could not make `{}` executable: {e}.", path.display())))?;
        }
    }
    Ok(())
}

/// The folder an engine a repair moved out of the way sits in while its replacement unpacks:
/// `root` with [`runtime::REPLACED`] appended to its name, beside where the engine stands.
fn replaced_of(root: &Path) -> PathBuf {
    let mut name = root.as_os_str().to_os_string();
    name.push(runtime::REPLACED);
    PathBuf::from(name)
}

/// Remove an engine set aside for a repair, when its marker says this app put it there. A folder
/// without the marker is not this app's and stays.
fn clear_replaced(replaced: &Path) -> Result<()> {
    if replaced.is_dir() && runtime::read_marker(replaced).is_some() {
        std::fs::remove_dir_all(replaced).map_err(|e| Error::Disk(format!("Could not clear the engine set aside in `{}`: {e}.", replaced.display())))?;
    }
    Ok(())
}

/// Put a job's engine folder right before anything else looks at it. A repair interrupted between
/// moving the old engine aside and unpacking its replacement leaves a [`runtime::REPLACED`]
/// folder beside the empty space the engine belongs in: the old engine goes back. Where a new
/// engine did arrive, that one stands and the set-aside folder, marked by this app, is its to
/// delete. A folder without the marker is not this app's and stays.
fn recover_replaced(layout: &Layout) {
    let replaced = replaced_of(&layout.engine_root);
    if !replaced.is_dir() || runtime::read_marker(&replaced).is_none() {
        return;
    }
    if layout.engine_root.exists() {
        let _ = clear_replaced(&replaced);
    } else {
        let _ = std::fs::rename(&replaced, &layout.engine_root);
    }
}

/// The empty file that tells Spotlight to leave a folder alone.
const NEVER_INDEX: &str = ".metadata_never_index";

/// Tell macOS to leave a folder this app can rebuild by itself alone: `tmutil addexclusion`
/// takes it out of Time Machine's way, and an empty `.metadata_never_index` out of Spotlight's.
/// The engine is reproducible from its pinned, checksummed archive, so both are pure cost on
/// the ~2 GB of Python and ComfyUI it unpacks. Neither is owed the install: on a machine that
/// has no `tmutil`, on which it refuses, or on which the marker file cannot be written, the
/// engine works just the same - so `run`'s answer, and any write error, stop here.
fn exclude_from_backup_and_index(dir: &Path, run: &dyn Fn(&str, &[&str]) -> bool) {
    let path = dir.to_string_lossy();
    run("tmutil", &["addexclusion", path.as_ref()]);
    let _ = std::fs::write(dir.join(NEVER_INDEX), b"");
}

/// Runs a system tool with its stdin, stdout and stderr all sent away, and answers only
/// whether it exited zero: a tool that is missing or refuses is a plain `false`, never an error.
#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn run_quietly(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// In the browser there are no system tools to run, so nothing to exclude a folder with: the
/// answer no managed install could ever need there.
#[cfg(all(not(test), target_arch = "wasm32"))]
fn run_quietly(_: &str, _: &[&str]) -> bool {
    false
}

/// In tests nothing is run at all: every call is recorded for [`process_calls`] to read back
/// and answered as [`set_process_reply`] last said. The log is process-wide because a setup
/// runs on its worker thread and the tests run in parallel - a rig's folders live in its own
/// tempdir, so a test only ever counts the calls that name its own.
#[cfg(test)]
fn run_quietly(program: &str, args: &[&str]) -> bool {
    PROCESS_CALLS.lock().unwrap_or_else(PoisonError::into_inner).push((program.to_string(), args.iter().map(|a| a.to_string()).collect()));
    PROCESS_REPLY.load(Ordering::SeqCst)
}

#[cfg(test)]
static PROCESS_CALLS: Mutex<Vec<(String, Vec<String>)>> = Mutex::new(Vec::new());

#[cfg(test)]
static PROCESS_REPLY: AtomicBool = AtomicBool::new(true);

/// What a system tool answers for the rest of the test run: a fake that refuses lets a test
/// show the install carries on without it.
#[cfg(test)]
fn set_process_reply(ok: bool) {
    PROCESS_REPLY.store(ok, Ordering::SeqCst);
}

/// Every system tool call made so far, in order: the program, then its arguments.
#[cfg(test)]
fn process_calls() -> Vec<(String, Vec<String>)> {
    PROCESS_CALLS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Unpack the runtime archive into the engine folder, then delete the archive. A marked engine
/// already standing there is moved aside rather than deleted, so a repair that fails or is
/// cancelled puts it back (see the body).
fn unpack_step(job: &Job, layout: &Layout, shared: &Shared, index: usize, stop: &dyn Fn() -> bool) -> Result<()> {
    shared.set(index, StepState::Active, "");
    let asset = job.manifest.asset(&job.choices.variant)?;
    let marker = Marker::managed(job.choices.platform, &job.manifest.comfy.version, &job.choices.variant, &asset.sha256);
    // The Python this asset's own manifest entry names, not the catalogue's default:
    // an asset may ship its runtime somewhere else.
    let paths = job.manifest.runtime_paths(&job.choices.variant)?;
    let python = paths.python.split('/').fold(PathBuf::new(), |p, part| p.join(part));
    let finish = |staging: &Path| {
        // An archive that unpacks without the Python it is started with is not an engine.
        if !staging.join(&python).is_file() {
            return Err(Error::Archive(format!("The engine archive has no `{}`: it is not the build this version of the app installs.", paths.python)));
        }
        apply_executable_permissions(staging, &asset.executables)?;
        runtime::write_marker(staging, &marker)
    };
    // The repair of a damaged engine: this step only runs when a file of the engine is missing,
    // and a folder carrying this app's own marker is the one an earlier unpack of this setup
    // filled - it is the setup's to replace. Rather than delete it and hope the fresh unpack
    // comes off, it is moved aside under a name that says this setup set it aside, so a failed
    // or stopped repair puts the old engine straight back instead of leaving the user with none.
    // `unpack` still refuses any non-empty folder without that marker (it did not put it there).
    let replaced = replaced_of(&layout.engine_root);
    if runtime::read_marker(&layout.engine_root).is_some() {
        // One still set aside from an earlier attempt is this setup's to clear first (the job's
        // start normally has; this also covers a repair called without one).
        clear_replaced(&replaced)?;
        std::fs::rename(&layout.engine_root, &replaced)
            .map_err(|e| Error::Disk(format!("Could not move the damaged engine out of `{}` to unpack a new one: {e}.", layout.engine_root.display())))?;
    }
    let done =
        unpack::unpack(&layout.archive, &layout.engine_root, &Limits::default(), stop, &|done, total| shared.bytes(index, done.min(total), false), &finish);
    match done {
        Ok(_) => {
            // The new engine stands in the old one's place: what it replaced is no longer wanted.
            let _ = clear_replaced(&replaced);
            // 2 GB the engine no longer needs.
            let _ = std::fs::remove_file(&layout.archive);
            // And the ~2 GB it is: Python and ComfyUI the machine can lay down again from the
            // pinned, checksummed archive, so neither Time Machine nor Spotlight is to spend
            // itself on them. The macOS managed engine only, and a polite ask: see the helper.
            if job.choices.platform == RuntimePlatform::MacosArm64 {
                exclude_from_backup_and_index(&layout.engine_root, &run_quietly);
            }
            Ok(())
        }
        Err(e) => {
            // The engine that stood here goes back, as long as nothing took its place while the
            // new one failed to arrive. Should even that fail, the next job's start finds the
            // set-aside folder and restores it; the error stays the one this attempt hit.
            if replaced.is_dir() && runtime::read_marker(&replaced).is_some() && !layout.engine_root.exists() {
                let _ = std::fs::rename(&replaced, &layout.engine_root);
            }
            match e {
                // Its checksum was right when it arrived: damaged since, or not what was expected.
                // Either way the next try starts from a fresh download.
                Error::Archive(why) => {
                    let _ = std::fs::remove_file(&layout.archive);
                    Err(Error::Archive(why))
                }
                e => Err(e),
            }
        }
    }
}

fn index_of(plan: &Plan, step: &Step) -> Option<usize> {
    plan.steps.iter().position(|s| s.step == *step)
}

/// The model folders as they stand now: the engine's own folder and the user's own folders
/// (managed), or the server's listing the wizard captured plus what has been downloaded into
/// the server's models folder (the user's own ComfyUI). A run lists this before its first
/// download and again after the last one to tell what it made ready.
fn model_folders(job: &Job, layout: &Layout, managed: bool) -> BTreeMap<String, Vec<String>> {
    let mut folders =
        if managed { runtime::list_all_models(&layout.models_dir, &layout.extra_models) } else { job.choices.server_files.clone().unwrap_or_default() };
    if !managed {
        // What was downloaded into the server's models folder is there too.
        for (folder, names) in runtime::list_models(&layout.models_dir) {
            let have = folders.entry(folder).or_default();
            for name in names {
                if !have.iter().any(|f| f.rsplit(['/', '\\']).next() == Some(name.as_str())) {
                    have.push(name);
                }
            }
        }
    }
    folders
}

fn run(env: &Env, job: &Job, shared: &Shared) -> Result<Outcome> {
    let layout = Layout::new(&job.manifest, &job.choices)?;
    let managed = job.choices.route == Route::Managed;
    // First thing, before anything reads the engine folder: an engine a repair interrupted
    // between setting it aside and unpacking its replacement goes back, or is cleared when the
    // replacement arrived (see [`recover_replaced`]).
    if managed {
        recover_replaced(&layout);
    }
    let plan = plan::plan(&job.manifest, &job.choices, &RealDisk)?;
    shared.set_steps(&plan);
    shared.message("Checking the folders");
    preflight(job, &plan, &layout, env.free_space)?;

    // What stands in the models folders before anything is downloaded: the test image compares
    // readiness after the run against this to find the models this run made ready (task 028).
    // Only a run that renders a test image asks.
    let folders_before = index_of(&plan, &Step::Verify).is_some().then(|| model_folders(job, &layout, managed));

    // ----- downloads, two at a time; the engine is unpacked as soon as its archive is there -----
    let downloads: Vec<(usize, &Planned)> = plan.steps.iter().enumerate().filter(|(_, s)| s.step.is_download() && !s.done && s.skipped.is_none()).collect();
    let next = AtomicUsize::new(0);
    let abort = AtomicBool::new(false);
    let failure: Mutex<Option<Error>> = Mutex::new(None);
    let stop = || shared.cancelled() || abort.load(Ordering::Relaxed);
    let fail = |index: usize, e: Error| {
        shared.set(index, StepState::Failed, &e.to_string());
        failure.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert(e);
        abort.store(true, Ordering::Relaxed);
    };
    let stopped = || {
        failure.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert(Error::Cancelled);
        abort.store(true, Ordering::Relaxed);
    };
    if !downloads.is_empty() {
        shared.message("Downloading");
    }
    std::thread::scope(|scope| {
        let worker = || {
            while let Some((index, planned)) = downloads.get(next.fetch_add(1, Ordering::SeqCst)).copied() {
                if stop() {
                    break;
                }
                match fetch_step(env, job, &layout, shared, index, planned, &stop) {
                    Ok(()) => shared.set(index, StepState::Done, ""),
                    // A gated file without access: its models are left out, the rest carries on.
                    Err(Error::Access(why)) => shared.set(index, StepState::Skipped, &why),
                    // Stopped: nothing after this is to run on half a download.
                    Err(Error::Cancelled) => {
                        shared.set(index, StepState::Pending, "");
                        stopped();
                        break;
                    }
                    Err(e) => {
                        fail(index, e);
                        break;
                    }
                }
            }
        };
        let spawned = [scope.spawn(worker), scope.spawn(worker)];
        if let (true, Some(unpack_index)) = (managed, index_of(&plan, &Step::UnpackRuntime).filter(|i| plan.steps.get(*i).is_some_and(|s| !s.done))) {
            // Wait for the archive (it may be there already), then unpack beside the downloads.
            let fetched = index_of(&plan, &Step::FetchRuntime);
            let ready = loop {
                match fetched.and_then(|i| shared.state_of(i)) {
                    Some(StepState::Done) | None => break true,
                    Some(StepState::Failed | StepState::Skipped) => break false,
                    _ if stop() => break false,
                    _ => std::thread::sleep(Duration::from_millis(50)),
                }
            };
            if ready && !stop() {
                match unpack_step(job, &layout, shared, unpack_index, &stop) {
                    Ok(()) => shared.set(unpack_index, StepState::Done, ""),
                    Err(Error::Cancelled) => {
                        shared.set(unpack_index, StepState::Pending, "");
                        stopped();
                    }
                    Err(e) => fail(unpack_index, e),
                }
            }
        }
        for handle in spawned {
            let _ = handle.join();
        }
    });
    // A cancel wins over whatever else went wrong while stopping.
    if shared.cancelled() {
        return Err(Error::Cancelled);
    }
    if let Some(e) = failure.lock().unwrap_or_else(PoisonError::into_inner).take() {
        return Err(e);
    }

    // ----- where the engine finds the models -----
    let (url, launch, port) = if managed {
        if let Some(i) = index_of(&plan, &Step::WriteModelPaths) {
            shared.set(i, StepState::Active, "");
            runtime::write_model_paths(&layout)?;
            shared.set(i, StepState::Done, "");
        }
        // The C++ runtime PyTorch needs, beside the engine's Python: a PC without the Visual C++
        // Redistributable (a new one, Windows Sandbox) runs the engine too. Every run looks, so
        // Continue or Repair mends an engine unpacked before this, with nothing downloaded again.
        if let (true, Some(sources), Some(python_dir)) = (job.choices.platform.needs_windows_support(), &env.vc_runtime, layout.python.parent()) {
            shared.message("Checking the engine's runtime");
            if let Err(e) = vcrt::ensure(python_dir, sources) {
                if let Some(row) = index_of(&plan, &Step::Verify).or_else(|| index_of(&plan, &Step::WriteModelPaths)) {
                    shared.set(row, StepState::Failed, &e.to_string());
                }
                return Err(e);
            }
        }
        let port = if job.port != 0 {
            job.port
        } else {
            runtime::free_port(runtime::FIRST_PORT)
                .ok_or_else(|| Error::Engine("No free port was found for the engine (8190 to 8389 are all taken).".into()))?
        };
        (runtime::url(port), Some(runtime::launch_for(job.choices.platform, &job.manifest, &layout, job.choices.cpu)), port)
    } else {
        (job.url.clone(), job.launch.clone(), 0)
    };

    // ----- the settings -----
    let configure_index = index_of(&plan, &Step::Configure);
    if let Some(i) = configure_index {
        shared.set(i, StepState::Active, "");
    }
    // The managed macOS engine writes its `user`, `input`, `output` and `temp` folders into the
    // data folder beside the engine folder (runtime::MACOS_DATA_ARGS): make them before it can be
    // started. Idempotent, so Repair and Continue re-run it. Nothing is made on Windows, whose
    // launch does not mention those folders.
    if managed && job.choices.platform == RuntimePlatform::MacosArm64 {
        runtime::create_data_dirs(&layout)?;
    }
    shared.message("Setting up the models");
    let folders = model_folders(job, &layout, managed);
    let mut settings = job.settings.clone();
    let configured = match (&launch, managed) {
        (Some(launch), true) => configure::managed(&mut settings, &job.manifest, &url, launch, &folders),
        _ => configure::existing(&mut settings, &job.manifest, &url, launch.as_ref(), &folders),
    };
    let installed: Vec<String> = configure::installed(&job.manifest, &folders).into_iter().filter(|id| job.choices.models.contains(id)).collect();
    // A chosen model that is not whole was left out: say why (the plan's reason, or the step's).
    let skipped = plan
        .models
        .iter()
        .filter(|m| !installed.contains(&m.id))
        .map(|m| {
            let why = m.skipped.clone().or_else(|| {
                let keys = job.manifest.model(&m.id).map(|e| e.files.clone()).unwrap_or_default();
                let progress = shared.progress();
                progress.steps.iter().find(|s| s.state == StepState::Skipped && keys.iter().any(|k| s.key == format!("file:{k}"))).map(|s| s.note.clone())
            });
            PlannedModel { id: m.id.clone(), name: m.name.clone(), skipped: Some(why.unwrap_or_else(|| "one of its files is missing".into())) }
        })
        .collect();
    if let Some(i) = configure_index {
        shared.set(i, StepState::Done, "");
    }

    // ----- the engine, and the test image -----
    let mut outcome = Outcome { route: job.choices.route, url, launch, port, folders, installed, skipped, test: None, test_error: None };
    if let Some(i) = index_of(&plan, &Step::Verify) {
        shared.set(i, StepState::Active, "Starting the engine");
        shared.message("Starting the engine");
        let cancelled = || shared.cancelled();
        // A render says only that it renders: what the engine printed says how far (loading the
        // model, step 7 of 20), and the clock that it still moves, on the processor for an hour.
        let rendering_since = Mutex::new(None::<Instant>);
        let progress = |_: f32, text: &str| {
            let text = text.trim_end_matches('…');
            let since = || *rendering_since.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(Instant::now);
            let words = if text.starts_with("Generating") {
                render_words(text, since().elapsed().as_secs(), env.server.as_deref())
            } else if from_render(text) {
                // ComfyUI said which step it is on: the clock is all there is to add.
                format!("{text} · {}", photocraft_aigen::activity::clock(since().elapsed().as_secs()))
            } else {
                text.to_string()
            };
            shared.note(i, &words);
            shared.message(&words);
        };
        let sleep = |d: Duration| (env.sleep)(d);
        let control = Control { cancelled: &cancelled, progress: &progress, sleep: &sleep };
        let from_engine = |e: photocraft_aigen::Error| match e {
            photocraft_aigen::Error::Cancelled => Error::Cancelled,
            e => Error::Engine(e.to_string()),
        };
        let started = match &env.server {
            Some(server) => server.ensure(env.transport.as_ref(), &settings, &control).map_err(from_engine),
            None if photocraft_aigen::launch::is_running(env.transport.as_ref(), &outcome.url) => Ok(()),
            None => Err(Error::Engine(format!("ComfyUI is not answering at {}.", outcome.url))),
        };
        if let Err(e) = started {
            let e = match e {
                Error::Engine(why) if !job.choices.cpu && no_card(&why) => Error::Engine(format!(
                    "{why} The engine could not use a graphics card on this PC. Without one it can use (in a virtual machine, say), \
local models can only run on the processor: open this setup again and continue it, and it starts the engine that way. Cloud models work on any PC."
                )),
                e => e,
            };
            if e != Error::Cancelled {
                shared.set(i, StepState::Failed, &e.to_string());
            }
            return Err(e);
        }
        // The models this run has just made ready: whole now, and not standing whole in the
        // folders before it started (for a model the manifest does not know: not runnable
        // before either). The engine counts as changed when this run unpacked it - a plan that
        // called for the unpack really did run it, as any failure returned before here - and
        // on the processor, whose run never renders and keeps its own note for any ready model.
        let empty = BTreeMap::new();
        let before = folders_before.as_ref().unwrap_or(&empty);
        let whole_before = configure::installed(&job.manifest, before);
        let ready_before = configure::runnable(&job.settings, before);
        let new_ids: Vec<String> = configured
            .ready
            .iter()
            .filter(|id| match job.manifest.model(id) {
                Some(_) => !whole_before.contains(id),
                None => !ready_before.contains(id),
            })
            .cloned()
            .collect();
        let engine_changed = managed && (job.choices.cpu || plan.steps.iter().any(|s| s.step == Step::UnpackRuntime && !s.done));
        let test_model =
            configure::test_model(&settings, &configured.ready, &new_ids, engine_changed).filter(|m| outcome.installed.contains(&m.id) || !managed);
        match test_model {
            // On the processor these models take far too long (and too much memory) for a test.
            Some(_) if managed && job.choices.cpu => {
                outcome.test_error = Some(
                    "The engine is installed and runs on the processor: this PC has no graphics card it can use. No test image was rendered: \
on the processor these models take a very long time per image, if they fit in memory at all. Cloud models work on any PC."
                        .into(),
                );
                shared.set(i, StepState::Done, "Running on the processor");
            }
            Some(model) => {
                shared.note(i, &format!("Rendering a test image with {}", model.name));
                let job = photocraft_aigen::Job {
                    prompt: TEST_PROMPT.into(),
                    width: TEST_SIZE,
                    height: TEST_SIZE,
                    seed: TEST_SEED,
                    alpha: false,
                    source: None,
                    loras: Vec::new(),
                    animation: None,
                };
                let began = Instant::now();
                match photocraft_aigen::generate(env.transport.as_ref(), &settings, model, "", &job, &control) {
                    Ok(image) => {
                        outcome.test = Some(TestImage {
                            bytes: image.bytes,
                            seconds: began.elapsed().as_secs_f64(),
                            model: model.id.clone(),
                            model_name: model.name.clone(),
                            width: image.width,
                            height: image.height,
                        });
                        shared.set(i, StepState::Done, "");
                    }
                    Err(photocraft_aigen::Error::Cancelled) => return Err(Error::Cancelled),
                    // The engine runs; the picture did not come. That is not a failed install.
                    Err(e) => {
                        let why = e.to_string();
                        let lower = why.to_ascii_lowercase();
                        // System memory (the paging file) comes with its own advice; this is the card's.
                        let card = !lower.contains("paging file") && !lower.contains("os error 1455");
                        let hint = if card && (lower.contains("out of memory") || lower.contains("allocation")) {
                            " The model may be too large for this graphics card: try a smaller model, or a lower render size in its settings."
                        } else {
                            ""
                        };
                        outcome.test_error = Some(format!("The engine is installed and running, but the test image did not render: {why}{hint}"));
                        shared.set(i, StepState::Done, "The test image did not render");
                    }
                }
            }
            None => {
                if configured.ready.is_empty() {
                    outcome.test_error =
                        Some("The engine is installed and running. No model was installed whole, so there was nothing to render a test image with.".into());
                    shared.set(i, StepState::Done, "No model to test with");
                } else {
                    // The task 028 rule: a run that added no new model and left the engine alone
                    // has nothing a test image could prove.
                    outcome.test_error = Some(
                        "The engine is installed and running. This run added no new model and left the engine as it was, so no test image was rendered.".into(),
                    );
                    shared.set(i, StepState::Done, "Nothing new to test");
                }
            }
        }
    }
    shared.message("Done");
    Ok(outcome)
}

/// The test image's row while it renders: `text` ("Generating with …"), how far the engine says
/// it is, and for how long it has been at it.
fn render_words(text: &str, seconds: u64, server: Option<&dyn LocalServer>) -> String {
    let clock = photocraft_aigen::activity::clock(seconds);
    match server.and_then(|s| photocraft_aigen::activity::describe(&s.output(80))) {
        Some(doing) => format!("{text}: {doing} · {clock}"),
        None => format!("{text} · {clock}"),
    }
}

/// Is `text` ComfyUI's own word on the render (over its websocket): the step it is on?
fn from_render(text: &str) -> bool {
    ["Loading ", "Reading the ", "Rendering with ", "Finishing the image", "Waiting for ComfyUI"].iter().any(|p| text.starts_with(p))
}

/// Did the engine die reaching for a graphics card that isn't there (or a driver that can't
/// serve it)? PyTorch then crashes in `torch\cuda`, or says it found no NVIDIA driver.
fn no_card(why: &str) -> bool {
    let lower = why.to_ascii_lowercase();
    lower.contains("found no nvidia driver")
        || lower.contains("torch not compiled with cuda")
        || ((lower.contains("torch\\cuda") || lower.contains("torch/cuda")) && lower.contains("access violation"))
}

/// The settings a finished setup leaves: `outcome` applied to the settings as they are now.
pub fn apply(settings: &mut Settings, manifest: &Manifest, outcome: &Outcome) -> configure::Configured {
    match (&outcome.launch, outcome.route) {
        (Some(launch), Route::Managed) => configure::managed(settings, manifest, &outcome.url, launch, &outcome.folders),
        _ => configure::existing(settings, manifest, &outcome.url, outcome.launch.as_ref(), &outcome.folders),
    }
}

#[cfg(test)]
mod tests;
