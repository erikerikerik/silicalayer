//! A whole setup against a fake network and a fake engine: nothing leaves the test's folder.

use super::*;
use crate::fetch::{Begin, Head, Request, Sink};
use crate::plan::part_of;
use photocraft_aigen::{Method, Response, ServerState};
use sevenz_rust2::{ArchiveEntry, ArchiveWriter};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A stand-in for the portable build: its top folder, a Python and a script.
fn build_archive(dir: &Path, with_python: bool) -> Vec<u8> {
    let path = dir.join("fixture.7z");
    let mut writer = ArchiveWriter::create(&path).unwrap();
    writer.push_archive_entry::<&[u8]>(ArchiveEntry::new_directory("ComfyUI_windows_portable"), None).unwrap();
    if with_python {
        writer.push_archive_entry(ArchiveEntry::new_file("ComfyUI_windows_portable/python_embeded/python.exe"), Some(&b"MZ python"[..])).unwrap();
    }
    writer.push_archive_entry(ArchiveEntry::new_file("ComfyUI_windows_portable/ComfyUI/main.py"), Some(&b"print('comfy')"[..])).unwrap();
    writer.finish().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    bytes
}

/// The contents of a model file in the tests: a few kilobytes that differ by file.
fn contents(key: &str) -> Vec<u8> {
    let seed = key.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    (0..40_000u32).map(|i| (i.wrapping_mul(seed) >> 7) as u8).collect()
}

/// The real manifest cut down to three models, with files small enough to serve from memory.
fn manifest(archive: &[u8]) -> Manifest {
    let mut m = Manifest::bundled().unwrap().clone();
    m.models.retain(|x| ["qwen-image-2-1", "z-image-turbo", "flux-3-klein"].contains(&x.id.as_str()));
    let used: Vec<String> = m.models.iter().flat_map(|x| x.files.clone()).collect();
    m.files.retain(|key, _| used.contains(key));
    for (key, file) in &mut m.files {
        let bytes = contents(key);
        (file.size, file.sha256) = (bytes.len() as u64, sha(&bytes));
    }
    m.presets.clear();
    let asset = m.comfy.assets.get_mut("nvidia").unwrap();
    (asset.size, asset.sha256, asset.unpacked_bytes) = (archive.len() as u64, sha(archive), 1000);
    m
}

/// What the fake network does with an address.
#[derive(Clone, Copy, PartialEq)]
enum Net {
    Serve,
    /// Send this many bytes, then stop as a cancel would.
    StopAfter(usize),
    /// Send this many bytes to one address in total and then lose the connection, on every
    /// request after that as well: a network that dies mid-file and stays down. Unlike
    /// `StopAfter` the download does not treat this as a cancel, and it retries until giving up.
    DropAfter(usize),
    Down,
    /// Refuse without the right token.
    Gated,
    Corrupt,
}

#[derive(Default)]
struct FakeNet {
    files: HashMap<String, Vec<u8>>,
    rules: Mutex<HashMap<String, Net>>,
    /// Bytes served, by address.
    served: Mutex<HashMap<String, u64>>,
    tokens: Mutex<Vec<(String, Option<String>)>>,
}

impl FakeNet {
    fn new(m: &Manifest, archive: &[u8]) -> Self {
        let mut files: HashMap<String, Vec<u8>> = m.files.iter().map(|(key, f)| (f.url.clone(), contents(key))).collect();
        files.insert(m.comfy.assets["nvidia"].url.clone(), archive.to_vec());
        Self { files, ..Self::default() }
    }

    fn rule(&self, m: &Manifest, key: &str, rule: Net) {
        let url = if key == "runtime" { m.comfy.assets["nvidia"].url.clone() } else { m.files[key].url.clone() };
        self.rules.lock().unwrap().insert(url, rule);
    }

    fn served(&self, m: &Manifest, key: &str) -> u64 {
        let url = if key == "runtime" { m.comfy.assets["nvidia"].url.clone() } else { m.files[key].url.clone() };
        self.served.lock().unwrap().get(&url).copied().unwrap_or(0)
    }

    fn total_served(&self) -> u64 {
        self.served.lock().unwrap().values().sum()
    }
}

impl Fetch for FakeNet {
    fn head(&self, url: &str, _: Option<&str>) -> Result<Head> {
        Ok(Head { status: 200, size: self.files.get(url).map(|f| f.len() as u64), sha256: None, ..Head::default() })
    }

    fn fetch(&self, request: &Request<'_>, sink: &mut dyn Sink, _: &dyn Fn() -> bool) -> Result<()> {
        self.tokens.lock().unwrap().push((request.url.to_string(), request.token.map(str::to_string)));
        let rule = self.rules.lock().unwrap().get(request.url).copied().unwrap_or(Net::Serve);
        let data = self.files.get(request.url).ok_or_else(|| crate::fetch::status_error(404, request.url))?;
        let from = (request.offset as usize).min(data.len());
        let mut send = |bytes: &[u8]| {
            sink.begin(&Begin { offset: from as u64, total: Some(data.len() as u64), validator: "\"v1\"".into() })?;
            *self.served.lock().unwrap().entry(request.url.to_string()).or_default() += bytes.len() as u64;
            sink.write(bytes)
        };
        match rule {
            Net::Serve => send(&data[from..]),
            Net::StopAfter(n) => {
                send(&data[from..(from + n).min(data.len())])?;
                Err(Error::Cancelled)
            }
            Net::DropAfter(n) => {
                let given = self.served.lock().unwrap().get(request.url).copied().unwrap_or(0);
                let left = (n as u64).saturating_sub(given).min((data.len() - from) as u64) as usize;
                send(&data[from..from + left])?;
                Err(Error::Network("the connection dropped in the middle of the file".into()))
            }
            Net::Down => Err(Error::Network("no route to host".into())),
            Net::Gated if request.token != Some("hf_ok") => Err(crate::fetch::status_error(401, request.url)),
            Net::Gated => send(&data[from..]),
            Net::Corrupt => send(&vec![0x55; data.len() - from]),
        }
    }
}

const CARD_OUT_OF_MEMORY: &str = "Allocation on device: out of memory";

/// A ComfyUI that renders one fixed "image", or fails as `fails` says.
struct FakeComfy {
    running: bool,
    /// The `exception_message` the render fails with, if it does.
    fails: Option<&'static str>,
    prompts: AtomicU64,
}

