//! The installed engine: how it is started, where it finds the models, and how it is removed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use photocraft_aigen::Launch;
use photocraft_aigen::launch::join_args;
use serde::{Deserialize, Serialize};

use crate::manifest::{FOLDERS, LEGACY_FOLDERS, Manifest, split_key};
use crate::plan::{DATA_FOLDER, Layout, MARKER, PART};
pub use crate::plan::{has_model_folders, models_root};
pub use crate::platform::RuntimePlatform;
use crate::{Error, Result};

/// The first port tried for the engine. Not 8188 (the portable build's default) and not 8000
/// (ComfyUI Desktop's), so nothing collides with a ComfyUI the user installs themselves.
pub const FIRST_PORT: u16 = 8190;

/// The name a repair gives an engine it moves out of the way while its replacement unpacks:
/// the engine's folder with `.replaced` added. It still carries the marker, but it is not an
/// installed engine where one stands; it goes back if the new one fails to arrive, and a job's
/// start puts it right either way (see `install::recover_replaced`).
pub const REPLACED: &str = ".replaced";

/// The current marker format version.
const MARKER_FORMAT_VERSION: u32 = 1;

/// What the marker file in an engine folder records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Marker {
    /// The marker schema, distinct from the ComfyUI version.
    pub format_version: u32,
    /// The runtime family this directory contains.
    pub platform: RuntimePlatform,
    /// The ComfyUI version.
    pub version: String,
    /// The build (`nvidia`, …) and the sha256 of the archive it was unpacked from.
    pub variant: String,
    pub sha256: String,
}

