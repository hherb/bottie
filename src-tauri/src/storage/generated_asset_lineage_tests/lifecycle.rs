//! Selected-branch export, backup, and retention coverage for image-edit lineage.

use std::{fs, io::Read};

use zip::ZipArchive;

use super::*;

#[test]
fn export_and_backup_preserve_portable_lineage_and_exact_source_bytes() {
    let live_path = test_database_path();
    let backup_path = live_path.with_file_name("edit-lineage-backup.sqlite3");
    let restore_path = live_path.with_file_name("restored").join("bottie.sqlite3");
    let store = ConversationStore::initialize(live_path).expect("store should initialize");
    let conversation = store
        .create_conversation("Portable edit")
        .expect("conversation should create");
    let (started, _, attachments) = started_edit(&store, &conversation.id);
    store
        .fail_generated_image_message(&started.message.id, GeneratedAssetStatus::Cancelled, None)
        .expect("edit should become terminal");

    let export = store
        .prepare_json_export(&conversation.id)
        .expect("export should prepare");
    let export_path = store.path.with_file_name("edit-lineage.zip");
    export.write_to(&export_path).expect("export should write");
    let mut archive = ZipArchive::new(fs::File::open(export_path).expect("ZIP should open"))
        .expect("ZIP should parse");
    let mut document = String::new();
    archive
        .by_name("bottie-portable-edit.json")
        .expect("JSON member should exist")
        .read_to_string(&mut document)
        .expect("JSON should read");
    let value: serde_json::Value = serde_json::from_str(&document).expect("JSON should parse");
    let sources = value["messages"][3]["generatedAssets"][0]["sources"]
        .as_array()
        .expect("portable lineage should exist");
    assert_eq!(value["version"], 7);
    assert_eq!(sources.len(), 3);
    assert!(
        sources
            .iter()
            .all(|source| source.get("sourceId").is_none())
    );
    for source in sources {
        let file = source["file"]
            .as_str()
            .expect("portable source file should exist");
        assert!(!archive_bytes(&mut archive, file).is_empty());
    }

    store
        .backup_to(&backup_path)
        .expect("backup should complete");
    let restored =
        ConversationStore::initialize(restore_path).expect("restore target should initialize");
    let safety = restored.path.with_file_name("pre-restore.sqlite3");
    restored
        .restore_from(&backup_path, &safety)
        .expect("portable lineage backup should restore");
    let reopened = restored
        .load_conversation(&conversation.id)
        .expect("restored lineage should reopen");
    assert_eq!(
        reopened.messages.last().unwrap().generated_assets[0]
            .sources
            .len(),
        3
    );
    assert!(
        restored
            .normalized_image_bytes_for_test(&attachments[0].id)
            .unwrap()
            .is_some()
    );
}

#[test]
fn branch_selection_preserves_lineage_only_on_its_original_ancestry() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let conversation = store
        .create_conversation("Branched edit")
        .expect("conversation should create");
    let original_branch_id = conversation.current_branch_id.clone();
    let (started, _, _) = started_edit(&store, &conversation.id);
    store
        .fail_generated_image_message(&started.message.id, GeneratedAssetStatus::Cancelled, None)
        .expect("edit should become terminal");
    let first_request_id = store
        .load_conversation(&conversation.id)
        .expect("original branch should load")
        .messages[0]
        .id
        .clone();

    let alternative = store
        .fork_from_user_message(
            &conversation.id,
            &first_request_id,
            "Create a different source",
        )
        .expect("earlier request should fork");
    assert_eq!(alternative.conversation.messages.len(), 1);
    assert!(
        alternative.conversation.messages[0]
            .generated_assets
            .is_empty()
    );

    let original = store
        .select_branch(&conversation.id, &original_branch_id)
        .expect("original edit branch should remain selectable");
    assert_eq!(
        original.messages.last().unwrap().generated_assets[0]
            .sources
            .len(),
        3
    );
}

#[test]
fn lineage_owns_removed_attachment_sources_until_retention_forgets_the_conversation() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("store should initialize");
    let conversation = store
        .create_conversation("Retained edit")
        .expect("conversation should create");
    let (started, generated, attachments) = started_edit(&store, &conversation.id);
    store
        .fail_generated_image_message(
            &started.message.id,
            GeneratedAssetStatus::Failed,
            Some("provider_failed"),
        )
        .expect("edit should become terminal");
    let edit_request_id = store
        .load_conversation(&conversation.id)
        .expect("conversation should load")
        .messages[2]
        .id
        .clone();
    let attachment_source = &started.message.generated_assets[0].sources[2];
    let attachment_path = store
        .normalized_image_path(
            &attachment_source.sha256,
            GeneratedImageSourceFormat::Png.normalized(),
        )
        .expect("attachment derivative path should resolve");
    let generated_path = store
        .generated_asset_blob_path(
            generated
                .sha256
                .as_deref()
                .expect("generated hash should exist"),
        )
        .expect("generated source path should resolve");

    store
        .remove_message_attachment(&conversation.id, &edit_request_id, &attachments[0].id)
        .expect("source association should be removable after acceptance");
    let collection = store
        .collect_all_unreferenced_attachments_for_test()
        .expect("lineage-aware collection should complete");
    assert_eq!(collection.catalog_entries_removed, 0);
    assert!(attachment_path.is_file());
    assert!(generated_path.is_file());

    store
        .delete_conversation(&conversation.id)
        .expect("conversation should move to Trash");
    store
        .set_conversation_retention_period(ConversationRetentionPeriod::ThirtyDays)
        .expect("retention should enable");
    let retention = store
        .apply_conversation_retention_at(i64::MAX)
        .expect("expired Trash should be forgotten");
    let forgotten = store
        .collect_all_unreferenced_attachments_for_test()
        .expect("forgotten bytes should collect");
    let connection = store.open().expect("store should open");
    let lineage_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM generated_asset_sources", [], |row| {
            row.get(0)
        })
        .expect("lineage count should load");

    assert_eq!(retention.forgotten_conversations, 1);
    assert_eq!(lineage_count, 0);
    assert!(!attachment_path.exists());
    assert!(!generated_path.exists());
    assert!(forgotten.catalog_entries_removed >= 1);
}

/// Reads one exact ZIP member for portable source-byte assertions.
fn archive_bytes(archive: &mut ZipArchive<fs::File>, name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("source member should exist")
        .read_to_end(&mut bytes)
        .expect("source member should read");
    bytes
}