impl Transport for FakeComfy {
    fn send(&self, request: &photocraft_aigen::Request) -> photocraft_aigen::Result<Response> {
        let ok = |body: &str| Ok(Response { status: 200, body: body.as_bytes().to_vec() });
        if !self.running {
            return Err(photocraft_aigen::Error::Network("refused".into()));
        }
        let url = request.url.as_str();
        if url.ends_with("/system_stats") {
            ok(r#"{"system":{"argv":["main.py"]}}"#)
        } else if url.ends_with("/prompt") && request.method == Method::Post {
            self.prompts.fetch_add(1, Ordering::SeqCst);
            ok(r#"{"prompt_id":"p1"}"#)
        } else if let (true, Some(why)) = (url.contains("/history/p1"), self.fails) {
            ok(
                &serde_json::json!({"p1": {"status": {"status_str": "error", "messages": [["execution_error", {"node_type": "KSampler", "exception_message": why}]]}}}).to_string(),
            )
        } else if url.contains("/history/p1") {
            ok(r#"{"p1":{"outputs":{"9":{"images":[{"filename":"test.png","subfolder":"","type":"output"}]}},"status":{"completed":true}}}"#)
        } else if url.contains("/view?") {
            ok("\u{89}PNG test image")
        } else {
            Ok(Response { status: 404, body: Vec::new() })
        }
    }
}

#[derive(Default)]
struct FakeServer {
    /// What `ensure` answers; the settings it was asked to start.
    refuse: Option<String>,
    asked: Mutex<Vec<Settings>>,
    /// What it last printed.
    printed: Vec<String>,
}

impl LocalServer for FakeServer {
    fn output(&self, lines: usize) -> Vec<String> {
        self.printed.iter().rev().take(lines).rev().cloned().collect()
    }

    fn ensure(&self, _: &dyn Transport, settings: &Settings, _: &Control<'_>) -> photocraft_aigen::Result<()> {
        self.asked.lock().unwrap().push(settings.clone());
        match &self.refuse {
            Some(why) => Err(photocraft_aigen::Error::Server(why.clone())),
            None => Ok(()),
        }
    }

    fn stop(&self) -> bool {
        false
    }

    fn state(&self) -> ServerState {
        ServerState::NotStarted
    }

    fn find(&self, _: &dyn Transport, _: &str) -> photocraft_aigen::Result<Launch> {
        Ok(Launch::default())
    }
}

struct Rig {
    dir: tempfile::TempDir,
    manifest: Manifest,
    net: Arc<FakeNet>,
    comfy: Arc<FakeComfy>,
    server: Arc<FakeServer>,
    /// Where the C++ runtime is put beside the engine's Python from (`None`: it isn't).
    vc_runtime: std::cell::RefCell<Option<crate::vcrt::Sources>>,
}

fn plenty(_: &Path) -> Option<u64> {
    Some(u64::MAX)
}

fn cramped(_: &Path) -> Option<u64> {
    Some(1_000_000)
}

impl Rig {
    fn new() -> Self {
        Self::with(true, FakeComfy { running: true, fails: None, prompts: AtomicU64::new(0) }, FakeServer::default())
    }

    fn with(python: bool, comfy: FakeComfy, server: FakeServer) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let archive = build_archive(dir.path(), python);
        let manifest = manifest(&archive);
        let net = Arc::new(FakeNet::new(&manifest, &archive));
        Self { dir, manifest, net, comfy: Arc::new(comfy), server: Arc::new(server), vc_runtime: Default::default() }
    }

    fn state_file(&self) -> PathBuf {
        self.dir.path().join("settings").join("ai-setup.json")
    }

    fn choices(&self, models: &[&str]) -> Choices {
        Choices {
            models: models.iter().map(|s| s.to_string()).collect(),
            engine_dir: self.dir.path().join("engine").to_string_lossy().into_owned(),
            models_dir: self.dir.path().join("models").to_string_lossy().into_owned(),
            variant: "nvidia".into(),
            test_image: true,
            ..Choices::default()
        }
    }

    fn job(&self, choices: Choices) -> Job {
        Job { manifest: self.manifest.clone(), choices, settings: Settings::default(), token: None, port: 0, url: String::new(), launch: None }
    }

    /// Run a setup to its end.
    fn run_job(&self, job: Job, free_space: fn(&Path) -> Option<u64>) -> (Result<Outcome>, Progress) {
        let net = self.net.clone();
        self.run_job_with(job, &net, free_space)
    }

    /// Run a setup whose manifest points at a different fake network: what a test does to
    /// try an asset or an archive the rig's own manifest does not describe.
    fn run_job_with(&self, job: Job, net: &Arc<FakeNet>, free_space: fn(&Path) -> Option<u64>) -> (Result<Outcome>, Progress) {
        let env = Env {
            fetch: net.clone(),
            transport: self.comfy.clone(),
            server: Some(self.server.clone()),
            state_file: Some(self.state_file()),
            sleep: |_| {},
            free_space,
            vc_runtime: self.vc_runtime.borrow().clone(),
        };
        let mut handle = Handle::start(Arc::new(env), job).unwrap();
        let began = Instant::now();
        loop {
            if let Some(result) = handle.take() {
                return (result, handle.progress());
            }
            assert!(began.elapsed() < Duration::from_secs(30), "the setup did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn run(&self, models: &[&str]) -> (Result<Outcome>, Progress) {
        self.run_job(self.job(self.choices(models)), plenty)
    }

    fn layout(&self) -> Layout {
        Layout::new(&self.manifest, &self.choices(&[])).unwrap()
    }

    fn phase(&self) -> Phase {
        state::load(&self.state_file()).phase
    }
}

const UNET: &str = "diffusion_models/qwen_image_2.1_int8_convrot.safetensors";
const ENCODER: &str = "text_encoders/qwen3vl_8b_fp8_scaled.safetensors";

/// The goal of the plan: from nothing to a test image, with no decisions left to make.
#[test]
fn a_clean_machine_ends_with_an_engine_its_models_and_a_test_image() {
    let rig = Rig::new();
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let done = result.unwrap();
    let layout = rig.layout();
    // The engine: unpacked without its top folder, marked, its archive gone again.
    assert_eq!(std::fs::read(&layout.python).unwrap(), b"MZ python");
    assert!(layout.script.is_file() && layout.model_paths.is_file());
    assert_eq!(
        runtime::read_marker(&layout.engine_root).unwrap(),
        Marker::managed(runtime::RuntimePlatform::WindowsPortable, "0.43.0", "nvidia", &rig.manifest.comfy.assets["nvidia"].sha256,)
    );
    assert!(!layout.archive.exists() && !part_of(&layout.archive).exists());
    // The models: each file whole, under its final name.
    for key in &rig.manifest.model("qwen-image-2-1").unwrap().files {
        assert_eq!(std::fs::read(layout.model_file(key).unwrap()).unwrap(), contents(key), "{key}");
    }
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert!(done.skipped.is_empty());
    assert_eq!(done.folders["vae"], ["qwen_image_2.1_vae_bf16.safetensors"]);
    // The engine gets a port of its own, not ComfyUI's usual ones, and is started as the
    // portable build starts itself.
    assert!(done.port >= runtime::FIRST_PORT && done.url == format!("http://127.0.0.1:{}", done.port));
    let launch = done.launch.clone().unwrap();
    assert_eq!(launch.python, layout.python.to_string_lossy());
    assert!(launch.args.contains("--disable-auto-launch") && launch.args.contains("layershop-models.yaml"));
    // The engine was started with exactly those settings, and rendered the test image.
    let asked = rig.server.asked.lock().unwrap();
    assert_eq!((asked.len(), asked[0].comfy_url.as_str(), asked[0].comfy_autostart, &asked[0].comfy_launch), (1, done.url.as_str(), true, &launch));
    let test = done.test.clone().unwrap();
    assert_eq!((test.model.as_str(), test.model_name.as_str(), test.bytes.as_slice()), ("qwen-image-2-1", "Qwen Image 2.1", "\u{89}PNG test image".as_bytes()));
    assert_eq!((done.test_error.clone(), rig.comfy.prompts.load(Ordering::SeqCst)), (None, 1));
    // Every row ended done, and the bytes add up.
    assert!(progress.steps.iter().all(|s| s.state == StepState::Done), "{:?}", progress.steps);
    assert_eq!(progress.bytes_done, progress.bytes_total);
    assert_eq!(
        progress.steps.iter().map(|s| s.key.as_str()).filter(|k| !k.starts_with("file:")).collect::<Vec<_>>(),
        ["runtime", "unpack", "paths", "configure", "verify"]
    );
    let record = state::load(&rig.state_file());
    assert_eq!(
        (record.phase, record.installed, record.comfy_version.as_str(), record.port),
        (Phase::Done, vec!["qwen-image-2-1".to_string()], "0.43.0", done.port)
    );
    // Applied to the settings as they are then: the panel offers what was installed.
    let mut settings = Settings::default();
    let configured = apply(&mut settings, &rig.manifest, &done);
    assert_eq!(configured.ready, ["qwen-image-2-1"]);
    assert_eq!((settings.comfy_url.as_str(), settings.comfy_autostart), (done.url.as_str(), true));
}

#[test]
fn a_setup_stopped_half_way_continues_with_only_what_is_missing() {
    let rig = Rig::new();
    rig.net.rule(&rig.manifest, ENCODER, Net::StopAfter(15_000));
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    assert_eq!(result.unwrap_err(), Error::Cancelled);
    assert_eq!(rig.phase(), Phase::Paused);
    let layout = rig.layout();
    // What arrived is kept: a part of the encoder, and nothing under its final name.
    let encoder = layout.model_file(ENCODER).unwrap();
    assert_eq!(std::fs::metadata(part_of(&encoder)).unwrap().len(), 15_000);
    assert!(!encoder.exists());
    assert!(progress.steps.iter().all(|s| s.state != StepState::Failed));
    assert!(rig.server.asked.lock().unwrap().is_empty(), "nothing is started on half an install");
    // Started again: planning finds what is there, and only the rest is asked for.
    let before = rig.net.total_served();
    let (unet_before, runtime_before) = (rig.net.served(&rig.manifest, UNET), rig.net.served(&rig.manifest, "runtime"));
    rig.net.rule(&rig.manifest, ENCODER, Net::Serve);
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let done = result.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert_eq!(std::fs::read(&encoder).unwrap(), contents(ENCODER));
    let again = rig.net.total_served() - before;
    let whole: u64 =
        rig.manifest.model("qwen-image-2-1").unwrap().files.iter().map(|k| rig.manifest.files[k].size).sum::<u64>() + rig.manifest.comfy.assets["nvidia"].size;
    assert!(again < whole, "{again} of {whole} bytes were fetched again");
    // A file that was finished is not fetched twice; the encoder only from where it stopped.
    let first_unet = unet_before.max(rig.net.served(&rig.manifest, UNET));
    assert_eq!(first_unet, rig.manifest.files[UNET].size);
    assert_eq!(rig.net.served(&rig.manifest, ENCODER), rig.manifest.files[ENCODER].size);
    assert_eq!(runtime_before.max(rig.net.served(&rig.manifest, "runtime")), rig.manifest.comfy.assets["nvidia"].size);
    assert!(progress.steps.iter().all(|s| s.state == StepState::Done));
    assert_eq!(rig.phase(), Phase::Done);
}

#[test]
fn adding_a_model_later_downloads_only_its_own_files_and_keeps_the_port() {
    let rig = Rig::new();
    let first = rig.run(&["qwen-image-2-1"]).0.unwrap();
    let before = rig.net.total_served();
    let mut job = rig.job(rig.choices(&["qwen-image-2-1", "z-image-turbo"]));
    job.port = first.port;
    let (result, progress) = rig.run_job(job, plenty);
    let done = result.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1", "z-image-turbo"]);
    assert_eq!(done.port, first.port);
    let added: u64 = rig.manifest.model("z-image-turbo").unwrap().files.iter().map(|k| rig.manifest.files[k].size).sum();
    assert_eq!(rig.net.total_served() - before, added);
    assert!(progress.steps.iter().all(|s| s.state == StepState::Done));
}

#[test]
fn a_gated_model_is_left_out_and_the_rest_installs() {
    let rig = Rig::new();
    let klein = "diffusion_models/flux3_image_klein_base_int8_convrot.safetensors";
    rig.net.rule(&rig.manifest, klein, Net::Gated);
    // No token: the plan leaves the model out, and nothing of it is asked for.
    let (result, progress) = rig.run(&["qwen-image-2-1", "flux-3-klein"]);
    let done = result.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert_eq!(
        done.skipped.iter().map(|m| (m.id.as_str(), m.skipped.as_deref().unwrap())).collect::<Vec<_>>(),
        [("flux-3-klein", "needs a Hugging Face sign-in")]
    );
    assert_eq!(rig.net.served(&rig.manifest, klein) + rig.net.served(&rig.manifest, "text_encoders/qwen3vl_4b_int8_convrot.safetensors"), 0);
    assert_eq!(progress.steps.iter().filter(|s| s.state == StepState::Skipped).count(), 3);
    assert!(done.test.is_some());
    // A token the model's licence was not accepted with: refused by the host, still not fatal.
    let mut job = rig.job(Choices { token: true, ..rig.choices(&["qwen-image-2-1", "flux-3-klein"]) });
    job.token = Some("hf_wrong".into());
    let (result, progress) = rig.run_job(job, plenty);
    let done = result.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert!(done.skipped[0].skipped.as_deref().unwrap().contains("gated"), "{:?}", done.skipped);
    let refused = progress.steps.iter().find(|s| s.key == format!("file:{klein}")).unwrap();
    assert_eq!(refused.state, StepState::Skipped);
    // With access: it installs, and the token went to the gated file only.
    let mut job = rig.job(Choices { token: true, ..rig.choices(&["qwen-image-2-1", "flux-3-klein"]) });
    job.token = Some("hf_ok".into());
    let done = rig.run_job(job, plenty).0.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1", "flux-3-klein"]);
    let tokens = rig.net.tokens.lock().unwrap();
    assert!(tokens.iter().all(|(url, token)| token.is_none() || *url == rig.manifest.files[klein].url), "the token went to a file that did not need it");
}

#[test]
fn a_network_that_is_down_fails_the_setup_and_leaves_it_resumable() {
    let rig = Rig::new();
    rig.net.rule(&rig.manifest, UNET, Net::Down);
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Network(m) if m.contains("kept failing")), "{e:?}");
    let failed = progress.steps.iter().find(|s| s.key == format!("file:{UNET}")).unwrap();
    assert!(failed.state == StepState::Failed && failed.note.contains("kept failing"));
    let record = state::load(&rig.state_file());
    assert!(record.phase == Phase::Failed && record.error.contains("kept failing") && record.interrupted());
    assert!(rig.server.asked.lock().unwrap().is_empty());
    // The network is back: the same setup finishes.
    rig.net.rule(&rig.manifest, UNET, Net::Serve);
    assert_eq!(rig.run(&["qwen-image-2-1"]).0.unwrap().installed, ["qwen-image-2-1"]);
}

#[test]
fn a_drive_without_the_room_is_found_before_anything_is_downloaded() {
    let rig = Rig::new();
    let (result, _) = rig.run_job(rig.job(rig.choices(&["qwen-image-2-1"])), cramped);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Disk(m) if m.contains("Not enough room") && m.contains("1.00 MB is free")), "{e:?}");
    assert_eq!(rig.net.total_served(), 0);
    assert_eq!(rig.phase(), Phase::Failed);
    // A folder that can't be one (a file is in the way) is said plainly too.
    std::fs::write(rig.dir.path().join("blocked"), b"a file").unwrap();
    let choices = Choices { models_dir: rig.dir.path().join("blocked").to_string_lossy().into_owned(), ..rig.choices(&["qwen-image-2-1"]) };
    let e = rig.run_job(rig.job(choices), plenty).0.unwrap_err();
    assert!(matches!(&e, Error::Disk(m) if m.contains("models folder") && m.contains("can't be written")), "{e:?}");
    assert_eq!(rig.net.total_served(), 0);
}

