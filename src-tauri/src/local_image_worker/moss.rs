//! Explicit MOSS-Image installations use private one-request workers, never a WebView path or shell.
#[path = "moss/protocol.rs"]
mod protocol;
#[path = "moss/runtime.rs"]
mod runtime;
#[cfg(test)]
#[path = "moss/tests.rs"]
mod tests;
#[path = "moss/transport.rs"]
mod transport;

pub(crate) use runtime::MossInstallation;
/// Durable provider family identity shared by Base/SFT and both hardware adapters.
pub(crate) const PROVIDER_ID: &str = "moss-image";
/// Bound UTF-8 layout input independently of Qwen prompts and the worker encoder token limit.
pub(crate) const MAX_PROMPT_BYTES: usize = 32 * 1024;

/// Closed pinned package identities; aliases or arbitrary remote code cannot select this adapter.
pub(crate) fn is_moss_model(id: &str) -> bool {
    let repository = if let Some((repository, revision)) = id.split_once('@') {
        if revision.len() != 40
            || !revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return false;
        }
        repository
    } else {
        id
    };
    matches!(
        repository,
        "mlx-community/MOSS-Image-SFT-bf16"
            | "mlx-community/MOSS-Image-Base-bf16"
            | "OpenMOSS-Team/MOSS-Image-SFT"
            | "OpenMOSS-Team/MOSS-Image-Base"
    )
}

/// Preserve family identity in durable provenance and retries instead of labelling MOSS as Qwen.
pub(crate) fn provider_for_model(id: &str) -> &'static str {
    if is_moss_model(id) {
        PROVIDER_ID
    } else {
        crate::image_generation::QWEN_IMAGE_PROVIDER_ID
    }
}

/// Sizes mirror the UI's exact ratios and remain inside the Swift port's 1456² pixel envelope.
pub(crate) fn valid_dimensions(size: (u32, u32)) -> bool {
    const SIZES: &[(u32, u32)] = &[
        (512, 512),
        (768, 512),
        (512, 768),
        (1024, 576),
        (576, 1024),
        (768, 768),
        (1152, 768),
        (768, 1152),
        (1536, 864),
        (864, 1536),
        (1024, 1024),
        (1536, 1024),
        (1024, 1536),
        (1792, 1008),
        (1008, 1792),
    ];
    SIZES.contains(&size)
}

/// Fixed path-free failure states shared with the existing local image normalization path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MossError {
    Unavailable,
    InvalidOutput,
    Cancelled,
}