impl Marker {
    pub fn managed(platform: RuntimePlatform, version: &str, variant: &str, sha256: &str) -> Marker {
        Marker { format_version: MARKER_FORMAT_VERSION, platform, version: version.to_string(), variant: variant.to_string(), sha256: sha256.to_string() }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyMarker {
    version: String,
    variant: String,
    sha256: String,
}

fn is_valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn valid_marker(marker: &Marker) -> bool {
    marker.format_version == MARKER_FORMAT_VERSION
        && !marker.version.trim().is_empty()
        && marker.platform.accepts_variant(&marker.variant)
        && is_valid_sha256(&marker.sha256)
}

fn disk(path: &Path, what: &str, e: &std::io::Error) -> Error {
    Error::Disk(format!("Could not {what} `{}`: {e}", path.display()))
}

/// Write the marker into `dir` (the staging folder, before it becomes the engine).
pub fn write_marker(dir: &Path, marker: &Marker) -> Result<()> {
    if !valid_marker(marker) {
        return Err(Error::Config("refusing to write an invalid managed-runtime marker".into()));
    }
    let text = serde_json::to_string_pretty(&marker).map_err(|e| Error::Disk(e.to_string()))?;
    let path = dir.join(MARKER);
    std::fs::write(&path, text).map_err(|e| disk(&path, "write", &e))
}

/// The marker of the engine in `root`, when it is one this app installed.
pub fn read_marker(root: &Path) -> Option<Marker> {
    let path = root.join(MARKER);
    if std::fs::metadata(&path).ok()?.len() > 64 * 1024 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if !value.is_object() {
        return None;
    }
    if value.get("formatVersion").is_some() {
        let marker: Marker = serde_json::from_value(value).ok()?;
        return valid_marker(&marker).then_some(marker);
    }
    let legacy: LegacyMarker = serde_json::from_value(value).ok()?;
    let marker = Marker {
        format_version: MARKER_FORMAT_VERSION,
        platform: RuntimePlatform::WindowsPortable,
        version: legacy.version,
        variant: legacy.variant,
        sha256: legacy.sha256,
    };
    valid_marker(&marker).then_some(marker)
}

/// A YAML double-quoted scalar: any path survives it, whatever characters it has.
fn yaml_quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// ComfyUI's "extra model paths" file for `models_dir`: every model folder of it, searched
/// first (`is_default`), so what the wizard downloads is what the engine lists. Then the user's
/// own models folders (`extra`), read after it, with the older folder names they may use.
pub fn model_paths_yaml(models_dir: &Path, extra: &[PathBuf]) -> String {
    let mut out = String::from("# Written by the Local AI setup: where this engine finds its models.\nlayershop:\n");
    out.push_str(&format!("    base_path: {}\n    is_default: true\n", yaml_quoted(&models_dir.to_string_lossy())));
    for folder in FOLDERS {
        out.push_str(&format!("    {folder}: {folder}\n"));
    }
    for (i, dir) in extra.iter().enumerate() {
        out.push_str(&format!("yours{}:\n    base_path: {}\n", i + 1, yaml_quoted(&dir.to_string_lossy())));
        for folder in FOLDERS.iter().copied().chain(LEGACY_FOLDERS.iter().map(|(_, old)| *old)) {
            out.push_str(&format!("    {folder}: {folder}\n"));
        }
    }
    out
}

/// Write the model paths file, and make the folders it names in the setup's own models folder
/// (ComfyUI lists a missing one as empty, but a download into it should not have to create it
/// under a running server). Nothing is made in the user's own folders.
pub fn write_model_paths(layout: &Layout) -> Result<()> {
    for folder in FOLDERS {
        let dir = layout.models_dir.join(folder);
        std::fs::create_dir_all(&dir).map_err(|e| disk(&dir, "create the folder", &e))?;
    }
    std::fs::write(&layout.model_paths, model_paths_yaml(&layout.models_dir, &layout.extra_models)).map_err(|e| disk(&layout.model_paths, "write", &e))
}

/// The file types ComfyUI lists as models (its `supported_pt_extensions`). Others in a models
/// folder (`.gguf`, which needs a custom node) are not offered.
const MODEL_EXTENSIONS: [&str; 8] = ["safetensors", "sft", "ckpt", "pt", "pt2", "pth", "bin", "pkl"];

/// The model files in one of the user's own models folders, by folder, named as the engine lists
/// them: in subfolders too (`sub\name` on Windows), with what an older folder name holds
/// (`unet`) listed under the folder it stands for, and only the file types ComfyUI loads.
pub fn list_linked_models(root: &Path) -> BTreeMap<String, Vec<String>> {
    const DEPTH: usize = 8;
    const MAX_FILES: usize = 20_000;
    let mut out = BTreeMap::new();
    for folder in FOLDERS {
        let older = LEGACY_FOLDERS.iter().filter(|(now, _)| *now == folder).map(|(_, old)| *old);
        let mut names: Vec<String> = Vec::new();
        for source in std::iter::once(folder).chain(older) {
            // Folders to read, each with the name it adds in front of what is in it.
            let mut stack = vec![(root.join(source), String::new(), 0usize)];
            // Bounded: a models folder has tens of subfolders, not thousands.
            let mut visited = 0usize;
            while let Some((dir, prefix, depth)) = stack.pop() {
                visited += 1;
                if visited > 5_000 {
                    break;
                }
                let Ok(entries) = std::fs::read_dir(&dir) else { continue };
                for entry in entries.flatten().take(MAX_FILES) {
                    let Ok(name) = entry.file_name().into_string() else { continue };
                    // Followed through links, as ComfyUI does.
                    let Ok(meta) = std::fs::metadata(entry.path()) else { continue };
                    let shown = if prefix.is_empty() { name.clone() } else { format!("{prefix}{}{name}", std::path::MAIN_SEPARATOR) };
                    if meta.is_dir() {
                        if depth < DEPTH && name != ".git" {
                            stack.push((entry.path(), shown, depth + 1));
                        }
                    } else if name.rsplit_once('.').is_some_and(|(_, ext)| MODEL_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
                        && names.len() < MAX_FILES
                        && !names.contains(&shown)
                    {
                        names.push(shown);
                    }
                }
            }
        }
        names.sort();
        if !names.is_empty() {
            out.insert(folder.to_string(), names);
        }
    }
    out
}

/// The model files the installed engine has, by folder, as it lists them: those in the setup's
/// own models folder, then those in the user's own folders (`extra`) that it doesn't have.
pub fn list_all_models(models_dir: &Path, extra: &[PathBuf]) -> BTreeMap<String, Vec<String>> {
    let mut out = list_models(models_dir);
    for root in extra {
        for (folder, names) in list_linked_models(root) {
            let have = out.entry(folder).or_default();
            for name in names {
                if !have.contains(&name) {
                    have.push(name);
                }
            }
        }
    }
    for names in out.values_mut() {
        names.sort();
    }
    out
}

/// How the installed engine is started: the portable build's own command line (its
/// `run_nvidia_gpu.bat`), without the browser it would open, with the models folder added.
/// `cpu`: on the processor, as its `run_cpu.bat` does, for a PC without a graphics card it can use.
pub fn launch(manifest: &Manifest, layout: &Layout, cpu: bool) -> Launch {
    launch_for(RuntimePlatform::WindowsPortable, manifest, layout, cpu)
}

/// The managed macOS engine's writable folders, pointed at [`Layout::data_dir`]: each flag is
/// [`MACOS_DATA_ARGS`] at the same position of [`crate::plan::DATA_FOLDERS`], so the two lists
/// read as one. Together with [`MACOS_SERVER_ARGS`] and [`MACOS_LAUNCH_ENV`] these are every
/// macOS-only addition to the launch; check them against ComfyUI 0.43.0's `comfy/cli_args.py`
/// (the four directory flags, `--listen`, `--disable-api-nodes`) and the Hugging Face Hub and
/// generic telemetry conventions (the environment) when the bundled ComfyUI version changes:
/// an unknown flag makes ComfyUI refuse to start.
pub const MACOS_DATA_ARGS: [&str; 4] = ["--user-directory", "--input-directory", "--output-directory", "--temp-directory"];
/// Keep the managed macOS engine on the loopback interface only, and away from node code it
/// would fetch itself (the wizard downloads everything; the server downloads nothing).
pub const MACOS_SERVER_ARGS: [&str; 3] = ["--listen", "127.0.0.1", "--disable-api-nodes"];
/// Environment of the managed macOS launch: no Hugging Face round-trips, and no telemetry.
pub const MACOS_LAUNCH_ENV: [(&str, &str); 4] =
    [("HF_HUB_OFFLINE", "1"), ("TRANSFORMERS_OFFLINE", "1"), ("HF_HUB_DISABLE_TELEMETRY", "1"), ("DO_NOT_TRACK", "1")];

/// Make the managed engine's writable folders ([`Layout::data_dirs`]). Idempotent: folders that
/// are already there are left as they are, so Repair and Continue re-run it. Nothing is ever
/// made inside a versioned engine folder.
pub fn create_data_dirs(layout: &Layout) -> Result<()> {
    for dir in layout.data_dirs() {
        std::fs::create_dir_all(&dir).map_err(|e| disk(&dir, "create the folder", &e))?;
    }
    Ok(())
}

/// How a managed runtime for `platform` is started. The older [`launch`] entry point remains the
/// Windows default for callers that have no explicit managed choice.
pub fn launch_for(platform: RuntimePlatform, manifest: &Manifest, layout: &Layout, cpu: bool) -> Launch {
    build_launch_command(platform, manifest, layout, cpu)
}

/// Build the ComfyUI launch command for a given platform. This is a pure function that accepts
/// the runtime platform explicitly (for future macOS support) and constructs the command line.
pub fn build_launch_command(platform: RuntimePlatform, _manifest: &Manifest, layout: &Layout, cpu: bool) -> Launch {
    let script = layout.script.strip_prefix(&layout.engine_root).unwrap_or(&layout.script).to_string_lossy().into_owned();
    let mut args = vec![
        // Python's `-s`: the user's own site-packages stay out of the engine.
        "-s".to_string(),
        script,
    ];
    // Windows portable builds need this flag; macOS managed runtime does not.
    args.extend(platform.extra_args().iter().map(|arg| (*arg).to_string()));
    args.push("--disable-auto-launch".into());
    args.push("--extra-model-paths-config".into());
    args.push(layout.model_paths.to_string_lossy().into_owned());
    let mut env = BTreeMap::new();
    if platform == RuntimePlatform::MacosArm64 {
        // The managed macOS engine keeps everything ComfyUI writes in the data folder beside the
        // engine folder, not inside it; and stays local, offline and untelemetered.
        for (flag, dir) in MACOS_DATA_ARGS.iter().zip(layout.data_dirs()) {
            args.push((*flag).to_string());
            args.push(dir.to_string_lossy().into_owned());
        }
        args.extend(MACOS_SERVER_ARGS.iter().map(|arg| (*arg).to_string()));
        env.extend(MACOS_LAUNCH_ENV.iter().map(|(key, value)| ((*key).to_string(), (*value).to_string())));
    }
    if cpu {
        args.push("--cpu".into());
    }
    Launch { python: layout.python.to_string_lossy().into_owned(), folder: layout.engine_root.to_string_lossy().into_owned(), args: join_args(&args), env }
}

/// The first port from `from` up that nothing on this computer listens on.
pub fn free_port(from: u16) -> Option<u16> {
    (from..from.saturating_add(200)).find(|port| std::net::TcpListener::bind(("127.0.0.1", *port)).is_ok())
}

/// The address the engine answers at on `port`.
pub fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// The model files in `models_dir`, by folder, as a ComfyUI server pointed at it lists them.
/// Parts of unfinished downloads are not files yet.
pub fn list_models(models_dir: &Path) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for folder in FOLDERS {
        let Ok(entries) = std::fs::read_dir(models_dir.join(folder)) else { continue };
        let mut names: Vec<String> = entries
            .flatten()
            .take(20_000)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| !n.ends_with(PART) && !n.ends_with(".etag"))
            .collect();
        names.sort();
        if !names.is_empty() {
            out.insert(folder.to_string(), names);
        }
    }
    out
}

/// The bytes under `path` (a file, or a folder and everything in it). Links are not followed.
pub fn size_of(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    // Bounded: an engine is under a hundred thousand folders.
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > 2_000_000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(t) if t.is_file() => total = total.saturating_add(entry.metadata().map_or(0, |m| m.len())),
                _ => {}
            }
        }
    }
    total
}