#[test]
fn a_repair_replaces_a_damaged_file_and_keeps_the_good_ones() {
    let rig = Rig::new();
    rig.run(&["qwen-image-2-1"]).0.unwrap();
    let layout = rig.layout();
    // Same length, other bytes: only a checksum can tell.
    let unet = layout.model_file(UNET).unwrap();
    std::fs::write(&unet, vec![0u8; contents(UNET).len()]).unwrap();
    let before = rig.net.total_served();
    let (result, progress) = rig.run_job(rig.job(Choices { recheck: true, ..rig.choices(&["qwen-image-2-1"]) }), plenty);
    assert_eq!(result.unwrap().installed, ["qwen-image-2-1"]);
    assert_eq!(std::fs::read(&unet).unwrap(), contents(UNET));
    assert_eq!(rig.net.total_served() - before, rig.manifest.files[UNET].size, "only the damaged file was fetched");
    assert!(progress.steps.iter().all(|s| s.state == StepState::Done));
    // A file that keeps arriving wrong fails the setup with both checksums.
    std::fs::remove_file(&unet).unwrap();
    rig.net.rule(&rig.manifest, UNET, Net::Corrupt);
    let e = rig.run(&["qwen-image-2-1"]).0.unwrap_err();
    assert!(matches!(&e, Error::Integrity(m) if m.contains(&rig.manifest.files[UNET].sha256)), "{e:?}");
    assert!(!unet.exists() && !part_of(&unet).exists());
}

/// The rule of task 028: a test image proves what a run changed. One that added no new model
/// and left the engine as it stood has nothing to prove, and the engine gets no prompt.
#[test]
fn a_run_that_adds_nothing_and_leaves_the_engine_alone_renders_no_test_image() {
    let rig = Rig::new();
    let first = rig.run(&["qwen-image-2-1"]).0.unwrap();
    assert!(first.test.is_some(), "the first install renders one");
    let prompts = rig.comfy.prompts.load(Ordering::SeqCst);
    // The same choices again, the engine and the model files as they are.
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let done = result.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert!(done.test.is_none(), "{:?}", done.test);
    assert!(done.test_error.clone().unwrap().contains("no new model"), "{:?}", done.test_error);
    assert_eq!(rig.comfy.prompts.load(Ordering::SeqCst), prompts, "no prompt was sent");
    assert!(progress.steps.iter().all(|s| s.state == StepState::Done), "{:?}", progress.steps);
}

/// A first install has a new engine to prove, and tests the model it installed - the only
/// one it has, and the one the panel would otherwise not have reached for.
#[test]
fn a_first_install_tests_the_model_it_installed() {
    let rig = Rig::new();
    let done = rig.run(&["z-image-turbo"]).0.unwrap();
    assert_eq!(done.installed, ["z-image-turbo"]);
    let test = done.test.unwrap();
    assert_eq!((test.model.as_str(), test.model_name.as_str()), ("z-image-turbo", "Z-Image Turbo"));
    assert_eq!(rig.comfy.prompts.load(Ordering::SeqCst), 1);
}

/// The other half of task 028: a run that replaced the engine renders a test image even when
/// no model is new - the engine is what has to prove it runs.
#[test]
fn a_repair_that_replaced_the_engine_still_renders_a_test_image() {
    let rig = Rig::new();
    rig.run(&["qwen-image-2-1"]).0.unwrap();
    let layout = rig.layout();
    // Damage the engine itself (an antivirus took the Python); the model files are untouched.
    std::fs::remove_file(&layout.python).unwrap();
    let (result, progress) = rig.run_job(rig.job(Choices { recheck: true, ..rig.choices(&["qwen-image-2-1"]) }), plenty);
    let done = result.unwrap();
    assert_eq!(std::fs::read(&layout.python).unwrap(), b"MZ python", "the engine was unpacked again");
    let test = done.test.unwrap_or_else(|| panic!("the repair rendered no test image: {:?}", done.test_error));
    assert_eq!(test.model.as_str(), "qwen-image-2-1", "no model is new, so the first ready one runs");
    assert!(progress.steps.iter().all(|s| s.state == StepState::Done), "{:?}", progress.steps);
}

#[test]
fn an_engine_that_does_not_start_fails_the_setup_with_its_own_words() {
    let server = FakeServer { refuse: Some("ComfyUI stopped while starting: ImportError: DLL load failed".into()), ..FakeServer::default() };
    let rig = Rig::with(true, FakeComfy { running: false, fails: None, prompts: AtomicU64::new(0) }, server);
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Engine(m) if m.contains("DLL load failed")), "{e:?}");
    let verify = progress.steps.last().unwrap();
    assert!(verify.key == "verify" && verify.state == StepState::Failed);
    // Everything before it is done and stays done: a repair has nothing to download again.
    assert!(progress.steps.iter().rev().skip(1).all(|s| s.state == StepState::Done));
    assert_eq!(rig.phase(), Phase::Failed);
}

/// A render that takes long says how far the engine is, and for how long it has been at it.
#[test]
fn a_rendering_test_image_says_how_far_the_engine_is() {
    let rendering = FakeServer {
        printed: vec!["got prompt".into(), "Requested to load QwenImage".into(), " 35%|###5      | 7/20 [00:41<03:50,  5.20s/it]".into()],
        ..FakeServer::default()
    };
    assert_eq!(render_words("Generating with Qwen Image 2.1", 754, Some(&rendering)), "Generating with Qwen Image 2.1: step 7 of 20, about 03:50 left · 12:34");
    let loading = FakeServer { printed: vec!["got prompt".into(), "Requested to load QwenImage".into()], ..FakeServer::default() };
    assert_eq!(render_words("Generating with X", 5, Some(&loading)), "Generating with X: loading QwenImage into memory · 0:05");
    // A server that prints nothing (or none at all) leaves the clock.
    assert_eq!(render_words("Generating with X", 61, Some(&FakeServer::default())), "Generating with X · 1:01");
    assert_eq!(render_words("Generating with X", 0, None), "Generating with X · 0:00");
    // ComfyUI's own word on the render gets the clock alone; the engine starting has its own.
    assert!(from_render("Rendering with X: step 7 of 20") && from_render("Loading X") && from_render("Finishing the image"));
    assert!(!from_render("Starting ComfyUI: loading PyTorch · 0:12") && !from_render("Sending to ComfyUI") && !from_render("Downloading the image"));
}

/// What a virtual machine said (2026-10-20): PyTorch reached for a graphics card that isn't there.
const NO_CARD_CRASH: &str = "ComfyUI stopped while starting: exit code 0xC0000005, a crash (access violation); it last printed: Windows fatal exception: \
access violation, in File \"C:\\e\\python_embeded\\Lib\\site-packages\\torch\\cuda\\__init__.py\", line 591 in _lazy_init";

#[test]
fn an_engine_that_crashes_reaching_for_a_missing_card_says_how_it_can_run() {
    let refused = |cpu: bool| {
        let server = FakeServer { refuse: Some(NO_CARD_CRASH.into()), ..FakeServer::default() };
        let rig = Rig::with(true, FakeComfy { running: false, fails: None, prompts: AtomicU64::new(0) }, server);
        let e = rig.run_job(rig.job(Choices { cpu, ..rig.choices(&["qwen-image-2-1"]) }), plenty).0.unwrap_err();
        let Error::Engine(why) = e else { panic!("{e:?}") };
        why
    };
    let why = refused(false);
    assert!(why.starts_with(NO_CARD_CRASH) && why.contains("on the processor") && why.contains("Cloud models"), "{why}");
    // On the processor already, a crash is something else: it is told as it is.
    assert_eq!(refused(true), NO_CARD_CRASH);
    // Other ways PyTorch says there is no card.
    assert!(no_card("RuntimeError: Found no NVIDIA driver on your system.") && no_card("AssertionError: Torch not compiled with CUDA enabled"));
    assert!(!no_card("ImportError: DLL load failed") && !no_card("access violation in python312.dll"));
}

