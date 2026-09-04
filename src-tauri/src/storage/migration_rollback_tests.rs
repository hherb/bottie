//! Staged migration and interrupted-promotion contract tests.

use std::fs;

use rusqlite::{Connection, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME, MIGRATION_1,
    migration_rollback::{
        MigrationFault, managed_recovery_points, migration_marker_path, prune_recovery_points,
    },
    tests::test_database_path,
};

#[test]
fn staged_upgrade_promotes_wal_content_and_keeps_source_recovery_point() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "wal-conversation");
    let attachment = attachment_fixture(&path);

    let store = ConversationStore::initialize(path.clone()).expect("staged migration should pass");
    let conversations = store
        .list_conversations()
        .expect("promoted conversations should load");
    let recovery_points = managed_recovery_points(&path).expect("recovery points should list");

    assert_eq!(
        store.status().expect("status should load").schema_version,
        22
    );
    assert_eq!(conversations[0].id, "wal-conversation");
    assert_eq!(recovery_points.len(), 1);
    assert_eq!(database_version(&recovery_points[0]), 1);
    assert!(conversation_exists(&recovery_points[0], "wal-conversation"));
    assert_eq!(
        fs::read(attachment).expect("attachment should remain"),
        b"unchanged"
    );
    drop(source);
}

#[test]
fn candidate_failure_leaves_live_source_and_attachment_unchanged() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "source-conversation");
    let attachment = attachment_fixture(&path);

    let error = ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::AfterCandidateMigration,
    )
    .expect_err("candidate fault should stop startup");

    assert_eq!(error.code, "migration_failed");
    assert_eq!(database_version(&path), 1);
    assert!(conversation_exists(&path, "source-conversation"));
    assert_eq!(
        fs::read(attachment).expect("attachment should remain"),
        b"unchanged"
    );
    assert!(
        managed_recovery_points(&path)
            .expect("recovery points should list")
            .is_empty()
    );
    assert!(!migration_marker_path(&path).exists());
    drop(source);
}

#[test]
fn copy_early_migration_and_candidate_validation_faults_never_touch_live_data() {
    for fault in [
        MigrationFault::BeforeCandidateCopy,
        MigrationFault::BeforeCandidateMigration,
        MigrationFault::DuringCandidateValidation,
    ] {
        let path = test_database_path();
        let source = version_one_fixture(&path, "fault-source");
        drop(source);

        let error = ConversationStore::initialize_with_migration_fault(path.clone(), fault)
            .expect_err("pre-promotion fault should stop startup");

        assert_eq!(error.code, "migration_failed");
        assert_eq!(database_version(&path), 1);
        assert!(conversation_exists(&path, "fault-source"));
        assert!(
            managed_recovery_points(&path)
                .expect("recovery points should list")
                .is_empty()
        );
        assert!(!migration_marker_path(&path).exists());
    }
}

#[test]
fn safety_copy_failure_prevents_live_promotion() {
    for fault in [
        MigrationFault::BeforeSafetyCopy,
        MigrationFault::DuringSafetyCopyValidation,
    ] {
        let path = test_database_path();
        let source = version_one_fixture(&path, "safe-source");

        let error = ConversationStore::initialize_with_migration_fault(path.clone(), fault)
            .expect_err("safety-copy fault should stop startup");

        assert_eq!(error.code, "migration_failed");
        assert_eq!(database_version(&path), 1);
        assert!(conversation_exists(&path, "safe-source"));
        assert!(
            managed_recovery_points(&path)
                .expect("recovery points should list")
                .is_empty()
        );
        assert!(!migration_marker_path(&path).exists());
        drop(source);
    }
}

#[test]
fn marker_write_failure_cleans_candidate_and_recovery_before_promotion() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "marker-write-source");
    drop(source);

    let error = ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::DuringMarkerWrite,
    )
    .expect_err("marker-write fault should stop startup");

    assert_eq!(error.code, "migration_failed");
    assert_eq!(database_version(&path), 1);
    assert!(conversation_exists(&path, "marker-write-source"));
    assert!(
        managed_recovery_points(&path)
            .expect("recovery points should list")
            .is_empty()
    );
    assert!(!migration_marker_path(&path).exists());
    assert!(!has_candidate_file(&path));
}

#[test]
fn promotion_failure_restores_the_verified_source() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "restore-source");
    drop(source);

    let error = ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::DuringLivePromotion,
    )
    .expect_err("promotion fault should stop startup");

    assert_eq!(error.code, "migration_failed");
    assert_eq!(database_version(&path), 1);
    assert!(conversation_exists(&path, "restore-source"));
    assert_eq!(
        managed_recovery_points(&path)
            .expect("recovery points should list")
            .len(),
        1
    );
    assert!(!migration_marker_path(&path).exists());
}