/// The ComfyUI versions installed in `engine_dir`: the folders that carry this app's marker.
/// An engine set aside for a repair ([`REPLACED`]) is not one of them, marker or not.
pub fn installed_engines(engine_dir: &Path) -> Vec<(PathBuf, Marker)> {
    let Ok(entries) = std::fs::read_dir(engine_dir) else { return Vec::new() };
    let mut out: Vec<(PathBuf, Marker)> = entries
        .flatten()
        .take(1000)
        .filter(|e| !e.file_name().to_string_lossy().ends_with(REPLACED))
        .filter_map(|e| read_marker(&e.path()).map(|m| (e.path(), m)))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Remove what the setup put in `engine_dir`: every engine that carries the marker, unfinished
/// unpacks, engines set aside for a repair ([`REPLACED`]) while they still carry the marker, and
/// runtime archives. Anything else in the folder is not this app's and stays.
/// The data folder of writable engine output ([`DATA_FOLDER`]) stays too — like the models —
/// unless `remove_data` says the user chose to delete their workflows and output with it.
/// Returns the bytes freed.
pub fn remove_engines(engine_dir: &Path, remove_data: bool) -> Result<u64> {
    let mut freed = 0u64;
    let Ok(entries) = std::fs::read_dir(engine_dir) else { return Ok(0) };
    for entry in entries.flatten().take(10_000) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() && name == DATA_FOLDER {
            // Not a versioned engine: the user's workflows, uploads and output, kept unless the
            // user asked for this folder too.
            if remove_data {
                let size = size_of(&path);
                std::fs::remove_dir_all(&path).map_err(|e| disk(&path, "remove", &e))?;
                freed = freed.saturating_add(size);
            }
        } else if kind.is_dir() && (read_marker(&path).is_some() || name.ends_with(".unpacking")) {
            // No name rule for [`REPLACED`] folders: a set-aside engine is always a moved engine
            // folder, so it carries the marker and the first test takes it; a `*.replaced` folder
            // without one is not this app's and must stay.
            let size = size_of(&path);
            std::fs::remove_dir_all(&path).map_err(|e| disk(&path, "remove", &e))?;
            freed = freed.saturating_add(size);
        } else if kind.is_file()
            && name.starts_with("ComfyUI_windows_portable")
            && (name.ends_with(".7z") || name.ends_with(".7z.part") || name.ends_with(".7z.part.etag"))
        {
            let size = entry.metadata().map_or(0, |m| m.len());
            std::fs::remove_file(&path).map_err(|e| disk(&path, "remove", &e))?;
            freed = freed.saturating_add(size);
        }
    }
    // The folder itself, when nothing else was in it: it is not when the data folder stayed.
    let _ = std::fs::remove_dir(engine_dir);
    Ok(freed)
}

