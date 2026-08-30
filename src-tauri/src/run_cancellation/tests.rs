//! Focused tests for shared provider and native-tool cancellation.

use futures_util::future::{AbortHandle, Abortable};

use super::*;

#[test]
fn voice_barge_in_cancels_every_registered_native_run() {
    tauri::async_runtime::block_on(async {
        let runs = ActiveRuns::default();
        let mut registrations = Vec::new();
        for run_id in ["run-one", "run-two"] {
            let (abort_handle, registration) = AbortHandle::new_pair();
            registrations.push(registration);
            runs.lock().await.insert(
                run_id.into(),
                ActiveRun {
                    abort_handle,
                    tool_cancellation: ToolLoopCancellation::default(),
                },
            );
        }

        assert_eq!(cancel_all_chats(&runs).await, 2);
        assert!(runs.lock().await.is_empty());
        for registration in registrations {
            assert!(
                Abortable::new(std::future::pending::<()>(), registration)
                    .await
                    .is_err()
            );
        }
    });
}
