//! One-pass hashing and durable append support for asynchronous model downloads.

use std::{fs::File, io::Write, path::PathBuf};

use sha2::{Digest, Sha256};

use super::{
    CacheError, CacheWriteStatus, ModelFileContract, hash_prefix, open_for_append,
    remove_failed_file, sync_parent,
};

/// Stateful writer that hashes a retained prefix once and accepts bounded response chunks.
pub(crate) struct CacheFileWriter {
    path: PathBuf,
    destination: Option<File>,
    hasher: Option<Sha256>,
    total: u64,
    expected_size: u64,
    expected_sha256: String,
    settled: bool,
}

impl CacheFileWriter {
    /// Opens an exact regular partial at its already validated durable offset.
    pub(super) fn open(
        path: PathBuf,
        offset: u64,
        contract: &ModelFileContract,
    ) -> Result<Self, CacheError> {
        let hasher = hash_prefix(&path, offset)?;
        let destination = open_for_append(&path, offset)?;
        Ok(Self {
            path,
            destination: Some(destination),
            hasher: Some(hasher),
            total: offset,
            expected_size: contract.byte_size,
            expected_sha256: contract.sha256.clone(),
            settled: false,
        })
    }

    /// Appends one response chunk while enforcing the exact file byte ceiling.
    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), CacheError> {
        let next_total = self
            .total
            .checked_add(bytes.len() as u64)
            .ok_or(CacheError::Integrity)?;
        if next_total > self.expected_size {
            return self.remove_untrusted(CacheError::Integrity);
        }
        if self
            .destination
            .as_mut()
            .ok_or(CacheError::InvalidState)?
            .write_all(bytes)
            .is_err()
        {
            return self.remove_untrusted(CacheError::Storage);
        }
        self.hasher
            .as_mut()
            .ok_or(CacheError::InvalidState)?
            .update(bytes);
        self.total = next_total;
        Ok(())
    }

    /// Syncs all appended bytes and their directory entry before progress is observable.
    pub(crate) fn sync_progress(&mut self) -> Result<u64, CacheError> {
        if self
            .destination
            .as_mut()
            .ok_or(CacheError::InvalidState)?
            .sync_all()
            .is_err()
        {
            return self.remove_untrusted(CacheError::Storage);
        }
        sync_parent(&self.path)?;
        Ok(self.total)
    }

    /// Retains a valid synced prefix and returns the caller's interruption classification.
    pub(crate) fn retain_partial(
        mut self,
        error: CacheError,
    ) -> Result<CacheWriteStatus, CacheError> {
        self.sync_progress()?;
        self.close();
        Err(error)
    }

    /// Returns the in-process byte count, which is not durable progress until `sync_progress` succeeds.
    pub(crate) fn appended_bytes(&self) -> u64 {
        self.total
    }

    /// Syncs and closes the file, then verifies the exact digest only when complete.
    pub(crate) fn finish(mut self) -> Result<CacheWriteStatus, CacheError> {
        self.sync_progress()?;
        self.close_destination();
        if self.total < self.expected_size {
            self.settled = true;
            return Ok(CacheWriteStatus::Incomplete);
        }
        let hasher = self.hasher.take().ok_or(CacheError::InvalidState)?;
        if format!("{:x}", hasher.finalize()) != self.expected_sha256 {
            return self.remove_untrusted(CacheError::Integrity);
        }
        self.settled = true;
        Ok(CacheWriteStatus::Complete)
    }

    fn remove_untrusted<T>(&mut self, error: CacheError) -> Result<T, CacheError> {
        self.close_destination();
        self.settled = true;
        remove_failed_file(&self.path)?;
        Err(error)
    }

    fn close(&mut self) {
        self.close_destination();
        self.settled = true;
    }

    fn close_destination(&mut self) {
        drop(self.destination.take());
    }
}

impl Drop for CacheFileWriter {
    /// Best-effort syncs an otherwise valid prefix if its future is externally dropped.
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        if let Some(destination) = self.destination.as_mut() {
            let _ = destination.sync_all();
        }
        self.close_destination();
        let _ = sync_parent(&self.path);
    }
}