/// Remove the model files of `ids` from `models_dir`, keeping every file a model in `keep` still
/// uses. Only files the manifest lists are ever touched (and their unfinished parts): what the
/// user put in the folder themselves stays. Returns the bytes freed.
pub fn remove_models(manifest: &Manifest, models_dir: &Path, ids: &[String], keep: &[String]) -> Result<u64> {
    let kept: Vec<&str> = manifest.file_keys(manifest.models.iter().filter(|m| keep.contains(&m.id) && !ids.contains(&m.id)));
    let mut freed = 0u64;
    for key in manifest.file_keys(manifest.models.iter().filter(|m| ids.contains(&m.id))) {
        if kept.contains(&key) {
            continue;
        }
        let Some((folder, name)) = split_key(key) else { continue };
        let file = models_dir.join(folder).join(name);
        for path in [file.clone(), crate::plan::part_of(&file), PathBuf::from(format!("{}{PART}.etag", file.display()))] {
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if !meta.is_file() {
                continue;
            }
            std::fs::remove_file(&path).map_err(|e| disk(&path, "remove", &e))?;
            freed = freed.saturating_add(meta.len());
        }
    }
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Choices, MODEL_PATHS, same_folder};
    use photocraft_aigen::launch::split_args;

    fn manifest() -> &'static Manifest {
        Manifest::bundled().unwrap()
    }

    fn layout(dir: &Path) -> Layout {
        let choices = Choices {
            engine_dir: dir.join("engine dir").to_string_lossy().into_owned(),
            models_dir: dir.join("my models").to_string_lossy().into_owned(),
            variant: "nvidia".into(),
            ..Choices::default()
        };
        Layout::new(manifest(), &choices).unwrap()
    }

    #[test]
    fn the_engine_starts_as_the_portable_build_does_without_a_browser() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(dir.path());
        let l = launch(manifest(), &layout, false);
        assert_eq!(l.python, layout.python.to_string_lossy());
        assert_eq!(l.folder, layout.engine_root.to_string_lossy());
        // The folder has a space in it: the arguments still split back into what was meant.
        let args = split_args(&l.args);
        let script = format!("ComfyUI{}main.py", std::path::MAIN_SEPARATOR);
        assert_eq!(args[..5], ["-s", script.as_str(), "--windows-standalone-build", "--disable-auto-launch", "--extra-model-paths-config"]);
        assert_eq!(args[5], layout.model_paths.to_string_lossy());
        assert!(args[5].ends_with(MODEL_PATHS) && !l.args.contains("--port") && !l.args.contains("--cpu"));
        // Without a graphics card it can use, the engine runs on the processor.
        let cpu = split_args(&launch(manifest(), &layout, true).args);
        assert_eq!((cpu.len(), cpu.last().map(String::as_str)), (args.len() + 1, Some("--cpu")));
    }

    #[test]
    fn the_model_paths_file_survives_any_folder_name() {
        let yaml = model_paths_yaml(Path::new("D:\\My \"AI\" models\\ünï: # stuff"), &[]);
        assert!(yaml.contains("    base_path: \"D:\\\\My \\\"AI\\\" models\\\\ünï: # stuff\"\n"), "{yaml}");
        assert!(yaml.contains("    is_default: true\n") && yaml.starts_with("# "));
        for folder in FOLDERS {
            assert!(yaml.contains(&format!("    {folder}: {folder}\n")));
        }
        assert_eq!(yaml_quoted("a\nb\u{7}"), "\"a\\u000Ab\\u0007\"");
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(dir.path());
        std::fs::create_dir_all(&layout.engine_root).unwrap();
        write_model_paths(&layout).unwrap();
        assert!(layout.model_paths.is_file() && layout.models_dir.join("text_encoders").is_dir());
    }

    #[test]
    fn the_users_own_models_folders_are_read_after_the_setups_and_never_made() {
        let dir = tempfile::tempdir().unwrap();
        let mut layout = layout(dir.path());
        let (mine, theirs) = (dir.path().join("ComfyUIModels").join("models"), dir.path().join("old drive"));
        layout.extra_models = vec![mine.clone(), theirs.clone()];
        std::fs::create_dir_all(&layout.engine_root).unwrap();
        std::fs::create_dir_all(mine.join("loras")).unwrap();
        write_model_paths(&layout).unwrap();
        let yaml = std::fs::read_to_string(&layout.model_paths).unwrap();
        // Only one folder is where ComfyUI saves: the setup's own.
        assert_eq!(yaml.matches("is_default").count(), 1, "{yaml}");
        assert!(yaml.contains(&format!("yours1:\n    base_path: {}\n", yaml_quoted(&mine.to_string_lossy()))), "{yaml}");
        assert!(yaml.contains("yours2:") && yaml.contains("    unet: unet\n") && yaml.contains("    clip: clip\n"), "{yaml}");
        assert!(!theirs.exists() && !mine.join("vae").exists(), "nothing is made in the user's folders");
        // The app reads the file the way ComfyUI does: its own folder first, then the user's.
        let launch = launch(manifest(), &layout, false);
        let found = photocraft_aigen::launch::models_folders(&launch, &|p| std::fs::read_to_string(p).ok(), &|p| p.is_dir());
        let at = |p: &Path| found.iter().position(|f| same_folder(Path::new(f), p));
        assert!(at(&layout.models_dir) < at(&mine) && at(&mine).is_some(), "{found:?}");
    }

    #[test]
    fn a_linked_folder_lists_what_comfyui_would() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for file in [
            "loras/style.safetensors",
            "loras/qwen/lightning.safetensors",
            "loras/notes.txt",
            "unet/old_style.safetensors",
            "diffusion_models/z_image_turbo_bf16.safetensors",
            "diffusion_models/ltx-q4.gguf",
            "clip/t5.SAFETENSORS",
            "loras/.git/objects/x.pt",
        ] {
            let path = file.split('/').fold(root.to_path_buf(), |p, part| p.join(part));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
        let sub = format!("qwen{}lightning.safetensors", std::path::MAIN_SEPARATOR);
        let listed = list_linked_models(root);
        assert_eq!(listed["loras"], vec![sub, "style.safetensors".to_string()]);
        assert_eq!(listed["diffusion_models"], vec!["old_style.safetensors".to_string(), "z_image_turbo_bf16.safetensors".to_string()]);
        assert_eq!(listed["text_encoders"], vec!["t5.SAFETENSORS".to_string()]);
        assert!(!listed.contains_key("vae") && list_linked_models(&root.join("missing")).is_empty());

        // Beside the setup's own folder: each name once.
        let own = dir.path().join("own");
        std::fs::create_dir_all(own.join("loras")).unwrap();
        std::fs::write(own.join("loras").join("style.safetensors"), b"x").unwrap();
        std::fs::write(own.join("loras").join("mine.safetensors"), b"x").unwrap();
        let all = list_all_models(&own, &[root.to_path_buf()]);
        assert_eq!(all["loras"].len(), 3, "{all:?}");
        assert!(all["loras"].contains(&"mine.safetensors".to_string()));
    }

    #[test]
    fn a_comfyui_folder_stands_for_the_models_folder_in_it() {
        let dir = tempfile::tempdir().unwrap();
        let comfy = dir.path().join("ComfyUI");
        std::fs::create_dir_all(comfy.join("models").join("checkpoints")).unwrap();
        assert_eq!(models_root(&comfy), comfy.join("models"));
        assert_eq!(models_root(&comfy.join("models")), comfy.join("models"));
        // Nothing to go by: what was picked.
        assert_eq!(models_root(dir.path()), dir.path().to_path_buf());
        assert!(has_model_folders(&comfy.join("models")) && !has_model_folders(&comfy));
    }

    #[test]
    fn a_free_port_is_one_nothing_listens_on() {
        let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = taken.local_addr().unwrap().port();
        let next = free_port(port).unwrap();
        assert!(next > port, "{next} vs {port}");
        assert_eq!(url(8190), "http://127.0.0.1:8190");
        assert!(free_port(u16::MAX).is_none() || free_port(u16::MAX) == Some(u16::MAX));
    }

    #[test]
    fn the_models_folder_is_listed_as_a_server_would() {
        let dir = tempfile::tempdir().unwrap();
        let vae = dir.path().join("vae");
        std::fs::create_dir_all(vae.join("sub")).unwrap();
        for name in ["b.safetensors", "a.safetensors", "c.safetensors.part", "c.safetensors.part.etag"] {
            std::fs::write(vae.join(name), b"x").unwrap();
        }
        std::fs::create_dir_all(dir.path().join("loras")).unwrap();
        std::fs::create_dir_all(dir.path().join("not_a_model_folder")).unwrap();
        let listed = list_models(dir.path());
        assert_eq!(listed, BTreeMap::from([("vae".to_string(), vec!["a.safetensors".to_string(), "b.safetensors".to_string()])]));
        assert!(list_models(&dir.path().join("missing")).is_empty());
        assert_eq!(size_of(&vae), 4);
        assert_eq!((size_of(&vae.join("a.safetensors")), size_of(&dir.path().join("missing"))), (1, 0));
    }
    #[test]
    fn current_marker_round_trips_with_version_and_platform() {
        let dir = tempfile::tempdir().unwrap();
        let engine = dir.path().join("engine");
        std::fs::create_dir_all(&engine).unwrap();
        let marker = Marker::managed(RuntimePlatform::MacosArm64, "0.43.0", "macos_arm64", &"a".repeat(64));
        write_marker(&engine, &marker).unwrap();
        let text = std::fs::read_to_string(engine.join(MARKER)).unwrap();
        assert!(text.contains("\"formatVersion\": 1") && text.contains("\"platform\": \"macosArm64\""));
        assert_eq!(read_marker(&engine), Some(marker));
    }

    #[test]
    fn legacy_windows_marker_migrates_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let engine = dir.path().join("engine");
        std::fs::create_dir_all(&engine).unwrap();
        let legacy = serde_json::json!({
            "version": "0.39.0",
            "variant": "nvidia",
            "sha256": "a".repeat(64),
        });
        std::fs::write(engine.join(MARKER), serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(read_marker(&engine), Some(Marker::managed(RuntimePlatform::WindowsPortable, "0.39.0", "nvidia", &"a".repeat(64))));
    }

    #[test]
    fn ambiguous_or_hostile_markers_are_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let engine = dir.path().join("engine");
        std::fs::create_dir_all(&engine).unwrap();
        let marker = engine.join(MARKER);
        let valid = |changes: serde_json::Value| {
            let mut value = serde_json::json!({
                "formatVersion": 1,
                "platform": "windowsPortable",
                "version": "0.43.0",
                "variant": "nvidia",
                "sha256": "a".repeat(64),
            });
            for (key, replacement) in changes.as_object().unwrap() {
                value[key] = replacement.clone();
            }
            value
        };
        for value in [
            serde_json::json!({}),
            valid(serde_json::json!({"formatVersion": 2})),
            valid(serde_json::json!({"platform": "linuxX64"})),
            valid(serde_json::json!({"version": ""})),
            valid(serde_json::json!({"variant": "voodoo"})),
            valid(serde_json::json!({"sha256": "ab"})),
        ] {
            std::fs::write(&marker, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(read_marker(&engine).is_none(), "{value}");
        }
        std::fs::write(&marker, vec![b' '; 64 * 1024 + 1]).unwrap();
        assert!(read_marker(&engine).is_none());
    }

    #[test]
    fn installed_engines_reads_only_trusted_markers() {
        let dir = tempfile::tempdir().unwrap();
        let engines = dir.path().join("engine");
        let trusted = engines.join("0.39.0");
        let notes = engines.join("notes");
        std::fs::create_dir_all(&trusted).unwrap();
        std::fs::create_dir_all(&notes).unwrap();
        write_marker(&trusted, &Marker::managed(RuntimePlatform::WindowsPortable, "0.39.0", "nvidia", &"a".repeat(64))).unwrap();
        std::fs::write(notes.join(MARKER), b"{}").unwrap();
        // An engine set aside for a repair carries a valid marker but stands where a newer one
        // is (or will be): it is not one of the installed engines.
        let aside = engines.join("0.40.0.replaced");
        std::fs::create_dir_all(&aside).unwrap();
        write_marker(&aside, &Marker::managed(RuntimePlatform::WindowsPortable, "0.40.0", "nvidia", &"b".repeat(64))).unwrap();
        let installed = installed_engines(&engines);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].0, trusted);
    }

    #[test]
    fn removing_takes_only_what_the_setup_installed() {
        let dir = tempfile::tempdir().unwrap();
        let engines = dir.path().join("engine");
        let ours = engines.join("0.39.0");
        let theirs = engines.join("my notes");
        std::fs::create_dir_all(ours.join("python_embeded")).unwrap();
        std::fs::create_dir_all(&theirs).unwrap();
        std::fs::write(ours.join("python_embeded").join("python.exe"), vec![0u8; 1000]).unwrap();
        write_marker(&ours, &Marker::managed(RuntimePlatform::WindowsPortable, "0.39.0", "nvidia", &"a".repeat(64))).unwrap();
        std::fs::create_dir_all(engines.join("0.40.0.unpacking")).unwrap();
        // An engine a repair set aside: not an installed engine, but the setup's to remove.
        std::fs::create_dir_all(engines.join("0.41.0.replaced")).unwrap();
        write_marker(engines.join("0.41.0.replaced").as_path(), &Marker::managed(RuntimePlatform::WindowsPortable, "0.41.0", "nvidia", &"b".repeat(64)))
            .unwrap();
        std::fs::write(theirs.join("keep.txt"), b"mine").unwrap();
        std::fs::write(engines.join("ComfyUI_windows_portable_nvidia.7z.part"), vec![0u8; 500]).unwrap();
        std::fs::write(engines.join("holiday.7z"), b"mine too").unwrap();
        // The data folder of writable engine output, beside the versioned engines.
        std::fs::create_dir_all(engines.join(DATA_FOLDER).join("user")).unwrap();
        std::fs::write(engines.join(DATA_FOLDER).join("user").join("flow.json"), b"mine").unwrap();
        assert_eq!(installed_engines(&engines).iter().map(|(_, m)| m.version.as_str()).collect::<Vec<_>>(), ["0.39.0"]);
        let freed = remove_engines(&engines, false).unwrap();
        assert!(freed >= 1500, "{freed}");
        assert!(
            !ours.exists()
                && !engines.join("0.40.0.unpacking").exists()
                && !engines.join("0.41.0.replaced").exists()
                && !engines.join("ComfyUI_windows_portable_nvidia.7z.part").exists()
        );
        assert!(theirs.join("keep.txt").is_file() && engines.join("holiday.7z").is_file());
        // The user's workflows and output stay with the setup removed, like the models do.
        assert!(engines.join(DATA_FOLDER).join("user").join("flow.json").is_file());
        // Only `remove_data` takes the data folder; what never belonged to the app still stays.
        let freed = remove_engines(&engines, true).unwrap();
        assert!(freed >= 4, "{freed}");
        assert!(!engines.join(DATA_FOLDER).exists());
        assert!(theirs.join("keep.txt").is_file() && engines.join("holiday.7z").is_file());
        assert_eq!(remove_engines(&dir.path().join("missing"), true).unwrap(), 0);
        // A marker that is not one (a huge file, junk) marks nothing.
        std::fs::write(theirs.join(MARKER), b"not json").unwrap();
        assert!(read_marker(&theirs).is_none());
    }

    /// A set-aside engine is always a moved engine folder, so it carries the marker and goes with
    /// the setup; a folder that merely ends in `.replaced` without one is not this app's.
    #[test]
    fn a_replaced_folder_is_removed_only_when_it_carries_the_marker() {
        let dir = tempfile::tempdir().unwrap();
        let engines = dir.path().join("engine");
        let aside = engines.join("0.41.0.replaced");
        let theirs = engines.join("notes.replaced");
        std::fs::create_dir_all(&aside).unwrap();
        std::fs::create_dir_all(&theirs).unwrap();
        std::fs::write(theirs.join("keep.txt"), b"mine").unwrap();
        write_marker(&aside, &Marker::managed(RuntimePlatform::WindowsPortable, "0.41.0", "nvidia", &"b".repeat(64))).unwrap();
        let freed = remove_engines(&engines, false).unwrap();
        assert!(freed > 0, "{freed}");
        assert!(!aside.exists(), "the set-aside engine carries the marker, so the setup removes it");
        assert!(theirs.join("keep.txt").is_file(), "a `*.replaced` folder without the marker is not the setup's to delete");
    }

    #[test]
    fn removing_a_model_keeps_the_files_another_still_uses() {
        let m = manifest();
        let dir = tempfile::tempdir().unwrap();
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let installed = ids(&["qwen-image-2-1", "ideogram-4"]);
        for key in m.file_keys(m.select(&installed).unwrap()) {
            let (folder, name) = split_key(key).unwrap();
            std::fs::create_dir_all(dir.path().join(folder)).unwrap();
            std::fs::write(dir.path().join(folder).join(name), vec![0u8; 10]).unwrap();
        }
        std::fs::write(dir.path().join("vae").join("mine.safetensors"), b"mine").unwrap();
        std::fs::write(dir.path().join("diffusion_models").join("ideogram4_fp8_scaled.safetensors.part"), vec![0u8; 3]).unwrap();
        // Ideogram 4 goes; the text encoder it shares with Qwen Image 2.1 stays.
        let freed = remove_models(m, dir.path(), &ids(&["ideogram-4"]), &installed).unwrap();
        assert_eq!(freed, 33, "two diffusion models, a part, and the VAE only Ideogram used");
        assert!(dir.path().join("text_encoders").join("qwen3vl_8b_fp8_scaled.safetensors").is_file());
        assert!(!dir.path().join("vae").join("flux2-vae.safetensors").exists());
        assert!(dir.path().join("vae").join("mine.safetensors").is_file());
        // Removing everything listed still leaves the user's own file.
        remove_models(m, dir.path(), &installed, &[]).unwrap();
        assert_eq!(list_models(dir.path()), BTreeMap::from([("vae".to_string(), vec!["mine.safetensors".to_string()])]));
        assert_eq!(remove_models(m, &dir.path().join("missing"), &installed, &[]).unwrap(), 0);
    }

    #[test]
    fn build_launch_command_platforms_build_distinct_commands() {
        let dir = tempfile::tempdir().unwrap();
        let layout = layout(dir.path());
        let m = manifest();

        let script = m.comfy.script.replace('/', std::path::MAIN_SEPARATOR_STR);
        let model_paths = layout.model_paths.to_string_lossy().into_owned();

        // Windows portable remains the public default and keeps its standalone-build flag,
        // byte-for-byte: no new argument, and no environment of its own.
        let windows = launch(m, &layout, false);
        let windows_args = split_args(&windows.args);
        assert_eq!(
            windows_args,
            ["-s", script.as_str(), "--windows-standalone-build", "--disable-auto-launch", "--extra-model-paths-config", model_paths.as_str()]
        );
        assert!(windows.env.is_empty());

        // A future macOS asset's own paths reach launch construction without the Windows-only
        // flag. CPU mode adds exactly one explicit argument on either platform.
        let mut mac_layout = layout.clone();
        mac_layout.python = mac_layout.engine_root.join("python/bin/python3");
        mac_layout.script = mac_layout.engine_root.join("ComfyUI-macos/main.py");
        let launched = build_launch_command(RuntimePlatform::MacosArm64, m, &mac_layout, false);
        assert!(launched.python.ends_with("python/bin/python3"));
        // The writable folders, every flag once, each path outside the engine folder. The engine
        // folder has a space in it, so this also proves the paths survive join_args/split_args.
        let macos = split_args(&launched.args);
        let data = mac_layout.data_dir.clone();
        assert!(!data.starts_with(&mac_layout.engine_root));
        let mut want = vec![
            "-s".to_string(),
            "ComfyUI-macos/main.py".to_string(),
            "--disable-auto-launch".to_string(),
            "--extra-model-paths-config".to_string(),
            model_paths.clone(),
        ];
        for (flag, folder) in crate::runtime::MACOS_DATA_ARGS.iter().zip(crate::plan::DATA_FOLDERS) {
            let path = data.join(folder);
            assert!(!path.starts_with(&mac_layout.engine_root));
            want.extend([(*flag).to_string(), path.to_string_lossy().into_owned()]);
        }
        want.extend(crate::runtime::MACOS_SERVER_ARGS.iter().map(|a| (*a).to_string()));
        assert_eq!(macos, want, "{macos:?}");
        for flag in crate::runtime::MACOS_DATA_ARGS.iter().chain(crate::runtime::MACOS_SERVER_ARGS.iter()) {
            assert_eq!(macos.iter().filter(|a| a == flag).count(), 1, "{flag}");
        }
        assert!(!launched.args.contains("--windows-standalone-build") && !launched.args.contains("--port"));
        // Offline and untelemetered, managed macOS only.
        assert_eq!(launched.env, crate::runtime::MACOS_LAUNCH_ENV.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<BTreeMap<String, String>>());
        let macos_cpu = split_args(&build_launch_command(RuntimePlatform::MacosArm64, m, &mac_layout, true).args);
        assert_eq!((macos_cpu.len(), macos_cpu.last().map(String::as_str)), (macos.len() + 1, Some("--cpu")));
    }

    #[test]
    fn the_writable_folders_are_made_outside_the_engine_and_survive_re_running() {
        let dir = tempfile::tempdir().unwrap();
        let lay = layout(dir.path());
        create_data_dirs(&lay).unwrap();
        create_data_dirs(&lay).unwrap(); // idempotent
        for folder in crate::plan::DATA_FOLDERS {
            let made = lay.data_dir.join(folder);
            assert!(made.is_dir(), "{made:?}");
            assert!(!made.starts_with(&lay.engine_root), "nothing inside the engine folder");
        }
        // What the user made in them is left alone.
        std::fs::write(lay.data_dir.join("output").join("a.png"), b"x").unwrap();
        create_data_dirs(&lay).unwrap();
        assert!(lay.data_dir.join("output").join("a.png").is_file());
    }
}
