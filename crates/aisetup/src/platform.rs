//! The runtime platform a host is (or is not) a target for, shared by the
//! host probe and the runtime code so neither depends on the other.
//!
//! Selecting a platform says what a host *is*, never that the installer is
//! allowed to install for it: qualification and installability stay with the
//! probe ([`crate::probe::Probe::variant`], [`crate::probe::Probe::can_install`])
//! and the platform's own readiness, which is tracked in `docs/ai-setup.md`.

use serde::{Deserialize, Serialize};

/// Launch platforms known to this setup: Windows portable build and macOS ARM64 managed runtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimePlatform {
    /// Windows portable build (the current implementation).
    #[default]
    WindowsPortable,
    /// macOS ARM64 managed runtime (defined for future platform support; CPU mode is explicit).
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
}
