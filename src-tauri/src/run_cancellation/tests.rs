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

#[test]
fn a_reserved_chat_run_blocks_restore_until_its_registration_drops() {
    tauri::async_runtime::block_on(async {
        let runs = ActiveRuns::default();
        let image_runs = ImageGenerationRuns::default();
        assert!(!any_generation_active(&runs, &image_runs).await);

        let (registration, _abort, _tools) = reserve_run(&runs, "run-one".into()).await;
        assert!(any_generation_active(&runs, &image_runs).await);

        drop(registration);
        assert!(!any_generation_active(&runs, &image_runs).await);
    });
}

#[test]
fn cancelling_a_reserved_run_reaches_its_task_handles() {
    tauri::async_runtime::block_on(async {
        let runs = ActiveRuns::default();
        let (_registration, abort, tools) = reserve_run(&runs, "run-one".into()).await;

        assert_eq!(cancel_all_chats(&runs).await, 1);
        assert!(tools.is_cancelled());
        assert!(
            Abortable::new(std::future::pending::<()>(), abort)
                .await
                .is_err()
        );
    });
}

#[test]
fn a_reserved_image_run_blocks_restore() {
    tauri::async_runtime::block_on(async {
        let runs = ActiveRuns::default();
        let image_runs = ImageGenerationRuns::default();
        let _abort = image_runs.reserve_abortable("image-one".into()).await;

        assert!(any_generation_active(&runs, &image_runs).await);
        image_runs.finish("image-one").await;
        assert!(!any_generation_active(&runs, &image_runs).await);
    });
}