/// A stand-in runtime DLL: a version resource saying 14.51.
fn runtime_dll(path: &Path) {
    let mut bytes = b"MZ".to_vec();
    bytes.extend_from_slice(&[0, 0]);
    for word in [0xFEEF_04BDu32, 0x0001_0000, (14 << 16) | 51, 36247 << 16] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

/// The engine's PyTorch needs msvcp140.dll, which a PC without the Visual C++ Redistributable
/// lacks (Windows Sandbox: WinError 126): the app's copy is put beside the engine's Python.
#[test]
fn the_cpp_runtime_is_put_beside_the_engines_python_and_continue_mends_an_engine_without_it() {
    use crate::vcrt::{DLLS, Sources};
    let rig = Rig::new();
    let bundle = rig.dir.path().join("vc-runtime");
    std::fs::create_dir_all(&bundle).unwrap();
    for name in DLLS {
        runtime_dll(&bundle.join(name));
    }
    let no_system = rig.dir.path().join("System32-without-it");
    *rig.vc_runtime.borrow_mut() = Some(Sources { bundled: Some(bundle), system: Some(no_system.clone()) });
    rig.run(&["qwen-image-2-1"]).0.unwrap();
    let python_dir = rig.layout().python.parent().unwrap().to_path_buf();
    for name in DLLS {
        assert_eq!(crate::vcrt::file_version(&python_dir.join(name)), Some([14, 51, 36247, 0]), "{name}");
    }
    // An engine unpacked before there was this, without the runtime: Continue (or Repair) puts it
    // there, and nothing is downloaded again.
    std::fs::remove_file(python_dir.join("msvcp140.dll")).unwrap();
    let served = rig.net.total_served();
    rig.run(&["qwen-image-2-1"]).0.unwrap();
    assert!(python_dir.join("msvcp140.dll").is_file());
    assert_eq!(rig.net.total_served(), served);
    // Nowhere to have it from: the setup stops at its last row, before starting the engine, and
    // says how to get it.
    std::fs::remove_file(python_dir.join("msvcp140.dll")).unwrap();
    let asked = rig.server.asked.lock().unwrap().len();
    *rig.vc_runtime.borrow_mut() = Some(Sources { bundled: None, system: Some(no_system) });
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Engine(m) if m.contains("msvcp140.dll") && m.contains(crate::vcrt::REDIST_URL)), "{e:?}");
    let verify = progress.steps.last().unwrap();
    assert!(verify.key == "verify" && verify.state == StepState::Failed && verify.note.contains("Visual C++"), "{verify:?}");
    assert_eq!(rig.server.asked.lock().unwrap().len(), asked, "the engine was not started without it");
}

#[test]
fn without_a_graphics_card_the_engine_runs_on_the_processor_and_renders_no_test_image() {
    let rig = Rig::new();
    let (result, progress) = rig.run_job(rig.job(Choices { cpu: true, ..rig.choices(&["qwen-image-2-1"]) }), plenty);
    let done = result.unwrap();
    // Started as ComfyUI's own run_cpu.bat starts it, and so in the settings it leaves.
    assert!(done.launch.as_ref().is_some_and(|l| l.args.ends_with("--cpu")), "{:?}", done.launch);
    assert!(done.test.is_none() && done.test_error.clone().unwrap().contains("processor"));
    assert_eq!(rig.comfy.prompts.load(Ordering::SeqCst), 0, "no test image on the processor");
    let verify = progress.steps.last().unwrap();
    assert_eq!((verify.state, verify.label.as_str()), (StepState::Done, "Start the engine on the processor"));
    // With a card, the same setup renders its test image.
    let rig = Rig::new();
    let done = rig.run(&["qwen-image-2-1"]).0.unwrap();
    assert!(done.launch.is_some_and(|l| !l.args.contains("--cpu")) && done.test.is_some());
}

#[test]
fn a_macos_managed_job_writes_its_platform_launches_without_windows_flags_and_skips_vcrt() {
    let rig = Rig::new();
    let mut manifest = rig.manifest.clone();
    let asset = manifest.comfy.assets["nvidia"].clone();
    manifest.comfy.assets.insert("macos_arm64".into(), asset);
    let choices = Choices { platform: runtime::RuntimePlatform::MacosArm64, variant: "macos_arm64".into(), test_image: false, ..rig.choices(&[]) };
    let layout = Layout::new(&manifest, &choices).unwrap();
    let mut job = rig.job(choices);
    job.manifest = manifest;
    // A Windows setup would fail to repair from these empty sources. macOS must never try.
    *rig.vc_runtime.borrow_mut() = Some(crate::vcrt::Sources { bundled: None, system: None });

    let done = rig.run_job(job, plenty).0.unwrap();
    let marker = runtime::read_marker(&layout.engine_root).unwrap();
    assert_eq!((marker.platform, marker.variant.as_str()), (RuntimePlatform::MacosArm64, "macos_arm64"));
    let launch = done.launch.unwrap();
    assert!(!launch.args.contains("--windows-standalone-build"));
    assert!(launch.args.contains("--disable-auto-launch"));
    // The Configure step made the writable folders, outside the engine folder, and the launch
    // points at them: an engine swap cannot take the user's workflows, uploads or output.
    let args = photocraft_aigen::launch::split_args(&launch.args);
    let after = |flag: &str| args.iter().position(|a| a == flag).map(|i| i + 1).and_then(|i| args.get(i).cloned());
    for folder in crate::plan::DATA_FOLDERS {
        assert!(layout.data_dir.join(folder).is_dir(), "{folder}");
        assert!(!layout.engine_root.join(folder).exists(), "{folder} inside the engine");
        assert_eq!(after(flag_for(folder)).as_deref(), Some(layout.data_dir.join(folder).to_string_lossy().as_ref()), "{folder}");
    }
    assert_eq!(after("--listen").as_deref(), Some("127.0.0.1"));
    assert!(args.iter().filter(|a| a.as_str() == "--disable-api-nodes").count() == 1);
    assert_eq!(launch.env.get("HF_HUB_OFFLINE").map(String::as_str), Some("1"));
}

/// The launch flag that points ComfyUI at one writable folder (see runtime::MACOS_DATA_ARGS).
fn flag_for(folder: &str) -> &'static str {
    crate::plan::DATA_FOLDERS.iter().copied().zip(runtime::MACOS_DATA_ARGS).find(|(f, _)| *f == folder).map(|(_, arg)| arg).unwrap()
}

/// A macOS managed setup with no models chosen: the rig's own offline archive serves as the
/// `macos_arm64` build (the same URL and checksum as `nvidia`), so a test can cancel, damage and
/// resume the engine install without touching anything but this folder. The fake network answers
/// as a cancel would (StopAfter) or as a restart in a new process does (bytes already on disk).
struct MacOs {
    rig: Rig,
    manifest: Manifest,
    layout: Layout,
}

impl MacOs {
    fn new() -> Self {
        let rig = Rig::new();
        let mut manifest = rig.manifest.clone();
        let asset = manifest.comfy.assets["nvidia"].clone();
        manifest.comfy.assets.insert("macos_arm64".into(), asset);
        let layout = Layout::new(&manifest, &self_choices(&rig)).unwrap();
        Self { rig, manifest, layout }
    }

    fn job(&self) -> Job {
        let mut job = self.rig.job(self_choices(&self.rig));
        job.manifest = self.manifest.clone();
        job
    }

    /// What the macOS launch of task 013 and the clean test check, on any job that completes:
    /// its marker, its Python, no Windows flag, the writable folders beside the engine, and
    /// nothing of the download left behind.
    fn assert_finished(&self, launch: &Launch, served: u64) {
        let (layout, manifest) = (&self.layout, &self.manifest);
        assert_eq!(
            runtime::read_marker(&layout.engine_root),
            Some(Marker::managed(RuntimePlatform::MacosArm64, &manifest.comfy.version, "macos_arm64", &manifest.comfy.assets["macos_arm64"].sha256))
        );
        assert_eq!(std::fs::read(&layout.python).unwrap(), b"MZ python");
        assert!(!launch.args.contains("--windows-standalone-build") && launch.args.contains("--disable-auto-launch"));
        assert!(!layout.archive.exists() && !part_of(&layout.archive).exists() && !part_of(&layout.archive).with_extension("part.etag").exists());
        for folder in crate::plan::DATA_FOLDERS {
            assert!(layout.data_dir.join(folder).is_dir(), "{folder}");
            assert!(!layout.engine_root.join(folder).exists(), "{folder} inside the engine");
        }
        assert!(layout.data_dir.starts_with(&layout.engine_dir) && !layout.data_dir.starts_with(&layout.engine_root));
        // The engine folder holds the engine, its model-paths file and nothing else: no half an
        // unpack, no archive.
        let mut names: Vec<String> = std::fs::read_dir(&layout.engine_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["0.43.0".to_string(), "data".to_string()]);
        assert_eq!(self.rig.net.served(&self.rig.manifest, "runtime"), served);
    }
}

fn self_choices(rig: &Rig) -> Choices {
    Choices { platform: runtime::RuntimePlatform::MacosArm64, variant: "macos_arm64".into(), test_image: false, ..rig.choices(&[]) }
}

