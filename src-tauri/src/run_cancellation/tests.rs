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
        // Runs stay registered until their tasks record the final cancelled state.
        assert_eq!(runs.lock().await.len(), 2);
        for registration in registrations {
            assert!(
                Abortable::new(std::future::pending::<()>(), registration)
                    .await
                    .is_err()
            );
        }
    });
}

#[test]
fn a_panicking_run_task_still_unregisters_its_run() {
    tauri::async_runtime::block_on(async {
        let runs = ActiveRuns::default();
        let (abort_handle, _registration) = AbortHandle::new_pair();
        runs.lock().await.insert(
            "run-one".into(),
            ActiveRun {
                abort_handle,
                tool_cancellation: ToolLoopCancellation::default(),
            },
        );

        let task_runs = runs.clone();
        let task = tauri::async_runtime::spawn(async move {
            let _registration = ActiveRunRegistration::new(task_runs, "run-one".into());
            panic!("provider task panicked");
        });

        assert!(task.await.is_err());
        assert!(runs.lock().await.is_empty());
    });
}
