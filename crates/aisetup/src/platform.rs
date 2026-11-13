//! The runtime platform a host is (or is not) a target for, shared by the
//! host probe and the runtime code so neither depends on the other.
//!
//! Selecting a platform says what a host *is*, never that the installer is
//! allowed to install for it: qualification and installability stay with the
//! probe ([`crate::probe::Probe::variant`], [`crate::probe::Probe::can_install`])
//! and the platform's own readiness, which is tracked in `docs/ai-setup.md`.

use std::cell::Cell;

use serde::{Deserialize, Serialize};

// Test-only stand-in for [`RuntimePlatform::host`], set by
// [`override_host_for_tests`]. The outer `Option` says whether an override is
// installed at all; the inner one is the answer to report (`Some(None)` lets a
// test pretend the machine has no managed build). Thread-local, so an override
// never leaks into another thread's view of the host. An ordinary comment, not
// a doc comment: rustdoc documents nothing on a `thread_local!` invocation, and
// the gate denies `unused_doc_comments`.
thread_local! {
    static HOST_OVERRIDE: Cell<Option<Option<RuntimePlatform>>> = const { Cell::new(None) };
}

/// Answers [`RuntimePlatform::host`] with `host` until the returned guard drops,
/// which restores whatever this thread saw before. `#[doc(hidden)]` and never
/// called by production code: it exists so setup tests can check both the
/// Windows and the macOS answers on one gate machine. The guard is per-thread,
/// so tests that spawn worker threads calling `host()` must not use it.
#[doc(hidden)]
#[must_use = "the host override ends as soon as this guard drops"]
pub fn override_host_for_tests(host: Option<RuntimePlatform>) -> HostOverride {
    let previous = HOST_OVERRIDE.with(|cell| cell.replace(Some(host)));
    HostOverride { previous }
}

/// Restores the previous host answer when dropped. See [`override_host_for_tests`].
#[doc(hidden)]
pub struct HostOverride {
    previous: Option<Option<RuntimePlatform>>,
}

impl Drop for HostOverride {
    fn drop(&mut self) {
        HOST_OVERRIDE.with(|cell| cell.set(self.previous));
    }
}

/// Launch platforms known to this setup: Windows portable build and macOS ARM64 managed runtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimePlatform {
    /// Windows portable build (the current implementation).
    #[default]
    WindowsPortable,
    /// macOS ARM64 managed runtime: the Apple Silicon build, installable once the host's
    /// unified memory qualifies ([`crate::probe::MIN_QUALIFIED_UNIFIED_MEMORY_MB`]).
    MacosArm64,
}

impl RuntimePlatform {
    /// Whether `variant` is a build this platform owns. Keeping this pairing in one typed
    /// boundary prevents a saved or hand-written choice from launching an archive as another
    /// platform.
    pub fn accepts_variant(self, variant: &str) -> bool {
        match self {
            RuntimePlatform::WindowsPortable => matches!(variant, "nvidia" | "nvidia_cu126" | "amd" | "intel"),
            RuntimePlatform::MacosArm64 => variant == RuntimeTarget::MACOS_ARM64_VARIANT,
        }
    }

    /// Windows portable alone needs Windows path and Visual C++ compatibility handling.
    pub fn needs_windows_support(self) -> bool {
        self == RuntimePlatform::WindowsPortable
    }

    /// The arguments for the platform: whether `--windows-standalone-build` is needed.
    pub fn extra_args(self) -> &'static [&'static str] {
        match self {
            RuntimePlatform::WindowsPortable => &["--windows-standalone-build"],
            RuntimePlatform::MacosArm64 => &[],
        }
    }

    /// The platform this computer is, if a managed runtime build exists for it.
    /// `None` means no managed build runs here (Linux, Intel Macs, other
    /// targets): setup must offer the externally-managed route instead, never
    /// plan or download a build for another platform.
    pub fn host() -> Option<Self> {
        // Only ever answered when a test installs one; production sees `None`
        // here and the real host facts decide, exactly as before.
        match HOST_OVERRIDE.with(|cell| cell.get()) {
            Some(answer) => answer,
            None => Self::host_from(std::env::consts::OS, std::env::consts::ARCH),
        }
    }

    /// The pure platform decision behind [`RuntimePlatform::host`], taking the
    /// host facts as parameters so every rule is testable on any machine:
    /// Apple-silicon macOS is `MacosArm64`, Windows is `WindowsPortable`, and
    /// nothing else has a managed build.
    pub fn host_from(os: &str, arch: &str) -> Option<Self> {
        match (os, arch) {
            ("macos", "aarch64") => Some(Self::MacosArm64),
            ("windows", _) => Some(Self::WindowsPortable),
            _ => None,
        }
    }

    /// The managed-build variant this platform uses when nothing more specific
    /// is chosen: the Apple-silicon runtime on macOS, the NVIDIA build on
    /// Windows (matching the historic default there).
    pub fn default_variant(self) -> &'static str {
        match self {
            RuntimePlatform::WindowsPortable => "nvidia",
            RuntimePlatform::MacosArm64 => RuntimeTarget::MACOS_ARM64_VARIANT,
        }
    }
}