/// Cancelling, a corrupt part, and a `.unpacking` folder left by a crash are shown on the paths
/// that can reach them: `Handle::cancel` while the archive downloads (the fetch itself answers
/// as a stop does, which is what `Net::StopAfter` stands for), a part written by an earlier
/// process, and unpack run with a stop that trips once bytes start landing on disk.
#[test]
fn a_macos_setup_cancelled_while_the_engine_downloads_pauses_and_continues_from_its_part() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let size = rig_mac.manifest.comfy.assets["macos_arm64"].size;
    rig.net.rule(&rig.manifest, "runtime", Net::StopAfter(10));
    let (result, progress) = rig.run_job(rig_mac.job(), plenty);
    assert_eq!(result.unwrap_err(), Error::Cancelled);
    // Paused, not failed: what arrived is kept, and nothing is started or marked.
    assert_eq!(rig.phase(), Phase::Paused);
    assert!(progress.steps.iter().all(|s| s.state != StepState::Failed), "{:?}", progress.steps);
    assert!(rig.server.asked.lock().unwrap().is_empty(), "nothing is started on half an engine");
    let part = part_of(&layout.archive);
    assert_eq!(std::fs::metadata(&part).unwrap().len(), 10, "the part of the archive that arrived is kept");
    assert!(!layout.archive.exists());
    assert!(!layout.engine_root.exists() && !unpack::staging_of(&layout.engine_root).exists());
    assert!(runtime::read_marker(&layout.engine_root).is_none() && runtime::installed_engines(&layout.engine_dir).is_empty());
    assert_eq!(rig.net.served(&rig.manifest, "runtime"), 10);

    // Started again: the part is checked and only what is missing comes down, the checksum is
    // verified, and the setup finishes as the clean macOS test says it should.
    rig.net.rule(&rig.manifest, "runtime", Net::Serve);
    let done = rig.run_job(rig_mac.job(), plenty).0.unwrap();
    assert!(done.installed.is_empty() && done.test.is_none());
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), size);
    // It came back to the network a second time, for the rest only: the two requests together
    // served the file once, not a file and a half (the served bytes in assert_finished).
    assert!(rig.net.tokens.lock().unwrap().len() >= 2);
    assert!(done.port >= runtime::FIRST_PORT);
}

#[test]
fn a_macos_setup_interrupted_while_unpacking_leaves_no_engine_and_restarts_cleanly() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let size = rig_mac.manifest.comfy.assets["macos_arm64"].size;
    let job = rig_mac.job();
    let env = Arc::new(Env {
        fetch: rig.net.clone(),
        transport: rig.comfy.clone(),
        server: Some(rig.server.clone()),
        state_file: None,
        sleep: |_| {},
        free_space: plenty,
        vc_runtime: rig.vc_runtime.borrow().clone(),
    });
    let plan = plan::plan(&job.manifest, &job.choices, &RealDisk).unwrap();
    let shared = Shared::default();
    shared.set_steps(&plan);
    let fetch = plan.steps.iter().position(|s| s.step == Step::FetchRuntime).unwrap_or_else(|| panic!("the plan has no runtime row"));
    fetch_step(&env, &job, &layout, &shared, fetch, &plan.steps[fetch], &|| false).unwrap();
    assert!(layout.archive.is_file());

    // Cancelled once the first bytes land in the staging folder: unpacking is Pending again,
    // and no half an engine is anywhere the app calls an engine.
    let unpack_row = plan.steps.iter().position(|s| s.step == Step::UnpackRuntime).unwrap_or_else(|| panic!("the plan has no unpack row"));
    // The stop a user's Cancel makes, asked at the same moments `unpack` asks it - before every
    // entry and after every chunk. It answers yes from the first bytes that land in the staging
    // folder on, so a real file is written to before the stop takes it away again.
    let stop = || shared.progress().steps.get(unpack_row).is_some_and(|s| s.done > 0);
    let e = unpack_step(&job, &layout, &shared, unpack_row, &stop).unwrap_err();
    assert_eq!(e, Error::Cancelled);
    // `unpack_step` leaves its row Active: it is the run above it that puts a cancelled row
    // back to Pending (install.rs), and nothing here ever marks the engine Failed.
    assert_eq!(shared.state_of(unpack_row), Some(StepState::Active));
    assert!(!layout.engine_root.exists() && runtime::read_marker(&layout.engine_root).is_none());
    assert!(!unpack::staging_of(&layout.engine_root).exists(), "the staging folder goes with the interrupt");
    assert!(layout.archive.is_file(), "the verified archive waits to be unpacked again");
    assert_eq!(rig.net.served(&rig.manifest, "runtime"), size);

    // A crash that takes the process takes the staging folder's cleanup with it: the folder it
    // leaves is no engine, whatever it holds.
    let staging = unpack::staging_of(&layout.engine_root);
    std::fs::create_dir_all(staging.join("python_embeded")).unwrap();
    std::fs::write(staging.join("python_embeded").join("python.exe"), b"MZ python").unwrap();
    assert!(runtime::read_marker(&staging).is_none());
    assert!(runtime::installed_engines(&layout.engine_dir).is_empty(), "the half unpack is not taken for an engine");

    // A restart clears it and finishes: unpacked from the archive that is there, nothing fetched.
    let mut again = rig_mac.job();
    again.port = 8_190;
    let (result, progress) = rig.run_job(again, plenty);
    let done = result.unwrap();
    assert_eq!(progress.steps.get(unpack_row).map(|s| s.state), Some(StepState::Done), "{:?}", progress.steps);
    assert!(!staging.exists(), "the stale staging folder is cleared");
    assert!(!layout.engine_root.join(crate::vcrt::DLLS[0]).exists(), "macOS never reaches for the Windows C++ runtime");
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), size);
    assert_eq!(done.port, 8_190, "the setup's own port, not one searched for on a test machine");
}

#[test]
fn a_macos_part_of_the_wrong_bytes_is_refused_by_the_checksum_and_fetched_whole() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let asset = rig_mac.manifest.comfy.assets["macos_arm64"].clone();
    // What an earlier process left: the right length, other bytes, as a truncated mirror or a
    // damaged drive leaves. Its checksum is the only thing that can tell, and it tells.
    std::fs::create_dir_all(&layout.engine_dir).unwrap();
    std::fs::write(part_of(&layout.archive), vec![0x5Au8; asset.size as usize]).unwrap();
    let done = rig.run_job(rig_mac.job(), plenty).0.unwrap();
    // Never unpacked - a correct engine stands there, from a whole fetch: the part alone
    // downloaded zero bytes and still the archive came down its full size.
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), asset.size);
    assert_eq!(rig.phase(), Phase::Done);
}

#[test]
fn a_part_of_the_wrong_bytes_never_reaches_its_final_name_on_any_platform() {
    // The download core refuses a damaged part on its own: the same guarantee the macOS
    // checksum test rides on, shown where no restart is involved at all.
    let dir = tempfile::tempdir().unwrap();
    let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let url = "https://example.invalid/engine.7z";
    let net = FakeNet { files: HashMap::from([(url.to_string(), data.clone())]), ..FakeNet::default() };
    let dest = dir.path().join("engine.7z");
    std::fs::write(part_of(&dest), vec![0x5Au8; data.len()]).unwrap();
    let ctx = download::Ctx { fetch: &net, cancelled: &|| false, sleep: &|_| {}, free_space: &plenty };
    download::download(&ctx, &download::Spec { url, size: data.len() as u64, sha256: &sha(&data), token: None }, &dest, &|_| {}).unwrap();
    // Rejected by the checksum, deleted, and fetched again whole: never renamed as it was.
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert!(!part_of(&dest).exists() && !part_of(&dest).with_extension("part.etag").exists());
    assert_eq!(net.served.lock().unwrap().get(url).copied(), Some(data.len() as u64));
}

/// The failures the managed macOS install can meet, and the recovery each one promises, on
/// the `macos_arm64` job: a drive without room, a connection that dies mid-file, a finished
/// archive damaged since it landed, a damaged engine, and a repair that must not go near a
/// model file it already verified. Everything answers from this folder and the offline fixtures.
#[test]
fn a_macos_setup_without_room_on_its_drive_stops_before_anything_is_downloaded() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let (result, progress) = rig.run_job(rig_mac.job(), cramped);
    let why = match result.unwrap_err() {
        Error::Disk(why) => why,
        e => panic!("{e:?}"),
    };
    // The folder it wanted to write to, the room there is, and the room it needs - all in one
    // sentence, before a single byte is asked for.
    assert!(why.contains("Not enough room") && why.contains("1.00 MB is free") && why.contains("needs") && why.contains("2.00 GB to spare"), "{why}");
    #[cfg(unix)]
    assert!(why.contains(&layout.models_dir.display().to_string()), "the message names the folder: {why}");
    assert_eq!(rig.net.total_served(), 0, "nothing was asked of the network");
    assert_eq!(rig.phase(), Phase::Failed);
    assert!(progress.steps.iter().all(|s| s.state == StepState::Pending), "{:?}", progress.steps);
    // And nothing was marked as an engine along the way.
    assert!(!layout.engine_root.exists() && runtime::read_marker(&layout.engine_root).is_none());
    assert!(runtime::installed_engines(&layout.engine_dir).is_empty());
}

#[test]
fn a_macos_engine_download_that_dies_mid_file_is_kept_and_finished_from_its_part() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let size = rig_mac.manifest.comfy.assets["macos_arm64"].size;
    rig.net.rule(&rig.manifest, "runtime", Net::DropAfter(10));
    let (result, progress) = rig.run_job(rig_mac.job(), plenty);
    // A network failure is told as one, with what to do about it - not as a cancel.
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Network(why) if why.contains("kept failing") && why.contains("check the connection")), "{e:?}");
    assert_eq!(rig.phase(), Phase::Failed);
    let row = progress.steps.iter().find(|s| s.key == "runtime").unwrap_or_else(|| panic!("{:?}", progress.steps));
    assert!(row.state == StepState::Failed && row.note.contains("kept failing"), "{row:?}");
    // What arrived is kept as the part - with its validator beside it, so the next start can
    // ask for the rest only if the file has not changed - and nothing else landed.
    assert_eq!(std::fs::metadata(part_of(&layout.archive)).unwrap().len(), 10);
    assert!(part_of(&layout.archive).with_extension("part.etag").exists());
    assert!(!layout.archive.exists());
    assert!(!layout.engine_root.exists() && !unpack::staging_of(&layout.engine_root).exists());
    assert!(runtime::read_marker(&layout.engine_root).is_none());
    assert!(rig.server.asked.lock().unwrap().is_empty(), "nothing is started on half an engine");
    let failed_after = rig.net.served(&rig.manifest, "runtime");
    assert_eq!(failed_after, 10);

    // Started again with the network back: the part is checked and only the rest comes down,
    // the checksum is verified, and the setup finishes as the clean macOS test says it should.
    rig.net.rule(&rig.manifest, "runtime", Net::Serve);
    let done = rig.run_job(rig_mac.job(), plenty).0.unwrap();
    assert_eq!(rig.net.served(&rig.manifest, "runtime") - failed_after, size - 10, "the rest only was fetched");
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), size);
}

