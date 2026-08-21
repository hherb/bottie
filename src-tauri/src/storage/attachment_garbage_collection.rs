//! Restart-boundary garbage collection for application-private attachment content.

use std::{
    collections::HashSet,
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

use rusqlite::{TransactionBehavior, params};

use super::{ConversationStore, StorageError, now_ms};

const BLOB_DIRECTORY_NAME: &str = "blobs";
const NORMALIZED_DIRECTORY_NAME: &str = "normalized-images";
const INGESTION_TEMPORARY_DIRECTORY_NAME: &str = "temporary";
const NORMALIZATION_TEMPORARY_DIRECTORY_NAME: &str = "normalization-temporary";
const SHA256_HEX_LENGTH: usize = 64;
const SHA256_SHARD_LENGTH: usize = 2;
const MILLISECONDS_PER_HOUR: i64 = 60 * 60 * 1_000;
const GARBAGE_COLLECTION_GRACE_HOURS: i64 = 24;
const GARBAGE_COLLECTION_GRACE_MS: i64 = GARBAGE_COLLECTION_GRACE_HOURS * MILLISECONDS_PER_HOUR;

/// Path-free summary of one completed attachment garbage-collection pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AttachmentGarbageCollection {
    /// Unreferenced catalog rows removed with their dependent processing metadata.
    pub(crate) catalog_entries_removed: usize,
    /// Unreferenced or crash-left original blob files removed from managed storage.
    pub(crate) original_files_removed: usize,
    /// Unreferenced or crash-left normalized derivative files removed from managed storage.
    pub(crate) derivative_files_removed: usize,
    /// Interrupted ingestion or normalization files removed from dedicated temporary storage.
    pub(crate) temporary_files_removed: usize,
    /// Exact bytes reclaimed from regular files that were present when removal began.
    pub(crate) reclaimed_bytes: u64,
}

/// Database identities that must remain available after catalog pruning.
struct LiveContent {
    originals: HashSet<String>,
    derivatives: HashSet<(String, String)>,
}

/// File-removal totals shared by managed-content and temporary-directory sweeps.
#[derive(Default)]
struct RemovalTotal {
    files: usize,
    bytes: u64,
}

impl ConversationStore {
    /// Removes attachment content with no durable message or conversation reference.
    ///
    /// This runs before this process can create a draft or start its processor. A safety window
    /// protects recent work from another process. Catalog deletion commits before file removal so
    /// a crash can leave only harmless untracked files, which the next pass will sweep.
    pub(crate) fn collect_unreferenced_attachments(
        &self,
    ) -> Result<AttachmentGarbageCollection, StorageError> {
        let catalog_cutoff_ms = now_ms()?.saturating_sub(GARBAGE_COLLECTION_GRACE_MS);
        let file_cutoff = SystemTime::now()
            .checked_sub(Duration::from_millis(GARBAGE_COLLECTION_GRACE_MS as u64))
            .ok_or_else(StorageError::internal)?;
        self.collect_unreferenced_attachments_before(catalog_cutoff_ms, file_cutoff)
    }

    /// Applies collection to content no newer than the supplied safety boundaries.
    fn collect_unreferenced_attachments_before(
        &self,
        catalog_cutoff_ms: i64,
        file_cutoff: SystemTime,
    ) -> Result<AttachmentGarbageCollection, StorageError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let catalog_entries_removed = transaction.execute(
            "DELETE FROM attachments
             WHERE NOT EXISTS (
                 SELECT 1 FROM message_attachments
                 WHERE message_attachments.attachment_id = attachments.id
             )
             AND NOT EXISTS (
                 SELECT 1 FROM conversation_attachments
                 WHERE conversation_attachments.attachment_id = attachments.id
             )
             AND created_at_ms <= ?1",
            params![catalog_cutoff_ms],
        )?;
        transaction.commit()?;

