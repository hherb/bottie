//! Native host facts for the closed local-image availability policy.

use super::availability::{
    HardwareEvidenceProfile, HostArchitecture, HostOperatingSystem, LocalImageHardwareFacts,
};

const APPLE_M3_MAX_BRAND: &str = "Apple M3 Max";

/// Stable failure when the operating system cannot provide required hardware facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HardwareProbeError {
    /// A required operating-system value was absent, malformed, or zero.
    Unavailable,
}

/// Reads exact host facts without consulting product names or model aliases.
pub(crate) fn probe_local_image_hardware() -> Result<LocalImageHardwareFacts, HardwareProbeError> {
    let physical_memory_bytes = physical_memory_bytes()?;
    Ok(LocalImageHardwareFacts {
        operating_system: current_operating_system(),
        architecture: current_architecture(),
        physical_memory_bytes,
        evidence_profile: current_evidence_profile(physical_memory_bytes),
    })
}

/// Maps only the chip and memory tier exercised by an accepted runtime proof.
pub(crate) fn evidence_profile_for(
    processor_brand: &str,
    physical_memory_bytes: u64,
) -> Option<HardwareEvidenceProfile> {
    let profile = (processor_brand == APPLE_M3_MAX_BRAND)
        .then_some(HardwareEvidenceProfile::AppleM3Max128Gb)?;
    (physical_memory_bytes == profile.physical_memory_bytes()).then_some(profile)
}

const fn current_operating_system() -> HostOperatingSystem {
    if cfg!(target_os = "macos") {
        HostOperatingSystem::MacOs
    } else if cfg!(target_os = "linux") {
        HostOperatingSystem::Linux
    } else if cfg!(target_os = "windows") {
        HostOperatingSystem::Windows
    } else {
        HostOperatingSystem::Other
    }
}

const fn current_architecture() -> HostArchitecture {
    if cfg!(target_arch = "aarch64") {
        HostArchitecture::Aarch64
    } else if cfg!(target_arch = "x86_64") {
        HostArchitecture::X86_64
    } else {
        HostArchitecture::Other
    }
}

#[cfg(target_os = "macos")]
fn current_evidence_profile(physical_memory_bytes: u64) -> Option<HardwareEvidenceProfile> {
    let processor_brand = sysctl_string(b"machdep.cpu.brand_string\0").ok()?;
    evidence_profile_for(&processor_brand, physical_memory_bytes)
}

#[cfg(not(target_os = "macos"))]
fn current_evidence_profile(_physical_memory_bytes: u64) -> Option<HardwareEvidenceProfile> {
    None
}

#[cfg(target_os = "macos")]
fn physical_memory_bytes() -> Result<u64, HardwareProbeError> {
    const SYSCTL_NAME: &[u8] = b"hw.memsize\0";
    let mut memory = 0_u64;
    let mut value_size = std::mem::size_of::<u64>();
    // SAFETY: both writable pointers reference initialized values of the advertised size, the
    // sysctl name is NUL-terminated, and this read supplies no replacement value.
    let result = unsafe {
        libc::sysctlbyname(
            SYSCTL_NAME.as_ptr().cast(),
            (&mut memory as *mut u64).cast(),
            &mut value_size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result == 0 && value_size == std::mem::size_of::<u64>() && memory > 0 {
        Ok(memory)
    } else {
        Err(HardwareProbeError::Unavailable)
    }
}

#[cfg(target_os = "macos")]
fn sysctl_string(name: &[u8]) -> Result<String, HardwareProbeError> {
    const MAX_SYSCTL_STRING_BYTES: usize = 256;
    if name.last() != Some(&0) {
        return Err(HardwareProbeError::Unavailable);
    }
    let mut buffer = [0_u8; MAX_SYSCTL_STRING_BYTES];
    let mut value_size = buffer.len();
    // SAFETY: the name is checked as NUL-terminated, the output buffer is writable for the
    // advertised size, and this read supplies no replacement value.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr().cast(),
            buffer.as_mut_ptr().cast(),
            &mut value_size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || value_size == 0 || value_size > buffer.len() {
        return Err(HardwareProbeError::Unavailable);
    }
    let bytes = &buffer[..value_size];
    let value = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    std::str::from_utf8(value)
        .ok()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or(HardwareProbeError::Unavailable)
}

#[cfg(not(target_os = "macos"))]
fn physical_memory_bytes() -> Result<u64, HardwareProbeError> {
    Ok(0)
}