#[test]
fn a_macos_archive_damaged_after_it_landed_is_refused_before_unpacking_and_fetched_again() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let asset = rig_mac.manifest.comfy.assets["macos_arm64"].clone();
    // What the next start finds: the archive under its final name at its full size - the plan
    // therefore has no download left - but damaged since it landed.
    std::fs::create_dir_all(&layout.engine_dir).unwrap();
    std::fs::write(&layout.archive, vec![0x55u8; asset.size as usize]).unwrap();
    let (result, progress) = rig.run_job(rig_mac.job(), plenty);
    // Read as far as its header, refused, and set aside before a byte was unpacked; the error
    // says what happens next.
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Archive(why) if why.contains("could not be read") && why.contains("downloaded again")), "{e:?}");
    assert!(!layout.archive.exists(), "the corrupt archive is gone, so the next start downloads afresh");
    assert!(!part_of(&layout.archive).exists());
    assert!(!layout.engine_root.exists() && !unpack::staging_of(&layout.engine_root).exists() && runtime::read_marker(&layout.engine_root).is_none());
    assert_eq!(progress.steps.iter().find(|s| s.key == "runtime").map(|s| s.state), Some(StepState::Done));
    assert_eq!(progress.steps.iter().find(|s| s.key == "unpack").map(|s| s.state), Some(StepState::Failed));
    assert_eq!(rig.net.total_served(), 0, "the corrupt file never went back to the network");

    // The next start: it comes down whole and the setup finishes.
    let done = rig.run_job(rig_mac.job(), plenty).0.unwrap();
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), asset.size);
}

#[test]
fn repairing_a_damaged_macos_engine_reunpacks_it_and_leaves_the_data_folder_alone() {
    let rig_mac = MacOs::new();
    let rig = &rig_mac.rig;
    let size = rig_mac.manifest.comfy.assets["macos_arm64"].size;
    let mut job = rig_mac.job();
    job.manifest.comfy.assets.get_mut("macos_arm64").unwrap().executables = vec!["ComfyUI/main.py".into()];
    rig.run_job(job, plenty).0.unwrap();
    let layout = rig_mac.layout.clone();
    let main_py = layout.engine_root.join("ComfyUI").join("main.py");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&main_py).unwrap().permissions().mode() & 0o777, 0o755, "unpacked executable");
    }
    // The user's own things in the data folder beside the engine: a repair must not go near them.
    let kept = layout.data_dir.join("my-workflow.json");
    std::fs::write(&kept, b"{}").unwrap();

    // Damage: the Python the plan checks for is gone (an antivirus took it), and main.py lost
    // its executable bit. The marker still says the folder is this setup's own.
    std::fs::remove_file(&layout.python).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&main_py, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    // Repair (recheck: true): the engine comes back whole - Python, the executable bit, and a
    // valid marker - its archive taken down again, and `<engineDir>/data` untouched. This run
    // exposed a real bug: unpack refused the engine folder its own earlier unpack had filled
    // ("already exists and is not empty"), so a damaged engine
    // could never be repaired. The fix is in `unpack_step` (install.rs): a folder carrying this
    // app's marker moves aside for the fresh unpack, back in place if the repair fails; a folder
    // without it still gets unpack's refusal.
    let mut job = rig_mac.job();
    job.manifest.comfy.assets.get_mut("macos_arm64").unwrap().executables = vec!["ComfyUI/main.py".into()];
    job.choices.recheck = true;
    let served_before = rig.net.served(&rig.manifest, "runtime");
    let done = rig.run_job(job, plenty).0.unwrap();
    assert_eq!(std::fs::read(&layout.python).unwrap(), b"MZ python", "the engine's Python is back");
    assert_eq!(std::fs::read(&main_py).unwrap(), b"print('comfy')");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&main_py).unwrap().permissions().mode() & 0o777, 0o755, "the executable bit is restored");
    }
    assert_eq!(std::fs::read(&kept).unwrap(), b"{}", "the repair never went near the data folder");
    assert_eq!(rig.net.served(&rig.manifest, "runtime") - served_before, size, "a fresh engine archive, nothing else");
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), 2 * size);
}

/// The reversibility of task 018's repair: the marked engine gives way to the fresh unpack by
/// being renamed aside (`<version>.replaced`) rather than deleted, so a repair that fails or is
/// cancelled puts the old engine back. What a crash leaves behind is put right when the next job
/// starts. All four tests ride on the macOS rig and its offline archive.
#[test]
fn a_repair_cancelled_while_unpacking_puts_the_old_engine_back() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    let marker = runtime::read_marker(&layout.engine_root).unwrap();

    // Damage: the Python the plan checks for is gone (an antivirus took it). The marker still
    // says the folder is this setup's own.
    std::fs::remove_file(&layout.python).unwrap();

    // A repair whose unpack is cancelled once the first bytes land in the staging folder, asked
    // at the same moments `unpack` asks it, as the interrupt test above does.
    let job = rig_mac.job();
    let env = Arc::new(Env {
        fetch: rig.net.clone(),
        transport: rig.comfy.clone(),
        server: Some(rig.server.clone()),
        state_file: None,
        sleep: |_| {},
        free_space: plenty,
        vc_runtime: rig.vc_runtime.borrow().clone(),
    });
    let plan = plan::plan(&job.manifest, &job.choices, &RealDisk).unwrap();
    let shared = Shared::default();
    shared.set_steps(&plan);
    let fetch = plan.steps.iter().position(|s| s.step == Step::FetchRuntime).unwrap_or_else(|| panic!("the plan has no runtime row"));
    fetch_step(&env, &job, &layout, &shared, fetch, &plan.steps[fetch], &|| false).unwrap();
    let unpack_row = plan.steps.iter().position(|s| s.step == Step::UnpackRuntime).unwrap_or_else(|| panic!("the plan has no unpack row"));
    let stop = || shared.progress().steps.get(unpack_row).is_some_and(|s| s.done > 0);
    let e = unpack_step(&job, &layout, &shared, unpack_row, &stop).unwrap_err();
    assert_eq!(e, Error::Cancelled);

    // The old engine is back where it stood - damaged as it was, marker and all - and nothing of
    // the repair is left beside it: nothing set aside, no staging folder, and the verified
    // archive waits to be unpacked again.
    assert!(layout.engine_root.is_dir());
    assert_eq!(runtime::read_marker(&layout.engine_root).as_ref(), Some(&marker));
    assert!(!layout.python.exists(), "the old engine came back as damaged as it stood");
    assert!(!replaced_of(&layout.engine_root).exists(), "what is back in place is not also set aside");
    assert!(!unpack::staging_of(&layout.engine_root).exists());
    assert!(layout.archive.is_file(), "the verified archive waits to be unpacked again");
    assert_eq!(runtime::installed_engines(&layout.engine_dir), vec![(layout.engine_root.clone(), marker)]);
}

#[test]
fn a_repair_whose_archive_is_corrupt_puts_the_old_engine_back_with_the_archive_error() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    let asset = rig_mac.manifest.comfy.assets["macos_arm64"].clone();
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    let marker = runtime::read_marker(&layout.engine_root).unwrap();

    // Damage on both sides: the engine lost its Python, and what stands where the archive
    // belongs has its size but not its contents - the plan sees the archive as fetched, and
    // unpacking is what reads it.
    std::fs::remove_file(&layout.python).unwrap();
    std::fs::write(&layout.archive, vec![0x55u8; asset.size as usize]).unwrap();

    let (result, progress) = rig.run_job(rig_mac.job(), plenty);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Archive(why) if why.contains("could not be read") && why.contains("downloaded again")), "{e:?}");
    assert_eq!(progress.steps.iter().find(|s| s.key == "unpack").map(|s| s.state), Some(StepState::Failed));

    // The answer is the archive error, and the engine that stood there is back - damaged as it
    // was, marker and all. The corrupt archive is gone, so the next start downloads afresh.
    assert!(layout.engine_root.is_dir());
    assert_eq!(runtime::read_marker(&layout.engine_root).as_ref(), Some(&marker));
    assert!(!layout.python.exists(), "the old engine came back as damaged as it stood");
    assert!(!replaced_of(&layout.engine_root).exists() && !unpack::staging_of(&layout.engine_root).exists());
    assert!(!layout.archive.exists(), "the corrupt archive is gone, so the next start downloads afresh");
    assert_eq!(rig.net.served(&rig.manifest, "runtime"), asset.size, "only the first install's archive was ever served");
}

#[test]
fn a_repair_that_finishes_leaves_the_new_engine_and_nothing_set_aside() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    let size = rig_mac.manifest.comfy.assets["macos_arm64"].size;

    // The user's own things in the data folder beside the engine: a repair must not go near them.
    let kept = layout.data_dir.join("my-workflow.json");
    std::fs::write(&kept, b"{}").unwrap();
    // What an earlier, interrupted repair left behind: an engine set aside beside the current
    // one. It is this setup's to clear, and it must not survive the repair that finishes.
    let aside = replaced_of(&layout.engine_root);
    std::fs::create_dir_all(&aside).unwrap();
    runtime::write_marker(&aside, &runtime::read_marker(&layout.engine_root).unwrap()).unwrap();
    // Damage: the Python the plan checks for is gone.
    std::fs::remove_file(&layout.python).unwrap();

    let served_before = rig.net.served(&rig.manifest, "runtime");
    let done = rig.run_job(rig_mac.job(), plenty).0.unwrap();

    // The new engine stands in place; the engines it replaced are gone, the set-aside folder
    // with them; and `<engineDir>/data` is untouched.
    assert_eq!(std::fs::read(&layout.python).unwrap(), b"MZ python", "the engine's Python is back");
    assert!(!aside.exists(), "no engine is left set aside once the new one is in place");
    assert_eq!(std::fs::read(&kept).unwrap(), b"{}", "the repair never went near the data folder");
    assert_eq!(rig.net.served(&rig.manifest, "runtime") - served_before, size, "a fresh engine archive, nothing else");
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), 2 * size);
}