        let sweep_transaction =
            connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let live_content = load_live_content(&sweep_transaction)?;
        let attachment_root = self.attachment_root();
        let originals = sweep_managed_content(
            &attachment_root.join(BLOB_DIRECTORY_NAME),
            ManagedContentKind::Original,
            file_cutoff,
            |sha256, _| live_content.originals.contains(sha256),
        )?;
        let derivatives = sweep_managed_content(
            &attachment_root.join(NORMALIZED_DIRECTORY_NAME),
            ManagedContentKind::Derivative,
            file_cutoff,
            |sha256, format| {
                format.is_some_and(|format| {
                    live_content
                        .derivatives
                        .contains(&(sha256.to_owned(), format.to_owned()))
                })
            },
        )?;
        let ingestion_temporary = clear_temporary_directory(
            &attachment_root.join(INGESTION_TEMPORARY_DIRECTORY_NAME),
            file_cutoff,
        )?;
        let normalization_temporary = clear_temporary_directory(
            &attachment_root.join(NORMALIZATION_TEMPORARY_DIRECTORY_NAME),
            file_cutoff,
        )?;
        sweep_transaction.commit()?;

        Ok(AttachmentGarbageCollection {
            catalog_entries_removed,
            original_files_removed: originals.files,
            derivative_files_removed: derivatives.files,
            temporary_files_removed: ingestion_temporary
                .files
                .saturating_add(normalization_temporary.files),
            reclaimed_bytes: originals
                .bytes
                .saturating_add(derivatives.bytes)
                .saturating_add(ingestion_temporary.bytes)
                .saturating_add(normalization_temporary.bytes),
        })
    }

    /// Collects every fixture regardless of age through the explicit storage test boundary.
    #[cfg(test)]
    pub(super) fn collect_all_unreferenced_attachments_for_test(
        &self,
    ) -> Result<AttachmentGarbageCollection, StorageError> {
        let future_cutoff = SystemTime::now()
            .checked_add(Duration::from_secs(1))
            .ok_or_else(StorageError::internal)?;
        self.collect_unreferenced_attachments_before(i64::MAX, future_cutoff)
    }
}

/// Loads every catalogued original and ready derivative after unreferenced rows are deleted.
fn load_live_content(connection: &rusqlite::Connection) -> Result<LiveContent, StorageError> {
    let mut original_statement = connection.prepare("SELECT sha256 FROM attachments")?;
    let originals = original_statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<HashSet<String>, _>>()?;
    let mut derivative_statement = connection.prepare(
        "SELECT DISTINCT normalized_sha256, format
         FROM attachment_image_normalizations WHERE state = 'ready'",
    )?;
    let derivatives = derivative_statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<HashSet<(String, String)>, _>>()?;
    Ok(LiveContent {
        originals,
        derivatives,
    })
}

/// Managed file family with a strict hash-sharded filename contract.
#[derive(Clone, Copy)]
enum ManagedContentKind {
    Original,
    Derivative,
}

/// Removes only strict managed files absent from the live SQLite identity sets.
fn sweep_managed_content(
    root: &Path,
    kind: ManagedContentKind,
    modified_before: SystemTime,
    is_live: impl Fn(&str, Option<&str>) -> bool,
) -> Result<RemovalTotal, StorageError> {
    let mut total = RemovalTotal::default();
    let Some(shards) = read_directory_if_present(root)? else {
        return Ok(total);
    };
    for shard in shards {
        let shard = shard?;
        let file_type = shard.file_type()?;
        let shard_name = shard.file_name();
        let Some(shard_name) = shard_name.to_str().filter(|name| is_hash_shard(name)) else {
            continue;
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        for entry in fs::read_dir(shard.path())? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() && !file_type.is_symlink() {
                continue;
            }
            let file_name = entry.file_name();
            let Some((sha256, format)) = managed_identity(&file_name, shard_name, kind) else {
                continue;
            };
            if is_live(sha256, format) {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.modified()? > modified_before {
                continue;
            }
            fs::remove_file(entry.path())?;
            total.files = total.files.saturating_add(1);
            if metadata.file_type().is_file() {
                total.bytes = total.bytes.saturating_add(metadata.len());
            }
        }
        remove_directory_if_empty(&shard.path())?;
    }
    Ok(total)
}

/// Decodes an original or derivative filename only when every managed-path invariant holds.
fn managed_identity<'a>(
    name: &'a std::ffi::OsStr,
    shard: &str,
    kind: ManagedContentKind,
) -> Option<(&'a str, Option<&'a str>)> {
    let name = name.to_str()?;
    let (sha256, format) = match kind {
        ManagedContentKind::Original => (name, None),
        ManagedContentKind::Derivative => {
            let (sha256, format) = name.rsplit_once('.')?;
            if !matches!(format, "jpeg" | "png") {
                return None;
            }
            (sha256, Some(format))
        }
    };
    (is_sha256(sha256) && &sha256[..SHA256_SHARD_LENGTH] == shard).then_some((sha256, format))
}

