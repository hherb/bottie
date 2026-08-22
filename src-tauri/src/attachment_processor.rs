//! Process-lifetime scheduling for durable attachment processing and indexing readiness.

use std::{
    sync::mpsc::{Receiver, Sender, channel},
    thread,
};

use tauri::{AppHandle, Emitter};

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    semantic_indexer::SemanticIndexer,
    storage::ConversationStore,
};

/// Path-free native event emitted after one retained attachment reaches current durable state.
pub(crate) const ATTACHMENT_PROCESSING_EVENT: &str = "attachment-processing-updated";
const WORKER_THREAD_NAME: &str = "bottie-attachment-processing";

/// Cheap wake handle for Bottie's single attachment-processing worker.
#[derive(Clone)]
pub(crate) struct AttachmentProcessor {
    sender: Sender<WorkerCommand>,
}

/// Commands serialized by the single native worker thread.
enum WorkerCommand {
    Wake,
    Pause(Sender<()>),
    Resume,
}

/// Scope guard that resumes processing even when restore returns an error.
pub(crate) struct AttachmentProcessingPause {
    sender: Sender<WorkerCommand>,
}

impl Drop for AttachmentProcessingPause {
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerCommand::Resume);
    }
}

impl AttachmentProcessor {
    /// Starts one process-lifetime worker without performing attachment work inline.
    pub(crate) fn start(
        app: AppHandle,
        conversations: ConversationStore,
        diagnostics: Diagnostics,
        semantic_indexing: SemanticIndexer,
    ) -> Self {
        let (sender, receiver) = channel();
        thread::Builder::new()
            .name(WORKER_THREAD_NAME.into())
            .spawn(move || run_worker(receiver, app, conversations, diagnostics, semantic_indexing))
            .expect("the attachment processing worker must start");
        Self { sender }
    }

    /// Wakes the worker after startup, ingestion, or a completed store restore.
    pub(crate) fn wake(&self) {
        let _ = self.sender.send(WorkerCommand::Wake);
    }

    /// Waits for any current item to finish, then pauses work for one store replacement.
    pub(crate) fn pause(&self) -> AttachmentProcessingPause {
        let (acknowledge, acknowledged) = channel();
        if self.sender.send(WorkerCommand::Pause(acknowledge)).is_ok() {
            let _ = acknowledged.recv();
        }
        AttachmentProcessingPause {
            sender: self.sender.clone(),
        }
    }
}

/// Drains requested work one item at a time so restore pauses are observed promptly.
fn run_worker(
    receiver: Receiver<WorkerCommand>,
    app: AppHandle,
    conversations: ConversationStore,
    diagnostics: Diagnostics,
    semantic_indexing: SemanticIndexer,
) {
    let mut wake_requested = false;
    let mut paused = false;
    loop {
        while let Ok(command) = receiver.try_recv() {
            apply_command(command, &mut wake_requested, &mut paused);
        }
        if !paused && wake_requested {
            match conversations.process_next_pending_attachment() {
                Ok(Some(attachment)) => {
                    let _ = app.emit(ATTACHMENT_PROCESSING_EVENT, attachment);
                    semantic_indexing.wake();
                    continue;
                }
                Ok(None) => wake_requested = false,
                Err(error) => {
                    wake_requested = false;
                    record_worker_failure(diagnostics.clone(), error.message);
                }
            }
        }
        let Ok(command) = receiver.recv() else {
            break;
        };
        apply_command(command, &mut wake_requested, &mut paused);
    }
}

/// Applies one scheduler command without touching attachment content.
fn apply_command(command: WorkerCommand, wake_requested: &mut bool, paused: &mut bool) {
    match command {
        WorkerCommand::Wake => *wake_requested = true,
        WorkerCommand::Pause(acknowledge) => {
            *paused = true;
            let _ = acknowledge.send(());
        }
        WorkerCommand::Resume => *paused = false,
    }
}

/// Records one path-redacted worker failure without blocking the processing thread.
fn record_worker_failure(diagnostics: Diagnostics, detail: String) {
    tauri::async_runtime::spawn(async move {
        record_diagnostic(
            &diagnostics,
            "error",
            "Attachment processing paused",
            None,
            Some(&detail),
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_preserves_coalesced_work_until_resume() {
        let mut wake_requested = false;
        let mut paused = false;
        apply_command(WorkerCommand::Wake, &mut wake_requested, &mut paused);
        let (acknowledge, acknowledged) = channel();
        apply_command(
            WorkerCommand::Pause(acknowledge),
            &mut wake_requested,
            &mut paused,
        );

        assert!(wake_requested);
        assert!(paused);
        acknowledged.recv().expect("pause should acknowledge");

        apply_command(WorkerCommand::Resume, &mut wake_requested, &mut paused);
        assert!(wake_requested);
        assert!(!paused);
    }
}
