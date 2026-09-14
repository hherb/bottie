//! Native macOS process measurement for the explicit local-image runtime proof.

/// Stable proof utility failures without filesystem paths or file contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeProofError {
    /// The operating system did not provide a usable whole-process measurement.
    Measurement,
}

/// Returns the lifetime maximum physical footprint for one live macOS process.
#[cfg(target_os = "macos")]
pub(crate) fn lifetime_peak_memory(process_id: u32) -> Result<u64, RuntimeProofError> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v4>::uninit();
    // SAFETY: macOS writes one rusage_info_v4 into the valid out pointer for the live PID.
    let result = unsafe {
        libc::proc_pid_rusage(
            process_id as libc::c_int,
            libc::RUSAGE_INFO_V4,
            usage.as_mut_ptr().cast(),
        )
    };
    if result != 0 {
        return Err(RuntimeProofError::Measurement);
    }
    // SAFETY: a zero result from proc_pid_rusage initialized the complete requested structure.
    let usage = unsafe { usage.assume_init() };
    if usage.ri_lifetime_max_phys_footprint == 0 {
        Err(RuntimeProofError::Measurement)
    } else {
        Ok(usage.ri_lifetime_max_phys_footprint)
    }
}

/// Rejects runtime measurement on targets outside the approved Apple-silicon proof host.
#[cfg(not(target_os = "macos"))]
pub(crate) fn lifetime_peak_memory(_process_id: u32) -> Result<u64, RuntimeProofError> {
    Err(RuntimeProofError::Measurement)
}