#[test]
fn an_engine_set_aside_by_an_interrupted_repair_is_restored_or_cleared_at_the_next_start() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    std::fs::write(layout.engine_root.join("my-file.txt"), b"mine").unwrap();

    // A crash between setting the engine aside and unpacking its replacement leaves only the
    // `.replaced` folder. The next job's start renames the engine back before anything else: it
    // plans with the engine standing and asks the network for nothing.
    let aside = replaced_of(&layout.engine_root);
    std::fs::rename(&layout.engine_root, &aside).unwrap();
    let served_before = rig.net.served(&rig.manifest, "runtime");
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    assert_eq!(std::fs::read(layout.engine_root.join("my-file.txt")).unwrap(), b"mine", "the engine that was set aside is back in place");
    assert!(!aside.exists() && !unpack::staging_of(&layout.engine_root).exists());
    assert_eq!(rig.net.served(&rig.manifest, "runtime"), served_before, "the restored engine needed nothing downloaded");

    // What a crash leaves beside a new engine that did arrive: the newer one stands in place and
    // the set-aside folder is deleted by the next start - and it was never an installed engine.
    std::fs::create_dir_all(&aside).unwrap();
    runtime::write_marker(&aside, &runtime::read_marker(&layout.engine_root).unwrap()).unwrap();
    std::fs::write(aside.join("my-file.txt"), b"outdated").unwrap();
    assert_eq!(
        runtime::installed_engines(&layout.engine_dir).iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
        vec![layout.engine_root.clone()],
        "a folder set aside for a repair is not an installed engine"
    );
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    assert!(!aside.exists(), "a set-aside engine goes once a newer one stands in place");
    assert_eq!(std::fs::read(layout.engine_root.join("my-file.txt")).unwrap(), b"mine", "the engine in place is the one that stays");
}

/// Calls to a system tool naming a path under `dir`. The log is process-wide (a setup runs on
/// its worker thread beside the other tests), but a rig's folders live in its own tempdir, so
/// counting the calls that name this rig's paths is exact whatever else runs at the same time.
fn process_calls_under(dir: &Path) -> Vec<(String, Vec<String>)> {
    let dir = dir.to_string_lossy().into_owned();
    process_calls().into_iter().filter(|(_, args)| args.iter().any(|arg| arg.starts_with(&dir))).collect()
}

/// The managed engine unpacks ~2 GB of Python and ComfyUI that come back on their own from the
/// pinned, checksummed archive: a finished macOS install tells Time Machine and Spotlight to
/// leave it alone, and only it - what the user put beside it stays backed up and indexed.
#[test]
fn a_completed_macos_managed_install_excludes_its_engine_from_time_machine_and_spotlight() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    let root = layout.engine_root.to_string_lossy().into_owned();
    let tmutil: Vec<Vec<String>> =
        process_calls_under(&layout.engine_root).into_iter().filter(|(program, _)| program == "tmutil").map(|(_, args)| args).collect();
    assert_eq!(tmutil, vec![vec!["addexclusion".to_string(), root]], "one `tmutil addexclusion` of the engine, and nothing else");
    assert!(layout.engine_root.join(".metadata_never_index").is_file(), "Spotlight's marker stands in the engine folder");
}

#[test]
fn an_engine_whose_tmutil_fails_still_finishes_the_install() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    set_process_reply(false);
    let done = rig.run_job(rig_mac.job(), plenty).0.unwrap();
    assert!(done.launch.is_some(), "an exclusion that was refused is not a failed install");
    assert_eq!(process_calls_under(&layout.engine_root).len(), 1, "the call was still made");
    assert!(layout.engine_root.join(".metadata_never_index").is_file(), "a refused tool does not hold back the marker file");
}

#[test]
fn a_macos_install_leaves_its_models_and_data_folder_in_the_backup_and_the_index() {
    let rig_mac = MacOs::new();
    let (rig, layout) = (&rig_mac.rig, rig_mac.layout.clone());
    rig.run_job(rig_mac.job(), plenty).0.unwrap();
    // This very install did exclude the engine: without this the guards below would pass
    // just as well when no exclusion happens at all.
    let excluded = process_calls_under(&layout.engine_root).into_iter().filter(|(program, _)| program == "tmutil").count();
    assert_eq!(excluded, 1, "the engine itself was not excluded, so the absence below would prove nothing");
    assert!(layout.engine_root.join(".metadata_never_index").is_file(), "Spotlight's marker stands in the engine folder");
    // Both folders stand after a finished setup, so what is shown is the absence of a mark,
    // not of a folder.
    assert!(layout.models_dir.is_dir() && layout.data_dir.is_dir());
    for dir in [&layout.models_dir, &layout.data_dir] {
        let name = dir.display().to_string();
        assert!(process_calls_under(dir).is_empty(), "something beside the engine was excluded: {name}");
        assert!(!dir.join(".metadata_never_index").exists(), "a Spotlight marker stands in {name}");
    }
}

#[test]
fn a_windows_managed_install_excludes_nothing() {
    let rig = Rig::new();
    rig.run(&[]).0.unwrap();
    let layout = rig.layout();
    assert!(layout.engine_root.is_dir(), "the rig's engine did install");
    assert!(process_calls_under(&layout.engine_dir).is_empty(), "no system tool ran for a Windows engine");
    assert!(!layout.engine_root.join(".metadata_never_index").exists() && !layout.models_dir.join(".metadata_never_index").exists());
}

#[test]
fn a_macos_repair_never_touches_a_model_file_it_already_verified() {
    let rig_mac = MacOs::new();
    let rig = &rig_mac.rig;
    let mut job = rig_mac.job();
    job.choices.models = vec!["qwen-image-2-1".into()];
    let done = rig.run_job(job, plenty).0.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    let layout = rig_mac.layout.clone();
    let keys: Vec<String> = rig.manifest.model("qwen-image-2-1").unwrap().files.clone();
    // Every model file as it stands after the install: bytes, length and modification time.
    let whole: Vec<(std::path::PathBuf, std::time::SystemTime, u64)> = keys
        .iter()
        .map(|k| layout.model_file(k).unwrap())
        .map(|p| (p.clone(), std::fs::metadata(&p).unwrap().modified().unwrap(), std::fs::metadata(&p).unwrap().len()))
        .collect();
    let size = rig_mac.manifest.comfy.assets["macos_arm64"].size;
    let model_bytes: u64 = keys.iter().map(|k| rig.manifest.files[k].size).sum();
    let runtime_before = rig.net.served(&rig.manifest, "runtime");
    let models_before: Vec<u64> = keys.iter().map(|k| rig.net.served(&rig.manifest, k)).collect();

    // Damage the engine only; a recheck run then looks at every file again.
    std::fs::remove_file(&layout.python).unwrap();
    let mut job = rig_mac.job();
    job.choices.models = vec!["qwen-image-2-1".into()];
    job.choices.recheck = true;
    let done = rig.run_job(job, plenty).0.unwrap();
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    // The engine came down again; each model file was checked where it stands: not asked for,
    // not rewritten - the same length and the same modification time.
    assert_eq!(rig.net.served(&rig.manifest, "runtime") - runtime_before, size);
    for (key, served) in keys.iter().zip(&models_before) {
        assert_eq!(rig.net.served(&rig.manifest, key), *served, "{key} was fetched again");
    }
    for ((path, when, len), key) in whole.iter().zip(&keys) {
        let after = std::fs::metadata(path).unwrap();
        assert_eq!((after.len(), after.modified().unwrap()), (*len, *when), "`{}` was rewritten", path.display());
        assert_eq!(std::fs::read(path).unwrap(), contents(key));
    }
    // Two engine archives over the whole test and one copy of every model file: the repair
    // cost a second engine and not a single model byte.
    assert_eq!(rig.net.total_served(), 2 * size + model_bytes, "the repair fetched more than a fresh engine");
    rig_mac.assert_finished(done.launch.as_ref().unwrap(), 2 * size);
}

#[test]
fn a_test_image_that_does_not_render_is_not_a_failed_install() {
    let rig = Rig::with(true, FakeComfy { running: true, fails: Some(CARD_OUT_OF_MEMORY), prompts: AtomicU64::new(0) }, FakeServer::default());
    let (result, progress) = rig.run(&["qwen-image-2-1"]);
    let done = result.unwrap();
    let why = done.test_error.clone().unwrap();
    assert!(why.contains("installed and running") && why.contains("out of memory") && why.contains("smaller model"), "{why}");
    assert!(done.test.is_none() && done.installed == ["qwen-image-2-1"]);
    assert_eq!(progress.steps.last().map(|s| s.state), Some(StepState::Done));
    assert_eq!(rig.phase(), Phase::Done);
    // Windows is untouched: no data folder to make, and its launch carries no environment.
    assert!(!rig.layout().data_dir.exists());
    assert!(done.launch.as_ref().is_some_and(|l| l.env.is_empty() && !l.args.contains("--user-directory") && !l.args.contains("--listen")));
    // Windows' paging file ran out (error 1455): that is system memory, not the card's, and the
    // message says what to do about it.
    let paging = "The paging file is too small for this operation to complete. (os error 1455)";
    let rig = Rig::with(true, FakeComfy { running: true, fails: Some(paging), prompts: AtomicU64::new(0) }, FakeServer::default());
    let why = rig.run(&["qwen-image-2-1"]).0.unwrap().test_error.unwrap();
    assert!(why.contains("os error 1455") && why.contains("Virtual memory") && !why.contains("graphics card"), "{why}");
    // With no model installed there is nothing to render with: the engine alone is still a setup.
    let rig = Rig::new();
    let done = rig.run(&[]).0.unwrap();
    assert!(done.test.is_none() && done.test_error.clone().unwrap().contains("No model was installed"));
    assert!(rig.layout().python.is_file());
    assert_eq!(rig.comfy.prompts.load(Ordering::SeqCst), 0);
}

#[test]
fn an_archive_that_is_not_the_build_leaves_no_engine_and_is_fetched_again() {
    // The right checksum (the manifest pins it), but no Python inside.
    let rig = Rig::with(false, FakeComfy { running: true, fails: None, prompts: AtomicU64::new(0) }, FakeServer::default());
    let (result, progress) = rig.run(&[]);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Archive(m) if m.contains("python_embeded/python.exe")), "{e:?}");
    let layout = rig.layout();
    assert!(!layout.engine_root.exists() && !unpack::staging_of(&layout.engine_root).exists() && !layout.archive.exists());
    assert_eq!(progress.steps.iter().find(|s| s.key == "unpack").map(|s| s.state), Some(StepState::Failed));
}

