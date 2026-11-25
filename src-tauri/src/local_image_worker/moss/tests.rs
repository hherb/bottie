//! Failure-boundary coverage for MOSS identity and bounded private output.
use super::*;

// Layout-request validation lives with `ImageGenerationRequest` in `image_generation/tests.rs`,
// so this tree stays free of crate-root modules the feature-gated bins do not compile (#249).
#[test]
fn moss_provenance_is_model_specific() {
    let model = "mlx-community/MOSS-Image-SFT-bf16";
    assert_eq!(provider_for_model(model), "moss-image");
    assert_eq!(provider_for_model("Qwen/Qwen-Image-2512"), "qwen-image");
}

#[test]
fn moss_output_requires_matching_identity_seed_dimensions_and_one_png() {
    let mut event = serde_json::json!({"event":"complete", "modelId":"model", "revision":"rev",
        "width":512,"height":512,"seed":42,"png":"iVBORw0KGgo="});
    assert!(protocol::decode_complete(&event, "model", "rev", (512, 512), 42).is_ok());
    for key in ["modelId", "revision", "seed", "width", "height", "png"] {
        let old = event[key].clone();
        event[key] = serde_json::Value::Null;
        assert!(protocol::decode_complete(&event, "model", "rev", (512, 512), 42).is_err());
        event[key] = old;
    }
}

#[test]
fn moss_configuration_is_optional_but_invalid_registration_never_becomes_ready() {
    let root = std::env::temp_dir().join(format!("bottie-moss-config-{}", uuid::Uuid::new_v4()));
    assert!(MossInstallation::inspect(&root).unwrap().is_none());
    std::fs::create_dir_all(root.join("moss-image")).unwrap();
    std::fs::write(root.join("moss-image/runtime.json"), b"{}").unwrap();
    assert!(MossInstallation::inspect(&root).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