/// The managed-runtime target the host's facts select: which
/// [`RuntimePlatform`] the host is a target for, which asset variant to use
/// on it, and whether the host qualifies for that variant. Selection only
/// reports what the host is; enabling installation for a platform is a
/// separate decision the setup UI makes with
/// [`Probe::can_install`](crate::probe::Probe::can_install).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeTarget {
    /// The platform family selected.
    pub platform: RuntimePlatform,
    /// The variant string to use on it: one of the manifest asset keys
    /// (`nvidia`, `nvidia_cu126`, `amd`, `intel`) on
    /// [`RuntimePlatform::WindowsPortable`], or
    /// [`RuntimeTarget::MACOS_ARM64_VARIANT`] on
    /// [`RuntimePlatform::MacosArm64`].
    pub variant: String,
    /// Whether the host's driver meets the selected variant's requirements:
    /// true for a Windows host whose NVIDIA driver is current enough for the
    /// main `nvidia` build, false for hosts that only qualify for a fallback
    /// or preview variant (the AMD and Intel previews) and for hosts whose
    /// platform is not installable yet.
    pub qualified: bool,
}

impl RuntimeTarget {
    /// The variant name the Apple-silicon macOS runtime uses, matching the
    /// marker validation in the `runtime` module.
    pub const MACOS_ARM64_VARIANT: &'static str = "macos_arm64";

    /// Builds a target from parts.
    pub fn new(platform: RuntimePlatform, variant: impl Into<String>, qualified: bool) -> Self {
        Self { platform, variant: variant.into(), qualified }
    }

    /// The selected runtime platform.
    pub fn platform(&self) -> RuntimePlatform {
        self.platform
    }

    /// The variant string to use on the platform.
    pub fn variant(&self) -> &str {
        &self.variant
    }

    /// Whether this host/runtime pairing has been qualified.
    pub fn is_qualified(&self) -> bool {
        self.qualified
    }

    /// Whether this target is the Windows portable runtime.
    pub fn is_windows(&self) -> bool {
        self.platform == RuntimePlatform::WindowsPortable
    }

    /// Whether this target is the Apple-silicon macOS runtime.
    pub fn is_macos_arm64(&self) -> bool {
        self.platform == RuntimePlatform::MacosArm64
    }
}