/// Removes every entry below a dedicated startup-only temporary directory without following links.
fn clear_temporary_directory(
    path: &Path,
    modified_before: SystemTime,
) -> Result<RemovalTotal, StorageError> {
    let mut total = RemovalTotal::default();
    let Some(entries) = read_directory_if_present(path)? else {
        return Ok(total);
    };
    for entry in entries {
        let entry = entry?;
        remove_temporary_entry(&entry.path(), modified_before, &mut total)?;
    }
    remove_directory_if_empty(path)?;
    Ok(total)
}

/// Recursively removes one temporary entry while never traversing a symbolic link.
fn remove_temporary_entry(
    path: &Path,
    modified_before: SystemTime,
    total: &mut RemovalTotal,
) -> Result<(), StorageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        for entry in fs::read_dir(path)? {
            remove_temporary_entry(&entry?.path(), modified_before, total)?;
        }
        remove_directory_if_empty(path)?;
        return Ok(());
    }
    if metadata.modified()? > modified_before {
        return Ok(());
    }
    fs::remove_file(path)?;
    total.files = total.files.saturating_add(1);
    if metadata.file_type().is_file() {
        total.bytes = total.bytes.saturating_add(metadata.len());
    }
    Ok(())
}

/// Opens an optional directory while preserving errors for existing unreadable paths.
fn read_directory_if_present(path: &Path) -> Result<Option<fs::ReadDir>, StorageError> {
    match fs::read_dir(path) {
        Ok(entries) => Ok(Some(entries)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Removes an empty managed directory while tolerating retained or unexpected entries.
fn remove_directory_if_empty(path: &Path) -> Result<(), StorageError> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

/// Validates one lowercase SHA-256 identity without accepting separators or alternate case.
fn is_sha256(value: &str) -> bool {
    value.len() == SHA256_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// Validates one lowercase two-character SHA-256 shard name.
fn is_hash_shard(value: &str) -> bool {
    value.len() == SHA256_SHARD_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_identity_rejects_wrong_shards_formats_and_case() {
        let hash = "a".repeat(SHA256_HEX_LENGTH);
        let png_name = format!("{hash}.png");
        let webp_name = format!("{hash}.webp");
        let uppercase_hash = hash.to_uppercase();
        assert_eq!(
            managed_identity(
                std::ffi::OsStr::new(&png_name),
                "aa",
                ManagedContentKind::Derivative
            ),
            Some((hash.as_str(), Some("png")))
        );
        assert!(
            managed_identity(
                std::ffi::OsStr::new(&webp_name),
                "aa",
                ManagedContentKind::Derivative
            )
            .is_none()
        );
        assert!(
            managed_identity(
                std::ffi::OsStr::new(&uppercase_hash),
                "aa",
                ManagedContentKind::Original
            )
            .is_none()
        );
        assert!(
            managed_identity(
                std::ffi::OsStr::new(&hash),
                "ab",
                ManagedContentKind::Original
            )
            .is_none()
        );
    }
}