#[test]
fn an_archive_built_for_another_asset_is_refused_by_that_assets_python() {
    // The catalogue's default Python is present, so a check on it alone would pass. The asset
    // names its own, deeper path: an archive without it is not the build that asset installs.
    let rig = Rig::with(true, FakeComfy { running: true, fails: None, prompts: AtomicU64::new(0) }, FakeServer::default());
    let mut manifest = rig.manifest.clone();
    let asset = manifest.comfy.assets.get_mut("nvidia").unwrap();
    (asset.python, asset.script) = (Some("runtime/python/python.exe".into()), Some("ComfyUI/main.py".into()));
    let archive = rig.net.files.get(&manifest.comfy.assets["nvidia"].url.clone()).unwrap().clone();
    let net = Arc::new(FakeNet::new(&manifest, &archive));
    let mut job = rig.job(rig.choices(&[]));
    job.manifest = manifest;
    let (result, _) = rig.run_job_with(job, &net, plenty);
    let e = result.unwrap_err();
    assert!(matches!(&e, Error::Archive(m) if m.contains("runtime/python/python.exe") && !m.contains("python_embeded/python.exe")), "{e:?}");
    let layout = rig.layout();
    assert!(!layout.engine_root.exists() && !unpack::staging_of(&layout.engine_root).exists() && !layout.archive.exists());
}

#[cfg(unix)]
#[test]
fn declared_runtime_executables_get_only_the_declared_unix_mode() {
    use std::os::unix::fs::PermissionsExt;

    let rig = Rig::new();
    let mut job = rig.job(rig.choices(&[]));
    job.manifest.comfy.assets.get_mut("nvidia").unwrap().executables = vec!["ComfyUI/main.py".into()];
    rig.run_job(job, plenty).0.unwrap();
    let mode = std::fs::metadata(rig.layout().engine_root.join("ComfyUI/main.py")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
}

#[test]
fn missing_or_non_file_declared_executables_refuse_the_archive_atomically() {
    for relative in ["missing-tool", "ComfyUI"] {
        let rig = Rig::new();
        let mut job = rig.job(rig.choices(&[]));
        job.manifest.comfy.assets.get_mut("nvidia").unwrap().executables = vec![relative.into()];
        let error = rig.run_job(job, plenty).0.unwrap_err();
        assert!(matches!(&error, Error::Archive(message) if message.contains(relative)), "{error:?}");
        let layout = rig.layout();
        assert!(!layout.engine_root.exists() && !unpack::staging_of(&layout.engine_root).exists() && !layout.archive.exists());
    }
}

#[test]
fn the_users_own_comfyui_gets_the_missing_files_and_nothing_else() {
    let rig = Rig::new();
    let server_files = BTreeMap::from([("diffusion_models".to_string(), vec!["qwen_image_2.1_int8_convrot.safetensors".to_string()])]);
    let choices = Choices {
        route: Route::Existing,
        engine_dir: String::new(),
        variant: String::new(),
        server_files: Some(server_files),
        ..rig.choices(&["qwen-image-2-1"])
    };
    let mut job = rig.job(choices);
    job.url = "http://127.0.0.1:8000".into();
    job.launch = Some(Launch { python: "C:/comfy/python.exe".into(), folder: "C:/comfy".into(), args: "main.py".into(), ..Launch::default() });
    let (result, progress) = rig.run_job(job, plenty);
    let done = result.unwrap();
    // No engine was installed, and the file their server has was not downloaded again.
    assert!(!rig.dir.path().join("engine").exists());
    assert_eq!(rig.net.served(&rig.manifest, UNET) + rig.net.served(&rig.manifest, "runtime"), 0);
    assert_eq!(rig.net.served(&rig.manifest, ENCODER), rig.manifest.files[ENCODER].size);
    assert_eq!((done.url.as_str(), done.port, done.route), ("http://127.0.0.1:8000", 0, Route::Existing));
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert!(done.folders["text_encoders"].contains(&"qwen3vl_8b_fp8_scaled.safetensors".to_string()));
    assert_eq!(progress.steps.iter().map(|s| s.key.as_str()).filter(|k| !k.starts_with("file:")).collect::<Vec<_>>(), ["configure", "verify"]);
    assert!(done.test.is_some());
    // Their launch settings are kept as theirs.
    let mut settings = Settings::default();
    apply(&mut settings, &rig.manifest, &done);
    assert_eq!((settings.comfy_url.as_str(), settings.comfy_launch.folder.as_str(), settings.comfy_autostart), ("http://127.0.0.1:8000", "C:/comfy", true));
}

/// The engine this app installs, with the user's own models folder beside its own: what that
/// folder has is used where it is, and what it lacks comes down into the setup's folder.
#[test]
fn the_users_own_models_folder_is_used_and_left_alone() {
    let rig = Rig::new();
    let mine = rig.dir.path().join("ComfyUIModels").join("models");
    let unet = mine.join("unet").join(UNET.rsplit('/').next().unwrap());
    std::fs::create_dir_all(unet.parent().unwrap()).unwrap();
    std::fs::write(&unet, vec![7u8; rig.manifest.files[UNET].size as usize]).unwrap();
    std::fs::create_dir_all(mine.join("loras")).unwrap();
    std::fs::write(mine.join("loras").join("style.safetensors"), b"lora").unwrap();
    let choices = Choices { extra_models_dirs: vec![mine.to_string_lossy().into_owned()], ..rig.choices(&["qwen-image-2-1"]) };
    let (result, progress) = rig.run_job(rig.job(choices), plenty);
    let done = result.unwrap();
    assert_eq!(rig.net.served(&rig.manifest, UNET), 0, "their copy is not downloaded again");
    assert_eq!(rig.net.served(&rig.manifest, ENCODER), rig.manifest.files[ENCODER].size);
    assert_eq!(done.installed, ["qwen-image-2-1"]);
    assert!(done.folders["loras"].contains(&"style.safetensors".to_string()));
    let row = progress.steps.iter().find(|s| s.key == format!("file:{UNET}")).unwrap();
    assert_eq!((row.state, row.note.as_str()), (StepState::Done, LINKED_NOTE));
    // Their folder is as it was: nothing added, nothing taken.
    let mut theirs: Vec<String> = walk(&mine);
    theirs.sort();
    assert_eq!(theirs.len(), 2, "{theirs:?}");
    assert!(!rig.layout().model_file(UNET).unwrap().exists());
    let yaml = std::fs::read_to_string(&rig.layout().model_paths).unwrap();
    assert!(yaml.contains(&mine.to_string_lossy().replace('\\', "\\\\")), "{yaml}");
}

/// Every file under `dir`, as paths.
fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        if entry.file_type().unwrap().is_dir() {
            out.extend(walk(&entry.path()));
        } else {
            out.push(entry.path().to_string_lossy().into_owned());
        }
    }
    out
}

#[test]
fn a_setup_that_cannot_be_planned_does_not_start() {
    let rig = Rig::new();
    let env = || {
        Arc::new(Env {
            fetch: rig.net.clone(),
            transport: rig.comfy.clone(),
            server: None,
            state_file: None,
            sleep: |_| {},
            free_space: plenty,
            vc_runtime: None,
        })
    };
    for choices in [
        rig.choices(&["no-such-model"]),
        Choices { models_dir: "relative".into(), ..rig.choices(&[]) },
        Choices { variant: "voodoo".into(), ..rig.choices(&[]) },
    ] {
        assert!(matches!(Handle::start(env(), rig.job(choices)), Err(Error::Config(_))));
    }
    assert!(!rig.dir.path().join("engine").exists() && rig.net.total_served() == 0);
    // Cancelled before it gets anywhere: it stops, and says so.
    rig.net.rule(&rig.manifest, "runtime", Net::StopAfter(10));
    let mut handle = Handle::start(env(), rig.job(Choices { test_image: false, ..rig.choices(&[]) })).unwrap();
    assert_eq!(handle.choices.variant, "nvidia");
    handle.cancel();
    let began = Instant::now();
    let result = loop {
        if let Some(result) = handle.take() {
            break result;
        }
        assert!(began.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(result.unwrap_err(), Error::Cancelled);
    assert!(handle.take().is_none(), "the result is handed over once");
}

#[test]
fn progress_reports_speed_only_while_bytes_arrive() {
    let shared = Shared::default();
    let rig = Rig::new();
    let plan = plan::plan(&rig.manifest, &rig.choices(&["qwen-image-2-1"]), &RealDisk).unwrap();
    shared.set_steps(&plan);
    let p = shared.progress();
    let all: u64 = plan.steps.iter().filter(|s| s.step.is_download()).map(|s| s.bytes).sum();
    assert_eq!((p.bytes_done, p.bytes_total, p.speed, p.eta_seconds), (0, all, 0.0, None));
    // Checking a file that is there moves its row, not the speed.
    shared.bytes(2, 10_000, false);
    assert_eq!((shared.progress().bytes_done, shared.progress().speed), (10_000, 0.0));
    shared.bytes(2, 20_000, true);
    std::thread::sleep(Duration::from_millis(300));
    shared.bytes(2, 40_000, true);
    let p = shared.progress();
    assert!(p.speed > 0.0 && p.eta_seconds.is_some(), "{p:?}");
    assert_eq!(p.bytes_done, 40_000);
    // A step that is done counts whole; one that was left out counts for nothing.
    shared.set(2, StepState::Done, "");
    shared.set(3, StepState::Skipped, "needs a Hugging Face sign-in");
    let p = shared.progress();
    assert_eq!(p.bytes_done, plan.steps[2].bytes);
    assert_eq!(p.bytes_total, all - plan.steps[3].bytes);
    // Rows that don't exist are ignored.
    shared.bytes(99, 1, true);
    shared.set(99, StepState::Done, "");
    shared.note(99, "x");
    assert_eq!(shared.state_of(99), None);
}