#[test]
fn restart_finishes_a_promoted_target_left_before_cleanup() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "promoted-target");
    drop(source);

    ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::AfterLivePromotion,
    )
    .expect_err("post-promotion fault should emulate process interruption");
    assert_eq!(database_version(&path), 22);
    assert!(migration_marker_path(&path).exists());

    let reopened = ConversationStore::initialize(path.clone())
        .expect("restart should accept and finish the promoted target");

    assert_eq!(
        reopened
            .status()
            .expect("status should load")
            .schema_version,
        22
    );
    assert!(conversation_exists(&path, "promoted-target"));
    assert!(!migration_marker_path(&path).exists());
}

#[test]
fn restart_restores_source_when_a_marked_live_store_is_damaged() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "marked-source");
    drop(source);

    ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::AfterPromotionMarker,
    )
    .expect_err("marker fault should emulate process interruption");
    fs::write(&path, b"damaged during promotion").expect("live store should be damaged");

    let error = ConversationStore::initialize(path.clone())
        .expect_err("reconciliation should restore source and stop this startup");

    assert_eq!(error.code, "migration_failed");
    assert_eq!(database_version(&path), 1);
    assert!(conversation_exists(&path, "marked-source"));
    assert!(!migration_marker_path(&path).exists());
}

#[test]
fn newer_schema_and_malformed_ledger_fail_without_managed_artifacts() {
    let newer_path = test_database_path();
    let newer = version_one_fixture(&newer_path, "newer-source");
    newer
        .pragma_update(None, "user_version", 23)
        .expect("newer version should be set");
    drop(newer);

    let newer_error = ConversationStore::initialize(newer_path.clone())
        .expect_err("newer schema should fail closed");
    assert_eq!(newer_error.code, "newer_schema");
    assert_eq!(database_version(&newer_path), 23);
    assert!(
        managed_recovery_points(&newer_path)
            .expect("recovery points should list")
            .is_empty()
    );

    let ledger_path = test_database_path();
    let ledger = version_one_fixture(&ledger_path, "ledger-source");
    ledger
        .execute("DELETE FROM schema_migrations", [])
        .expect("ledger should be malformed");
    drop(ledger);

    let ledger_error = ConversationStore::initialize(ledger_path.clone())
        .expect_err("malformed ledger should fail closed");
    assert_eq!(ledger_error.code, "migration_failed");
    assert_eq!(database_version(&ledger_path), 1);
    assert!(
        managed_recovery_points(&ledger_path)
            .expect("recovery points should list")
            .is_empty()
    );
}

#[test]
fn foreign_key_failures_and_malformed_markers_preserve_every_managed_file() {
    let foreign_key_path = test_database_path();
    let foreign_key = version_one_fixture(&foreign_key_path, "foreign-key-source");
    foreign_key
        .pragma_update(None, "foreign_keys", false)
        .expect("fixture foreign keys should disable");
    foreign_key
        .execute(
            "INSERT INTO branches (id, conversation_id, name, created_at_ms)
             VALUES ('dangling', 'missing', 'Main', 3)",
            [],
        )
        .expect("disabled foreign keys should allow the malformed fixture");
    drop(foreign_key);

    let foreign_key_error = ConversationStore::initialize(foreign_key_path.clone())
        .expect_err("foreign-key failure should stop preflight");
    assert_eq!(foreign_key_error.code, "migration_failed");
    assert!(conversation_exists(&foreign_key_path, "foreign-key-source"));

    let marker_path = test_database_path();
    let marker_source = version_one_fixture(&marker_path, "marker-source");
    drop(marker_source);
    let marker = migration_marker_path(&marker_path);
    fs::write(&marker, br#"{"unexpected":"field"}"#).expect("malformed marker should be written");

    let marker_error = ConversationStore::initialize(marker_path.clone())
        .expect_err("malformed marker should stop reconciliation");
    assert_eq!(marker_error.code, "migration_failed");
    assert!(conversation_exists(&marker_path, "marker-source"));
    assert!(marker.exists());
}

#[test]
fn marker_rejects_a_valid_managed_name_with_any_parent_component() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "traversal-source");
    drop(source);
    ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::AfterPromotionMarker,
    )
    .expect_err("active marker should remain");
    let active = managed_recovery_points(&path)
        .expect("active recovery should list")
        .pop()
        .expect("active recovery should exist");
    let leaf = active
        .file_name()
        .and_then(|name| name.to_str())
        .expect("recovery leaf should be UTF-8");
    let parent_copy = path
        .parent()
        .expect("database should have a parent")
        .join(leaf);
    fs::copy(&active, &parent_copy).expect("parent traversal target should exist");
    let marker = migration_marker_path(&path);
    let marker_json = fs::read_to_string(&marker).expect("marker should be readable");
    let unsafe_json = marker_json.replacen(
        &format!("\"recoveryFile\":\"{leaf}\""),
        &format!("\"recoveryFile\":\"../{leaf}\""),
        1,
    );
    fs::write(&marker, unsafe_json).expect("traversal marker should be written");

    let error = ConversationStore::initialize(path.clone())
        .expect_err("marker traversal should fail closed before file resolution");

    assert_eq!(error.code, "migration_failed");
    assert!(conversation_exists(&path, "traversal-source"));
    assert!(marker.exists());
    assert!(active.exists());
    assert!(parent_copy.exists());
}

