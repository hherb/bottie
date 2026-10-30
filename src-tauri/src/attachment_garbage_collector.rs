//! Native startup orchestration for restart-safe attachment garbage collection.

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    storage::{AttachmentGarbageCollection, ConversationStore, StagingSweep, StorageError},
};

/// Sweeps abandoned restore staging and collects unreferenced attachment content before drafts or background processing can begin.
pub(crate) fn collect_at_startup(conversations: &ConversationStore, diagnostics: Diagnostics) {
    let sweep = conversations.sweep_interrupted_restore_staging();
    let collection = conversations.collect_unreferenced_attachments();
    tauri::async_runtime::spawn(async move {
        if let Some(detail) = sweep_failure_detail(&sweep) {
            record_diagnostic(
                &diagnostics,
                "warn",
                "Restore staging cleanup incomplete",
                None,
                Some(&detail),
            )
            .await;
        }
        record_collection_diagnostic(diagnostics, collection).await;
    });
}

/// Describes abandoned restore staging that could not be deleted, which otherwise leaks disk space silently.
fn sweep_failure_detail(sweep: &StagingSweep) -> Option<String> {
    (sweep.failed > 0).then(|| {
        format!(
            "{} interrupted restore staging item(s) could not be removed; {} removed",
            sweep.failed, sweep.removed
        )
    })
}

/// Records one path-free startup collection result in bounded session diagnostics.
async fn record_collection_diagnostic(
    diagnostics: Diagnostics,
    collection: Result<AttachmentGarbageCollection, StorageError>,
) {
    let (level, event, detail) = collection_diagnostic(collection);
    record_diagnostic(&diagnostics, level, event, None, Some(&detail)).await;
}

/// Converts native-only collection totals into one path-free diagnostic tuple.
fn collection_diagnostic(
    collection: Result<AttachmentGarbageCollection, StorageError>,
) -> (&'static str, &'static str, String) {
    match collection {
        Ok(outcome) => (
            "info",
            "Attachment cleanup completed",
            format!(
                "{} catalog item(s), {} original(s), {} derivative(s), {} generated image(s), and {} interrupted \
                 temporary file(s) removed; \
                 {} byte(s) reclaimed",
                outcome.catalog_entries_removed,
                outcome.original_files_removed,
                outcome.derivative_files_removed,
                outcome.generated_files_removed,
                outcome.temporary_files_removed,
                outcome.reclaimed_bytes
            ),
        ),
        Err(error) => ("error", "Attachment cleanup incomplete", error.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_diagnostics_expose_only_counts_and_reclaimed_bytes() {
        let (_, event, detail) = collection_diagnostic(Ok(AttachmentGarbageCollection {
            catalog_entries_removed: 2,
            original_files_removed: 3,
            derivative_files_removed: 4,
            generated_files_removed: 2,
            temporary_files_removed: 5,
            reclaimed_bytes: 6,
        }));

        assert_eq!(event, "Attachment cleanup completed");
        assert_eq!(
            detail,
            "2 catalog item(s), 3 original(s), 4 derivative(s), 2 generated image(s), and 5 interrupted temporary \
             file(s) removed; \
             6 byte(s) reclaimed"
        );
        assert!(!detail.contains('/'));
    }
}
