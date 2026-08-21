//! Native startup orchestration for restart-safe attachment garbage collection.

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    storage::{AttachmentGarbageCollection, ConversationStore, StorageError},
};

/// Collects unreferenced attachment content before drafts or background processing can begin.
pub(crate) fn collect_at_startup(conversations: &ConversationStore, diagnostics: Diagnostics) {
    let collection = conversations.collect_unreferenced_attachments();
    tauri::async_runtime::spawn(record_collection_diagnostic(diagnostics, collection));
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
                "{} catalog item(s), {} original(s), {} derivative(s), and {} interrupted temporary file(s) removed; \
                 {} byte(s) reclaimed",
                outcome.catalog_entries_removed,
                outcome.original_files_removed,
                outcome.derivative_files_removed,
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
            temporary_files_removed: 5,
            reclaimed_bytes: 6,
        }));

        assert_eq!(event, "Attachment cleanup completed");
        assert_eq!(
            detail,
            "2 catalog item(s), 3 original(s), 4 derivative(s), and 5 interrupted temporary file(s) removed; \
             6 byte(s) reclaimed"
        );
        assert!(!detail.contains('/'));
    }
}
