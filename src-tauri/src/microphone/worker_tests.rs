//! Capture-owner replacement regression coverage.

use super::{MicrophonePhase, worker_can_be_replaced};

#[test]
fn permits_replacement_after_capture_stops_before_the_worker_reports_finished() {
    assert!(worker_can_be_replaced(MicrophonePhase::Idle, false));
    assert!(worker_can_be_replaced(MicrophonePhase::Captured, false));
    assert!(worker_can_be_replaced(MicrophonePhase::Error, false));
    assert!(!worker_can_be_replaced(MicrophonePhase::Starting, false));
    assert!(!worker_can_be_replaced(MicrophonePhase::Recording, false));
    assert!(worker_can_be_replaced(MicrophonePhase::Recording, true));
}
