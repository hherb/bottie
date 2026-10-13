//! Regression coverage for explicit revision-bound optional model license acceptance.

use super::*;

#[test]
fn research_license_requires_explicit_revision_bound_acceptance() {
    let metadata = LocalImageAvailabilityMetadata {
        model_id: "Qwen/Qwen-Image-2.1".into(),
        package_id: "Qwen/Qwen-Image-2.1".into(),
        runtime_id: "diffusers-test@0123456789abcdef".into(),
        license: "LicenseRef-Qwen-Research".into(),
        source_revision: "1".repeat(40),
        expected_disk_bytes: 1,
        worker_expected_disk_bytes: 1,
        required_memory_bytes: 1,
        availability: LocalImageAvailability::ModelMissing,
    };
    let mut approval = LocalImageAcquisitionApproval {
        model_id: metadata.model_id.clone(),
        package_id: metadata.package_id.clone(),
        runtime_id: metadata.runtime_id.clone(),
        license: metadata.license.clone(),
        source_revision: metadata.source_revision.clone(),
        expected_disk_bytes: 1,
        worker_expected_disk_bytes: 1,
        required_memory_bytes: 1,
        approved: true,
        license_accepted: false,
    };
    assert!(!approval_matches(&approval, &metadata));
    approval.license_accepted = true;
    assert!(approval_matches(&approval, &metadata));
    approval.source_revision = "2".repeat(40);
    assert!(!approval_matches(&approval, &metadata));
}