#[test]
fn cleanup_retains_two_completed_points_active_recovery_and_unmanaged_lookalikes() {
    let path = test_database_path();
    let source = version_one_fixture(&path, "cleanup-source");
    drop(source);
    ConversationStore::initialize_with_migration_fault(
        path.clone(),
        MigrationFault::AfterPromotionMarker,
    )
    .expect_err("active marker should remain");
    let active = managed_recovery_points(&path)
        .expect("active recovery should list")
        .pop()
        .expect("active recovery should exist");
    let directory = active.parent().expect("recovery should have a parent");
    for timestamp in [i64::MAX - 2, i64::MAX - 1, i64::MAX] {
        fs::write(
            directory.join(format!(
                "bottie-migration-{timestamp}-{}-v1.sqlite3",
                uuid::Uuid::new_v4()
            )),
            b"managed fixture",
        )
        .expect("managed fixture should be written");
    }
    let unmanaged = directory.join("bottie-migration-lookalike.sqlite3");
    fs::write(&unmanaged, b"unmanaged").expect("unmanaged lookalike should be written");

    prune_recovery_points(&path).expect("strict recovery rotation should succeed");
    let retained = managed_recovery_points(&path).expect("retained points should list");

    assert_eq!(retained.len(), 3);
    assert!(retained.contains(&active));
    assert!(unmanaged.exists());
}

/// Creates a version-one store and leaves one committed conversation in its WAL.
fn version_one_fixture(path: &std::path::Path, conversation_id: &str) -> Connection {
    let connection = Connection::open(path).expect("fixture database should open");
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .expect("WAL should enable");
    connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .expect("automatic checkpoints should disable");
    connection
        .execute_batch(MIGRATION_1)
        .expect("foundation migration should apply");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms)
             VALUES (1, 'storage foundation', 1)",
            [],
        )
        .expect("foundation migration should be recorded");
    connection
        .execute(
            "INSERT INTO profiles (id, name, created_at_ms) VALUES (?1, ?2, 1)",
            params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME],
        )
        .expect("local profile should be inserted");
    connection
        .execute(
            "INSERT INTO conversations
             (id, profile_id, title, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, 'Migration fixture', 2, 2)",
            params![conversation_id, DEFAULT_PROFILE_ID],
        )
        .expect("conversation should be committed");
    connection
        .execute(
            "INSERT INTO branches (id, conversation_id, name, created_at_ms)
             VALUES ('fixture-branch', ?1, 'Main', 2)",
            [conversation_id],
        )
        .expect("branch should be committed");
    connection
        .pragma_update(None, "user_version", 1)
        .expect("fixture version should be set");
    connection
}

/// Creates one application-private attachment-like file that migrations must never touch.
fn attachment_fixture(database: &std::path::Path) -> std::path::PathBuf {
    let path = database
        .parent()
        .expect("database should have a parent")
        .join("attachments")
        .join("unchanged.bin");
    fs::create_dir_all(path.parent().expect("attachment should have a parent"))
        .expect("attachment directory should exist");
    fs::write(&path, b"unchanged").expect("attachment fixture should be written");
    path
}

/// Reads one database schema version through an independent connection.
fn database_version(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .expect("database should reopen")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version should load")
}

/// Returns whether a source conversation identity survived independently.
fn conversation_exists(path: &std::path::Path, conversation_id: &str) -> bool {
    Connection::open(path)
        .expect("database should reopen")
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM conversations WHERE id = ?1)",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("conversation identity should load")
}

/// Returns whether the database parent still contains a strict candidate artifact.
fn has_candidate_file(database: &std::path::Path) -> bool {
    fs::read_dir(database.parent().expect("database should have a parent"))
        .expect("database parent should list")
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".bottie-migration-candidate-"))
        })
}
