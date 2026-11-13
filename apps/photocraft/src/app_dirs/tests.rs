use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp(tag: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("photocraft-app-dirs-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn env_of(pairs: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let m: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), v.as_os_str().to_owned())).collect();
    move |k| m.get(k).cloned()
}

/// A fake user folder that every platform's convention resolves under.
fn fake_home(root: &Path) -> impl Fn(&str) -> Option<OsString> + use<> {
    let home = root.join("home");
    let appdata = root.join("appdata");
    env_of(&[("HOME", &home), ("APPDATA", &appdata)])
}

#[test]
fn no_marker_uses_the_platform_dir() {
    let root = temp("platform");
    let exe = root.join("bin");
    std::fs::create_dir_all(&exe).unwrap();
    let env = fake_home(&root);
    let d = resolve(&env, Some(&exe));
    assert_eq!(d.mode, Mode::Platform);
    assert!(d.warning.is_none());
    let dir = d.dir.unwrap();
    assert_eq!(Some(dir.clone()), platform_dir(&env));
    assert!(!dir.starts_with(&exe));
    assert!(!exe.join(portable_data_dir()).exists(), "no portable folder without a marker");
    // The convention per platform.
    if cfg!(target_os = "macos") {
        assert_eq!(dir, root.join("home/Library/Application Support/SilicaLyr"));
    } else if cfg!(windows) {
        assert_eq!(dir, root.join("appdata").join("LayerShop"));
    } else {
        assert_eq!(dir, root.join("home/.config/layershop"));
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// The Local AI folders follow the data folder: macOS got SilicaLyr's own and they moved
/// with it; Windows keeps `%LOCALAPPDATA%\LayerShop` and Linux the XDG config folder.
#[test]
fn the_local_ai_folders_sit_under_the_data_folder() {
    let root = temp("local-ai");
    let env = env_of(&[("HOME", &root.join("home")), ("APPDATA", &root.join("appdata")), ("LOCALAPPDATA", &root.join("local"))]);
    let d = resolve(&env, None);
    let dir = d.dir.unwrap();
    let (engine, models) = local_ai_dirs(&d, &env).unwrap();
    if cfg!(target_os = "macos") {
        assert_eq!(dir, root.join("home/Library/Application Support/SilicaLyr"));
        assert_eq!(engine, dir.join("LocalAI").join("engine"));
        assert_eq!(models, dir.join("LocalAI").join("models"));
    } else if cfg!(windows) {
        assert_eq!(dir, root.join("appdata").join("LayerShop"));
        assert_eq!(engine, root.join("local").join("LayerShop").join("engine"));
        assert_eq!(models, root.join("local").join("LayerShop").join("models"));
    } else {
        assert_eq!(dir, root.join("home/.config/layershop"));
        assert_eq!(engine, dir.join("LocalAI").join("engine"));
        assert_eq!(models, dir.join("LocalAI").join("models"));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn either_marker_keeps_everything_beside_the_exe() {
    for marker in PORTABLE_MARKERS {
        let root = temp("portable");
        let exe = root.join("PhotoCraft");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join(marker), b"").unwrap();
        let d = resolve(fake_home(&root), Some(&exe));
        assert_eq!(d.mode, Mode::Portable, "{marker}");
        assert!(d.warning.is_none());
        let dir = d.dir.unwrap();
        assert_eq!(dir, exe.join(portable_data_dir()));
        assert!(dir.is_dir(), "the data folder is created");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "the write probe is removed");
        // Every derived path sits under the exe dir.
        for sub in ["preferences.json", "Presets", "Recovery", crate::gpu_startup::MARKER_FILE] {
            assert!(dir.join(sub).starts_with(&exe));
        }
        assert!(!root.join("home").exists() && !root.join("appdata").exists(), "nothing written to the user folder");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn a_marker_folder_is_not_a_marker() {
    let root = temp("dir-marker");
    let exe = root.join("bin");
    std::fs::create_dir_all(exe.join("portable.txt")).unwrap();
    assert_eq!(resolve(fake_home(&root), Some(&exe)).mode, Mode::Platform);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unwritable_portable_folder_falls_back_with_a_warning() {
    let root = temp("unwritable");
    let exe = root.join("bin");
    std::fs::create_dir_all(&exe).unwrap();
    std::fs::write(exe.join("portable.txt"), b"").unwrap();
    // A file where the data folder should go: it can't be created, on every platform.
    std::fs::write(exe.join(portable_data_dir()), b"in the way").unwrap();
    let env = fake_home(&root);
    let d = resolve(&env, Some(&exe));
    assert_eq!(d.mode, Mode::Platform);
    assert_eq!(d.dir, platform_dir(&env));
    let w = d.warning.unwrap();
    assert!(w.contains("portable") && w.contains("isn't writable"), "{w}");
    // No user folder at all: still no panic, just no directory.
    let d = resolve(|_| None, Some(&exe));
    assert!(d.dir.is_none() && d.warning.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn override_wins_over_portable() {
    let root = temp("override");
    let exe = root.join("bin");
    std::fs::create_dir_all(&exe).unwrap();
    std::fs::write(exe.join("portable.txt"), b"").unwrap();
    let custom = root.join("custom");
    let d = resolve(env_of(&[("PHOTOCRAFT_CONFIG_DIR", &custom)]), Some(&exe));
    assert_eq!((d.mode, d.dir), (Mode::Override, Some(custom)));
    assert!(!exe.join(portable_data_dir()).exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// #5648: on Windows, autosaves go to the machine-local `%LOCALAPPDATA%`, not the roaming
/// profile; an override or portable folder keeps them inside itself.
#[test]
fn autosaves_stay_out_of_the_roaming_profile() {
    let root = temp("recovery-local");
    let settings = root.join("appdata").join("LayerShop");
    let local = root.join("local");
    let env = env_of(&[("LOCALAPPDATA", &local)]);
    let platform = DataDir { dir: Some(settings.clone()), mode: Mode::Platform, warning: None };
    let expected = if cfg!(windows) { local.join("LayerShop").join("Recovery") } else { settings.join("Recovery") };
    assert_eq!(resolve_recovery(&platform, &env), Some(expected));
    for mode in [Mode::Override, Mode::Portable] {
        let d = DataDir { dir: Some(settings.clone()), mode, warning: None };
        assert_eq!(resolve_recovery(&d, &env), Some(settings.join("Recovery")), "{mode:?}");
    }
    // No (or an empty) LOCALAPPDATA: beside the settings, as before.
    let empty = PathBuf::new();
    assert_eq!(resolve_recovery(&platform, env_of(&[("LOCALAPPDATA", &empty)])), Some(settings.join("Recovery")));
    assert_eq!(resolve_recovery(&platform, |_| None), Some(settings.join("Recovery")));
    let nowhere = DataDir { dir: None, mode: Mode::Platform, warning: None };
    assert_eq!(resolve_recovery(&nowhere, |_| None), None);
    let _ = std::fs::remove_dir_all(&root);
}

/// #6612: on Linux (and other XDG systems) autosaves go to `$XDG_STATE_HOME`, by default
/// `~/.local/state`, not the config folder; a relative `XDG_STATE_HOME` is ignored. Windows
/// and macOS don't read it, and an override or portable folder keeps them inside itself.
#[test]
fn autosaves_go_to_the_xdg_state_folder() {
    let root = temp("recovery-xdg");
    let settings = root.join("home").join(".config").join("layershop");
    let home = root.join("home");
    let state = root.join("state");
    let platform = DataDir { dir: Some(settings.clone()), mode: Mode::Platform, warning: None };
    let xdg = !cfg!(windows) && !cfg!(target_os = "macos");
    let expect = |xdg_path: PathBuf| Some(if xdg { xdg_path } else { settings.join("Recovery") });
    assert_eq!(resolve_recovery(&platform, env_of(&[("XDG_STATE_HOME", &state), ("HOME", &home)])), expect(state.join("layershop").join("Recovery")));
    let default = home.join(".local").join("state").join("layershop").join("Recovery");
    assert_eq!(resolve_recovery(&platform, env_of(&[("HOME", &home)])), expect(default.clone()));
    let relative = PathBuf::from("relative/state");
    assert_eq!(resolve_recovery(&platform, env_of(&[("XDG_STATE_HOME", &relative), ("HOME", &home)])), expect(default.clone()));
    let empty = PathBuf::new();
    assert_eq!(resolve_recovery(&platform, env_of(&[("XDG_STATE_HOME", &empty), ("HOME", &home)])), expect(default));
    // No home at all: beside the settings, as before.
    assert_eq!(resolve_recovery(&platform, |_| None), Some(settings.join("Recovery")));
    for mode in [Mode::Override, Mode::Portable] {
        let d = DataDir { dir: Some(settings.clone()), mode, warning: None };
        assert_eq!(resolve_recovery(&d, env_of(&[("XDG_STATE_HOME", &state), ("HOME", &home)])), Some(settings.join("Recovery")), "{mode:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// An autosave bundle and its sidecar, as `format::Autosaver` writes them.
fn autosave(dir: &Path, key: &str) {
    std::fs::create_dir_all(dir.join(format!("{key}.pcraft"))).unwrap();
    std::fs::write(dir.join(format!("{key}.pcraft")).join("manifest.json"), key).unwrap();
    std::fs::write(dir.join(format!("{key}.json")), key).unwrap();
}

#[test]
fn autosaves_from_the_old_folder_move_to_the_new_one() {
    let root = temp("recovery-move");
    let legacy = root.join("roaming").join("Recovery");
    let dir = root.join("local").join("LayerShop").join("Recovery");
    autosave(&legacy, "a");
    assert_eq!(adopt_legacy_recovery(&legacy, &dir), dir);
    assert!(!legacy.exists(), "the old folder is gone");
    assert_eq!(std::fs::read_to_string(dir.join("a.pcraft").join("manifest.json")).unwrap(), "a");
    assert!(dir.join("a.json").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn autosaves_merge_into_an_existing_folder_without_overwriting() {
    let root = temp("recovery-merge");
    let legacy = root.join("roaming").join("Recovery");
    let dir = root.join("local").join("Recovery");
    autosave(&legacy, "old");
    autosave(&legacy, "both");
    // A temp file of an interrupted write moves along, for the recovery store's sweep.
    let temp_file = ".old.json.4294967294-0.pcsave.tmp";
    std::fs::write(legacy.join(temp_file), b"partial").unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("both.json"), "newer").unwrap();
    assert_eq!(adopt_legacy_recovery(&legacy, &dir), dir);
    assert!(dir.join(temp_file).is_file() && !legacy.join(temp_file).exists());
    assert!(dir.join("old.pcraft").is_dir() && dir.join("old.json").is_file(), "a whole autosave moves");
    assert_eq!(std::fs::read_to_string(dir.join("both.json")).unwrap(), "newer", "never overwritten");
    assert!(!dir.join("both.pcraft").exists(), "a clashing autosave isn't split");
    assert!(legacy.join("both.pcraft").is_dir() && legacy.join("both.json").is_file(), "it stays behind whole");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn autosaves_that_cannot_move_keep_being_used_where_they_are() {
    let root = temp("recovery-stuck");
    let legacy = root.join("roaming").join("Recovery");
    autosave(&legacy, "a");
    // A file where the new folder's parent should be: nothing can be created there.
    std::fs::write(root.join("blocked"), b"in the way").unwrap();
    let dir = root.join("blocked").join("Recovery");
    assert_eq!(adopt_legacy_recovery(&legacy, &dir), legacy);
    assert!(legacy.join("a.json").is_file(), "nothing lost");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_empty_old_folder_is_removed_even_where_it_cannot_move() {
    let root = temp("recovery-empty");
    let legacy = root.join("roaming").join("Recovery");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(root.join("blocked"), b"in the way").unwrap();
    let dir = root.join("blocked").join("Recovery");
    assert_eq!(adopt_legacy_recovery(&legacy, &dir), dir, "nothing to keep listed");
    assert!(!legacy.exists(), "the empty old folder is gone");
    let _ = std::fs::remove_dir_all(&root);
}

/// Where a rename can't cross volumes (a roaming profile on a network share), an autosave
/// is copied whole, nested folders included, and only then removed.
#[test]
fn a_move_across_volumes_copies_then_removes() {
    let root = temp("recovery-copy");
    let legacy = root.join("roaming").join("Recovery");
    autosave(&legacy, "a");
    std::fs::create_dir_all(legacy.join("a.pcraft").join("tiles")).unwrap();
    std::fs::write(legacy.join("a.pcraft").join("tiles").join("t0.zst"), b"tile").unwrap();
    let dir = root.join("local").join("Recovery");
    std::fs::create_dir_all(root.join("local")).unwrap();
    move_across(&legacy, &dir).unwrap();
    assert!(!legacy.exists(), "the original is removed after the copy");
    assert_eq!(side_leftovers(&root.join("roaming")), 0, "nothing is left held aside");
    assert_eq!(std::fs::read_to_string(dir.join("a.pcraft").join("manifest.json")).unwrap(), "a");
    assert_eq!(std::fs::read(dir.join("a.pcraft").join("tiles").join("t0.zst")).unwrap(), b"tile");
    assert_eq!(std::fs::read_to_string(dir.join("a.json")).unwrap(), "a");
    // One file on its own, as the per-autosave moves do.
    move_across(&dir.join("a.json"), &root.join("a.json")).unwrap();
    assert!(!dir.join("a.json").exists() && root.join("a.json").is_file());
    assert_eq!(side_leftovers(&dir) + side_leftovers(&root), 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failed_copy_puts_the_original_back_and_leaves_no_partial_copy() {
    let root = temp("recovery-copy-fail");
    let legacy = root.join("Recovery");
    autosave(&legacy, "a");
    // Deeper than any bundle: the copy gives up part way.
    let mut deep = legacy.join("a.pcraft");
    for _ in 0..=MAX_COPY_DEPTH {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    let dir = root.join("Local");
    assert!(move_across(&legacy, &dir).is_err());
    assert!(!dir.exists(), "nothing is published");
    assert_eq!(side_leftovers(&root), 0, "no staged copy, nothing left held aside");
    assert!(legacy.join("a.json").is_file() && deep.is_dir(), "the original is back in place");
    // A target that already exists is never copied into or removed.
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("keep"), b"mine").unwrap();
    assert!(move_across(&legacy.join("a.json"), &dir).is_err());
    assert_eq!(std::fs::read(dir.join("keep")).unwrap(), b"mine");
    assert!(legacy.join("a.json").is_file());
    assert_eq!(side_leftovers(&root) + side_leftovers(&legacy), 0);
    let _ = std::fs::remove_dir_all(&root);
}

/// Staged (`.moving`) and held (`.held`) side paths left in `dir`.
fn side_leftovers(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| side_of(n, STAGED).is_some() || side_of(n, HELD).is_some()))
        .count()
}

/// The target appears after the copy is staged (another process got there first): publishing
/// fails, that process's target keeps its contents, the staged copy is removed and the
/// original is left as it was. Folder and file alike.
#[test]
fn a_target_that_appears_while_copying_is_never_replaced_or_removed() {
    let root = temp("recovery-race");
    let legacy = root.join("roaming").join("Recovery");
    autosave(&legacy, "a");
    let local = root.join("local");
    std::fs::create_dir_all(&local).unwrap();
    // A whole folder.
    let dir = local.join("Recovery");
    let staged = stage_copy(&legacy, &dir).unwrap();
    autosave(&dir, "theirs");
    let e = publish(&staged, &dir).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_to_string(dir.join("theirs.json")).unwrap(), "theirs", "their folder is intact");
    assert!(!dir.join("a.json").exists(), "nothing was copied into it");
    assert!(!staged.exists() && side_leftovers(&local) == 0, "only the staged copy is removed");
    assert!(legacy.join("a.json").is_file(), "the original is untouched");
    // One file.
    let to = dir.join("a.json");
    let staged = stage_copy(&legacy.join("a.json"), &to).unwrap();
    std::fs::write(&to, "theirs").unwrap();
    assert_eq!(publish(&staged, &to).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "theirs");
    assert!(!staged.exists() && legacy.join("a.json").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

/// A process holds the lock and is copying the old folder (held aside, copy staged) when a
/// second launch gives up waiting. The second launch must get the new folder, never the old
/// one, and what it writes there survives the first one finishing: the publish refuses the
/// now-existing folder and the original is put back, so both autosaves stay (#6412 review).
#[test]
fn a_launch_that_times_out_never_writes_where_a_move_removes() {
    let _spawns = crate::no_child_spawns();
    let root = temp("recovery-timeout");
    let legacy = root.join("roaming").join("Recovery");
    autosave(&legacy, "old");
    let local = root.join("local");
    std::fs::create_dir_all(&local).unwrap();
    let dir = local.join("Recovery");
    // First process: takes the lock, sets the original aside and stages its copy, then pauses.
    let first = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(local.join(MIGRATION_LOCK)).unwrap();
    first.lock().unwrap();
    let held = hold(&legacy).unwrap();
    let staged = stage_copy(&held, &dir).unwrap();
    // Second launch: times out and writes a newer autosave where it was told to.
    let chosen = adopt_legacy_recovery_waiting(&legacy, &dir, std::time::Duration::ZERO);
    assert_eq!(chosen, dir, "a launch that couldn't get the lock never uses the old folder");
    autosave(&chosen, "new");
    // First process resumes: publish, and on failure put the original back (as `move_across`).
    assert_eq!(publish(&staged, &dir).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    put_back(&held, &legacy);
    drop(first);
    assert_eq!(std::fs::read_to_string(dir.join("new.json")).unwrap(), "new", "the newer autosave survives");
    assert_eq!(std::fs::read_to_string(legacy.join("old.json")).unwrap(), "old", "the old one is back in place");
    assert_eq!(side_leftovers(&local) + side_leftovers(&root.join("roaming")), 0);
    // The next launch merges them.
    assert_eq!(adopt_legacy_recovery_waiting(&legacy, &dir, std::time::Duration::ZERO), dir);
    assert!(dir.join("old.json").is_file() && dir.join("new.json").is_file() && !legacy.exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// A writer that still uses the old path while it is being copied (an older PhotoCraft)
/// starts a new folder there; the move removes only the original it held aside.
#[test]
fn a_write_to_the_old_path_during_a_copy_is_not_removed() {
    let root = temp("recovery-old-writer");
    let legacy = root.join("roaming").join("Recovery");
    autosave(&legacy, "old");
    let local = root.join("local");
    std::fs::create_dir_all(&local).unwrap();
    let dir = local.join("Recovery");
    let held = hold(&legacy).unwrap();
    let staged = stage_copy(&held, &dir).unwrap();
    autosave(&legacy, "new");
    publish(&staged, &dir).unwrap();
    std::fs::remove_dir_all(&held).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("old.json")).unwrap(), "old");
    assert_eq!(std::fs::read_to_string(legacy.join("new.json")).unwrap(), "new", "the new write is untouched");
    let _ = std::fs::remove_dir_all(&root);
}

/// What a process killed mid-move left is tidied at the next move: a held original is put
/// back (it may be the only copy) and moved, a staged copy is removed; other dot-files and
/// look-alike names are not touched.
#[test]
fn a_killed_move_is_tidied_next_time() {
    let root = temp("recovery-stale");
    let roaming = root.join("roaming");
    let legacy = roaming.join("Recovery");
    autosave(&legacy, "a");
    let held = hold(&legacy).unwrap();
    assert!(!legacy.exists() && held.is_dir());
    let local = root.join("local");
    let dir = local.join("Recovery");
    let stale = local.join(".Recovery.4294967294-3.moving");
    autosave(&stale, "half");
    let kept = [local.join(".Recovery.moving"), local.join(".keep.1-x.moving"), local.join("notes.txt")];
    for k in &kept {
        std::fs::write(k, b"x").unwrap();
    }
    assert_eq!(adopt_legacy_recovery_waiting(&legacy, &dir, std::time::Duration::ZERO), dir);
    assert_eq!(std::fs::read_to_string(dir.join("a.json")).unwrap(), "a", "the held original was put back and moved");
    assert!(!stale.exists(), "the stale staged copy is gone");
    assert_eq!(side_leftovers(&roaming), 0);
    assert!(kept.iter().all(|k| k.is_file()), "only side names are touched");
    assert_eq!(side_of(".a.json.12-0.moving", STAGED), Some("a.json"));
    assert!(side_of("a.json.12-0.moving", STAGED).is_none() && side_of(".a.json.-0.moving", STAGED).is_none());
    assert!(side_of(".a.json.12-0.held", STAGED).is_none() && side_of(".a.json.12-0.held", HELD) == Some("a.json"));
    let _ = std::fs::remove_dir_all(&root);
}

/// While another process holds the move lock, a launch moves nothing and uses the new folder;
/// once the lock is released, the move happens.
#[test]
fn a_move_waits_for_another_process_holding_the_lock() {
    let _spawns = crate::no_child_spawns();
    let root = temp("recovery-lock");
    let legacy = root.join("roaming").join("Recovery");
    autosave(&legacy, "a");
    let dir = root.join("local").join("Recovery");
    std::fs::create_dir_all(root.join("local")).unwrap();
    let held = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(root.join("local").join(MIGRATION_LOCK)).unwrap();
    held.lock().unwrap();
    assert_eq!(adopt_legacy_recovery_waiting(&legacy, &dir, std::time::Duration::ZERO), dir);
    assert!(legacy.join("a.json").is_file() && !dir.exists(), "nothing moved under another's lock");
    drop(held);
    assert_eq!(adopt_legacy_recovery_waiting(&legacy, &dir, std::time::Duration::ZERO), dir);
    assert!(dir.join("a.json").is_file() && !legacy.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn no_old_folder_or_the_same_folder_changes_nothing() {
    let root = temp("recovery-noop");
    let dir = root.join("Recovery");
    assert_eq!(adopt_legacy_recovery(&root.join("missing"), &dir), dir);
    assert!(!dir.exists(), "nothing is created up front");
    autosave(&dir, "a");
    assert_eq!(adopt_legacy_recovery(&dir, &dir), dir);
    assert!(dir.join("a.json").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_exe_dir_or_env_never_panics() {
    let d = resolve(|_| None, None);
    assert_eq!((d.mode, d.dir), (Mode::Platform, None));
    let empty = PathBuf::new();
    let d = resolve(env_of(&[("PHOTOCRAFT_CONFIG_DIR", &empty), ("HOME", &empty), ("APPDATA", &empty)]), None);
    assert_eq!(d.dir, None, "empty variables are ignored");
}

#[cfg(unix)]
#[test]
fn the_write_probe_never_follows_an_entry_already_at_its_name() {
    let root = temp("probe-link");
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let victim = root.join("victim.txt");
    std::fs::write(&victim, b"keep me").unwrap();
    // Every name the probe tries, except the last, is a link to a file outside the folder.
    let pid = std::process::id();
    for attempt in 0..WRITE_PROBE_ATTEMPTS - 1 {
        std::os::unix::fs::symlink(&victim, data.join(format!(".write-test-{pid}-{attempt}"))).unwrap();
    }
    ensure_writable(&data).unwrap();
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep me");
    // The links are left alone, and the probe that was created is gone.
    assert!(std::fs::symlink_metadata(data.join(format!(".write-test-{pid}-0"))).unwrap().file_type().is_symlink());
    assert!(!data.join(format!(".write-test-{pid}-{}", WRITE_PROBE_ATTEMPTS - 1)).exists());
    // With every name taken the folder is reported unwritable, still without touching the file.
    std::os::unix::fs::symlink(&victim, data.join(format!(".write-test-{pid}-{}", WRITE_PROBE_ATTEMPTS - 1))).unwrap();
    assert!(ensure_writable(&data).is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep me");
    let _ = std::fs::remove_dir_all(&root);
}

/// The one-time macOS move of the data folder out of the `LayerShop` name this fork used
/// before the rebrand, on temporary folders.
#[cfg(target_os = "macos")]
fn record_in(dir: &Path, support: &Path) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let text = format!(
        "{{\"phase\": \"done\", \"version\": 1, \"ownership\": \"managed\", \"port\": 8188, \
         \"choices\": {{\"engineDir\": \"{dir}/LocalAI/engine\", \"modelsDir\": \"{dir}/LocalAI/models\", \
         \"extraModelsDirs\": [\"/Users/dev/Models\", \"{support}/LayerShopMore/models\"]}}, \
         \"nextRelease\": {{\"note\": \"a field this app does not know\"}}}}",
        dir = dir.display(),
        support = support.display()
    );
    std::fs::write(dir.join("ai-setup.json"), &text).unwrap();
    text
}

#[cfg(target_os = "macos")]
#[test]
fn an_old_data_folder_moves_and_its_record_points_at_the_new_one() {
    let root = temp("migration-old");
    let support = root.join("Library/Application Support");
    let old = support.join("LayerShop");
    let new = support.join("SilicaLyr");
    std::fs::create_dir_all(old.join("Recovery")).unwrap();
    std::fs::write(old.join("preferences.json"), b"settings").unwrap();
    record_in(&old, &support);
    assert_eq!(migrate_data_folder(&new, &old), Migrated::Renamed);
    assert!(!old.exists(), "the folder was renamed, never copied around");
    assert_eq!(std::fs::read(new.join("preferences.json")).unwrap(), b"settings", "everything moved with it");
    assert!(new.join("Recovery").is_dir());
    let record: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(new.join("ai-setup.json")).unwrap()).unwrap();
    let choices = &record["choices"];
    assert_eq!(choices["engineDir"].as_str(), Some(new.join("LocalAI/engine").to_str().unwrap()));
    assert_eq!(choices["modelsDir"].as_str(), Some(new.join("LocalAI/models").to_str().unwrap()));
    // A models folder the user chose outside the data folder keeps its path, and so does one
    // whose name only starts with the old folder's text.
    assert_eq!(choices["extraModelsDirs"][0].as_str(), Some("/Users/dev/Models"));
    assert_eq!(choices["extraModelsDirs"][1].as_str(), Some(format!("{}/LayerShopMore/models", support.display()).as_str()));
    // Everything the app does not know, from this or a newer version, survives the rewrite.
    assert_eq!(record["nextRelease"]["note"].as_str(), Some("a field this app does not know"));
    assert_eq!(record["version"], 1);
    assert_eq!(record["port"], 8188);
    assert!(!new.join("ai-setup.json.new").exists(), "the rewrite leaves no temp file");
    let _ = std::fs::remove_dir_all(&root);
}

/// The move rewrites the preferences' Local AI launch paths too: `python` and `folder` name
/// absolute paths into the engine folder, and Local AI won't start when they still name the
/// old one.
#[cfg(target_os = "macos")]
#[test]
fn a_moved_preferences_json_points_at_the_new_folder_too() {
    let root = temp("migration-prefs");
    let support = root.join("Library/Application Support");
    let old = support.join("LayerShop");
    let new = support.join("SilicaLyr");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(
        old.join("preferences.json"),
        format!(
            "{{\"ai\": {{\"comfy_launch\": {{\"python\": \"{old}/LocalAI/engine/python/bin/python3\", \
             \"folder\": \"{old}/LocalAI/engine\", \"args\": \"-s main.py\"}}}}, \
             \"recentFiles\": [\"/Users/dev/Pictures/a.pcraft\"], \"unknownField\": 7}}",
            old = old.display()
        ),
    )
    .unwrap();
    assert_eq!(migrate_data_folder(&new, &old), Migrated::Renamed);
    let prefs: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(new.join("preferences.json")).unwrap()).unwrap();
    let launch = &prefs["ai"]["comfy_launch"];
    assert_eq!(launch["python"].as_str(), Some(new.join("LocalAI/engine/python/bin/python3").to_str().unwrap()));
    assert_eq!(launch["folder"].as_str(), Some(new.join("LocalAI/engine").to_str().unwrap()));
    // What doesn't name the old folder stays exactly as it was: the args, a document outside
    // the data folder, and a field this app does not know.
    assert_eq!(launch["args"].as_str(), Some("-s main.py"));
    assert_eq!(prefs["recentFiles"][0].as_str(), Some("/Users/dev/Pictures/a.pcraft"));
    assert_eq!(prefs["unknownField"], 7);
    assert!(!new.join("preferences.json.new").exists(), "the rewrite leaves no temp file");
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn a_new_data_folder_is_left_as_it_is() {
    let root = temp("migration-new");
    let new = root.join("Library/Application Support/SilicaLyr");
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(new.join("preferences.json"), b"mine").unwrap();
    let old = root.join("Library/Application Support/LayerShop");
    assert_eq!(migrate_data_folder(&new, &old), Migrated::Nothing);
    assert_eq!(std::fs::read(new.join("preferences.json")).unwrap(), b"mine");
    assert!(!old.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn when_both_folders_exist_the_new_one_wins_and_the_old_is_untouched() {
    let root = temp("migration-both");
    let support = root.join("Library/Application Support");
    let old = support.join("LayerShop");
    let new = support.join("SilicaLyr");
    let original = record_in(&old, &support);
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(new.join("preferences.json"), b"mine").unwrap();
    assert_eq!(migrate_data_folder(&new, &old), Migrated::UsedNew);
    assert_eq!(std::fs::read_to_string(old.join("ai-setup.json")).unwrap(), original, "the old record still names the old folder");
    assert_eq!(std::fs::read(new.join("preferences.json")).unwrap(), b"mine", "and the new data is untouched");
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn no_folder_at_all_creates_nothing() {
    let root = temp("migration-none");
    let support = root.join("Library/Application Support");
    assert_eq!(migrate_data_folder(&support.join("SilicaLyr"), &support.join("LayerShop")), Migrated::Nothing);
    assert!(!support.exists(), "nothing was created");
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn a_link_where_the_old_folder_should_be_is_not_moved_and_is_kept_for_this_run() {
    let root = temp("migration-fail");
    let support = root.join("Library/Application Support");
    std::fs::create_dir_all(&support).unwrap();
    // A link to a folder on another volume: renaming it would leave the data behind.
    let real = root.join("Volume");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::write(real.join("preferences.json"), b"keep").unwrap();
    std::os::unix::fs::symlink(&real, support.join("LayerShop")).unwrap();
    let kept: OnceLock<PathBuf> = OnceLock::new();
    let outcome = migrate_macos_data_dir_from(&root, &kept);
    assert!(matches!(&outcome, Migrated::Failed(why) if why.contains("isn't a folder")), "{outcome:?}");
    assert_eq!(kept.get(), Some(&support.join("LayerShop")), "this run keeps using the old folder");
    assert_eq!(std::fs::read(real.join("preferences.json")).unwrap(), b"keep", "the linked data is untouched");
    assert!(!support.join("SilicaLyr").exists(), "no empty second data set is started");
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn the_start_up_wrapper_moves_the_real_folder_names_under_a_temp_home() {
    let root = temp("migration-home");
    let support = root.join("Library/Application Support");
    std::fs::create_dir_all(support.join("LayerShop")).unwrap();
    let kept: OnceLock<PathBuf> = OnceLock::new();
    assert_eq!(migrate_macos_data_dir_from(&root, &kept), Migrated::Renamed);
    assert!(support.join("SilicaLyr").is_dir() && !support.join("LayerShop").exists());
    assert!(kept.get().is_none(), "the fallback is only for a failed move");
    let _ = std::fs::remove_dir_all(&root);
}