/// Decide which managed runtime a setup should use, so a host never plans,
/// downloads or launches a build made for another platform.
///
/// - `host` is [`RuntimePlatform::host`]: `None` (Linux, Intel Macs) means no
///   managed build runs here at all.
/// - `want_platform` / `want_variant` are explicit request parameters; a
///   platform that is not the host's, or a variant the host platform does not
///   own, is an actionable error, never a silent cross-platform choice. A
///   variant the host does own is used as asked, ahead of `saved` and the default.
/// - `saved` is the platform and variant from the stored setup record: it is
///   reused only when it names this host platform and an accepted variant; a
///   record from another platform is ignored (host defaults apply) rather than
///   reused.
///
/// Returns the platform and variant to use, or an actionable message.
pub fn resolve_runtime(
    host: Option<RuntimePlatform>,
    want_platform: Option<RuntimePlatform>,
    want_variant: Option<&str>,
    saved: Option<(RuntimePlatform, &str)>,
) -> Result<(RuntimePlatform, String), String> {
    let Some(host) = host else {
        return Err(
            "This computer can't run a managed ComfyUI build. Point PhotoCraft at your own ComfyUI installation (\"Use my ComfyUI\") instead.".to_string()
        );
    };
    if let Some(want) = want_platform
        && want != host
    {
        return Err(format!(
            "The {want:?} runtime can't run on this computer, so it won't be downloaded. This machine is a {host:?} host; leave the platform unset (or set it to {host:?}) to use the build for this computer."
        ));
    }
    if let Some(variant) = want_variant.filter(|v| !v.is_empty()) {
        if !host.accepts_variant(variant) {
            let usable = if host == RuntimePlatform::MacosArm64 { "'macos_arm64'" } else { "'nvidia', 'nvidia_cu126', 'amd' or 'intel'" };
            return Err(format!("There is no {host:?} build for graphics card '{variant}'. This computer can use {usable}."));
        }
        // An explicit variant the host platform owns is used as asked, in preference
        // to the saved record and the platform default.
        return Ok((host, variant.to_string()));
    }
    if let Some((saved_platform, saved_variant)) = saved
        && saved_platform == host
        && !saved_variant.is_empty()
        && host.accepts_variant(saved_variant)
    {
        return Ok((host, saved_variant.to_string()));
    }
    Ok((host, host.default_variant().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_round_trip_with_typed_platform_and_variant() {
        for t in [
            RuntimeTarget::new(RuntimePlatform::WindowsPortable, "nvidia", true),
            RuntimeTarget::new(RuntimePlatform::WindowsPortable, "nvidia_cu126".to_string(), true),
            RuntimeTarget::new(RuntimePlatform::MacosArm64, RuntimeTarget::MACOS_ARM64_VARIANT, false),
        ] {
            let json = serde_json::to_string(&t).unwrap();
            assert_eq!(serde_json::from_str::<RuntimeTarget>(&json).unwrap(), t, "{json}");
        }
        assert_eq!(
            serde_json::to_string(&RuntimeTarget::new(RuntimePlatform::MacosArm64, "macos_arm64", false)).unwrap(),
            r#"{"platform":"macosArm64","variant":"macos_arm64","qualified":false}"#
        );
    }

    #[test]
    fn platform_names_are_the_marker_wire_names() {
        // The marker format already uses these names; changing one orphans installed markers.
        assert_eq!(serde_json::to_string(&RuntimePlatform::WindowsPortable).unwrap(), r#""windowsPortable""#);
        assert_eq!(serde_json::to_string(&RuntimePlatform::MacosArm64).unwrap(), r#""macosArm64""#);
        assert_eq!(RuntimePlatform::default(), RuntimePlatform::WindowsPortable);
    }

    #[test]
    fn platforms_accept_only_their_own_runtime_variants() {
        for variant in ["nvidia", "nvidia_cu126", "amd", "intel"] {
            assert!(RuntimePlatform::WindowsPortable.accepts_variant(variant));
            assert!(!RuntimePlatform::MacosArm64.accepts_variant(variant));
        }
        assert!(RuntimePlatform::MacosArm64.accepts_variant(RuntimeTarget::MACOS_ARM64_VARIANT));
        assert!(!RuntimePlatform::WindowsPortable.accepts_variant(RuntimeTarget::MACOS_ARM64_VARIANT));
        assert!(!RuntimePlatform::WindowsPortable.accepts_variant("voodoo"));
        assert!(RuntimePlatform::WindowsPortable.needs_windows_support());
        assert!(!RuntimePlatform::MacosArm64.needs_windows_support());
    }

    #[test]
    fn host_platform_is_decoded_from_host_facts_only() {
        assert_eq!(RuntimePlatform::host_from("macos", "aarch64"), Some(RuntimePlatform::MacosArm64));
        assert_eq!(RuntimePlatform::host_from("macos", "x86_64"), None);
        assert_eq!(RuntimePlatform::host_from("windows", "x86_64"), Some(RuntimePlatform::WindowsPortable));
        assert_eq!(RuntimePlatform::host_from("windows", "aarch64"), Some(RuntimePlatform::WindowsPortable));
        assert_eq!(RuntimePlatform::host_from("linux", "x86_64"), None);
        assert_eq!(RuntimePlatform::host_from("linux", "aarch64"), None);
        // The live call must agree with the pure rule for this machine's own facts.
        assert_eq!(RuntimePlatform::host(), RuntimePlatform::host_from(std::env::consts::OS, std::env::consts::ARCH));
    }

    #[test]
    fn host_override_answers_the_host_call_until_the_guard_drops() {
        let real = RuntimePlatform::host();
        let win = Some(RuntimePlatform::WindowsPortable);
        let mac = Some(RuntimePlatform::MacosArm64);
        {
            let _first = override_host_for_tests(win);
            assert_eq!(RuntimePlatform::host(), win, "the override answers while it is installed");
            {
                // Guards nest: the inner answer applies, and the outer one returns when it drops.
                let _second = override_host_for_tests(mac);
                assert_eq!(RuntimePlatform::host(), mac);
                {
                    // An override can even pretend the machine has no managed build.
                    let _third = override_host_for_tests(None);
                    assert_eq!(RuntimePlatform::host(), None);
                }
                assert_eq!(RuntimePlatform::host(), mac, "the inner guard restores the outer answer");
            }
            assert_eq!(RuntimePlatform::host(), win, "dropping a guard restores the previous answer");
        }
        assert_eq!(RuntimePlatform::host(), real, "the real host returns once every guard has dropped");
    }

    #[test]
    fn each_platform_defaults_to_its_own_build() {
        assert_eq!(RuntimePlatform::MacosArm64.default_variant(), RuntimeTarget::MACOS_ARM64_VARIANT);
        assert_eq!(RuntimePlatform::WindowsPortable.default_variant(), "nvidia");
        for p in [RuntimePlatform::MacosArm64, RuntimePlatform::WindowsPortable] {
            assert!(p.accepts_variant(p.default_variant()));
        }
    }

    #[test]
    fn a_macos_host_with_no_parameters_gets_the_macos_arm64_build() {
        let (platform, variant) = resolve_runtime(Some(RuntimePlatform::MacosArm64), None, None, None).unwrap();
        assert_eq!(platform, RuntimePlatform::MacosArm64);
        assert_eq!(variant, "macos_arm64");
    }

    #[test]
    fn a_macos_host_cannot_be_asked_for_the_windows_build() {
        // Asking for the other platform outright is an actionable error, never a silent download.
        let err = resolve_runtime(Some(RuntimePlatform::MacosArm64), Some(RuntimePlatform::WindowsPortable), None, None).unwrap_err();
        assert!(err.contains("can't run on this computer"), "{err}");
        // And so is naming a variant the host platform does not own, even without a platform param.
        let err = resolve_runtime(Some(RuntimePlatform::MacosArm64), None, Some("nvidia"), None).unwrap_err();
        assert!(err.contains("macos_arm64"), "{err}");
        for variant in ["nvidia", "nvidia_cu126", "amd", "intel", "dml", "voodoo"] {
            assert!(resolve_runtime(Some(RuntimePlatform::MacosArm64), None, Some(variant), None).is_err(), "{variant} must not be planned on macOS");
        }
    }

    #[test]
    fn a_windows_host_cannot_be_asked_for_the_macos_build() {
        let win = Some(RuntimePlatform::WindowsPortable);
        let err = resolve_runtime(win, Some(RuntimePlatform::MacosArm64), None, None).unwrap_err();
        assert!(err.contains("can't run on this computer"), "{err}");
        let err = resolve_runtime(win, None, Some("macos_arm64"), None).unwrap_err();
        assert!(err.contains("nvidia"), "{err}");
        // A Windows host still resolves exactly as before: nvidia by default, its own variants on request.
        assert_eq!(resolve_runtime(win, None, None, None).unwrap(), (RuntimePlatform::WindowsPortable, "nvidia".to_string()));
        assert_eq!(resolve_runtime(win, None, Some("amd"), None).unwrap(), (RuntimePlatform::WindowsPortable, "amd".to_string()));
        assert_eq!(resolve_runtime(win, win, Some("nvidia_cu126"), None).unwrap(), (RuntimePlatform::WindowsPortable, "nvidia_cu126".to_string()));
    }

    #[test]
    fn a_saved_record_for_another_platform_is_not_reused() {
        let mac = Some(RuntimePlatform::MacosArm64);
        let win = Some(RuntimePlatform::WindowsPortable);
        // A record carried over from a Windows machine must not make a Mac download or run the Windows build:
        // the host platform and its own default apply instead, without an error (setup can continue).
        let (platform, variant) = resolve_runtime(mac, None, None, Some((RuntimePlatform::WindowsPortable, "nvidia"))).unwrap();
        assert_eq!(platform, RuntimePlatform::MacosArm64);
        assert_eq!(variant, "macos_arm64");
        // Same for a foreign platform with a foreign variant string, and for an empty saved variant.
        assert_eq!(resolve_runtime(mac, None, None, Some((RuntimePlatform::WindowsPortable, "amd"))).unwrap().1, "macos_arm64");
        assert_eq!(resolve_runtime(mac, None, None, Some((RuntimePlatform::MacosArm64, ""))).unwrap().1, "macos_arm64");
        // A foreign variant saved under this platform is not reused either; an accepted one is.
        assert_eq!(resolve_runtime(mac, None, None, Some((RuntimePlatform::MacosArm64, "nvidia"))).unwrap().1, "macos_arm64");
        assert_eq!(resolve_runtime(mac, None, None, Some((RuntimePlatform::MacosArm64, "macos_arm64"))).unwrap().1, "macos_arm64");
        assert_eq!(resolve_runtime(win, None, None, Some((RuntimePlatform::WindowsPortable, "amd"))).unwrap().1, "amd");
        // An explicit accepted variant wins over the saved one; an explicit foreign variant is still an error.
        assert_eq!(resolve_runtime(win, None, Some("intel"), Some((RuntimePlatform::WindowsPortable, "amd"))).unwrap().1, "intel");
        assert!(resolve_runtime(mac, None, Some("nvidia"), Some((RuntimePlatform::WindowsPortable, "nvidia"))).is_err());
    }

    #[test]
    fn a_host_without_a_managed_build_gets_an_actionable_error() {
        let err = resolve_runtime(None, None, None, None).unwrap_err();
        assert!(err.contains("Use my ComfyUI"), "{err}");
        assert!(resolve_runtime(None, Some(RuntimePlatform::WindowsPortable), Some("nvidia"), Some((RuntimePlatform::WindowsPortable, "nvidia"))).is_err());
    }
}
