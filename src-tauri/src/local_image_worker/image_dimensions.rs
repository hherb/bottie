//! Closed output sizes exposed by Bottie's local Qwen 2.1 image controls.

/// Three resolution tiers across square, 3:2, 2:3, 16:9, and 9:16 orientations.
/// All axes are multiples of 32; selection never grants the legacy worker a new profile.
const QWEN_21_DIMENSIONS: &[(u32, u32)] = &[
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
    (2048, 1152),
    (1152, 2048),
];

/// Accepts only concrete settings offered for the exact preferred image-server pack.
pub(crate) fn valid_qwen_21_dimensions(dimensions: (u32, u32)) -> bool {
    QWEN_21_DIMENSIONS.contains(&dimensions)
}
