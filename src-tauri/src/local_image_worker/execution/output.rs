//! Native output-directory containment for private local-image worker PNGs.

use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use super::LocalImageExecutionError;

const MAX_LOCAL_OUTPUT_BYTES: u64 = 25 * 1_024 * 1_024;

/// One worker-created PNG retained only long enough for shared native normalization.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LocalGeneratedOutput {
    path: PathBuf,
    width: u32,
    height: u32,
    seed: u64,
}

impl LocalGeneratedOutput {
    /// Creates a worker-output handle for shared-normalization tests only.
    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf, width: u32, height: u32, seed: u64) -> Self {
        Self {
            path,
            width,
            height,
            seed,
        }
    }

    /// Returns the private worker output location for immediate native normalization.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the worker-reported dimensions already correlated to the accepted request.
    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns the exact generation seed correlated by the worker manager.
    pub(crate) fn seed(&self) -> u64 {
        self.seed
    }
}

impl Drop for LocalGeneratedOutput {
    /// Removes worker output bytes after normalization, failure, or cancellation.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) fn validate_outputs(
    output_directory: &Path,
    outputs: Vec<super::super::protocol::WorkerOutput>,
) -> Result<Vec<LocalGeneratedOutput>, LocalImageExecutionError> {
    let expected_names = outputs
        .iter()
        .map(|output| output.output_name.as_str())
        .collect::<HashSet<_>>();
    let entries = fs::read_dir(output_directory)
        .map_err(|_| LocalImageExecutionError::InvalidOutput)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LocalImageExecutionError::InvalidOutput)?;
    if entries.len() != outputs.len()
        || entries.iter().any(|entry| {
            !entry
                .file_name()
                .to_str()
                .is_some_and(|name| expected_names.contains(name))
        })
    {
        return Err(LocalImageExecutionError::InvalidOutput);
    }
    outputs
        .into_iter()
        .map(|output| {
            let path = output_directory.join(&output.output_name);
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| LocalImageExecutionError::InvalidOutput)?;
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() == 0
                || metadata.len() > MAX_LOCAL_OUTPUT_BYTES
                || file_has_multiple_links(&metadata)
            {
                return Err(LocalImageExecutionError::InvalidOutput);
            }
            Ok(LocalGeneratedOutput {
                path,
                width: output.width,
                height: output.height,
                seed: output.seed.ok_or(LocalImageExecutionError::InvalidOutput)?,
            })
        })
        .collect()
}

#[cfg(unix)]
fn file_has_multiple_links(metadata: &fs::Metadata) -> bool {
    metadata.nlink() != 1
}

#[cfg(not(unix))]
fn file_has_multiple_links(_metadata: &fs::Metadata) -> bool {
    false
}

pub(super) fn create_output_directory(parent: &Path) -> Result<PathBuf, LocalImageExecutionError> {
    fs::create_dir_all(parent).map_err(|_| LocalImageExecutionError::InvalidLayout)?;
    let metadata =
        fs::symlink_metadata(parent).map_err(|_| LocalImageExecutionError::InvalidLayout)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(LocalImageExecutionError::InvalidLayout);
    }
    let output = parent.join(format!("local-worker-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&output).map_err(|_| LocalImageExecutionError::InvalidLayout)?;
    Ok(output)
}

pub(super) fn prepare_empty_output_directory(path: &Path) -> Result<(), LocalImageExecutionError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| LocalImageExecutionError::InvalidLayout)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(LocalImageExecutionError::InvalidLayout);
    }
    cleanup_output_directory(path);
    if fs::read_dir(path)
        .map_err(|_| LocalImageExecutionError::InvalidLayout)?
        .next()
        .is_some()
    {
        return Err(LocalImageExecutionError::InvalidLayout);
    }
    Ok(())
}

pub(super) fn cleanup_output_directory(path: &Path) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.file_type().is_dir()) {
            let _ = fs::remove_file(path);
        }
    }
}

pub(super) fn validate_absolute_path(path: &Path) -> Result<(), LocalImageExecutionError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        Err(LocalImageExecutionError::InvalidLayout)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::protocol::WorkerOutput;
    use super::*;

    #[test]
    fn accepts_only_the_exact_regular_single_link_output_set() {
        let root = test_root("exact");
        let output_directory = root.join("outputs");
        fs::create_dir_all(&output_directory).unwrap();
        let output_path = output_directory.join("output.png");
        fs::write(&output_path, b"bounded fixture bytes").unwrap();

        let outputs = validate_outputs(&output_directory, vec![worker_output("output.png")])
            .expect("one exact regular file should pass filesystem validation");

        assert_eq!(outputs[0].path(), output_path);
        drop(outputs);
        assert!(
            !output_path.exists(),
            "dropping the handle removes source bytes"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_extra_entries_and_directories_without_removing_them() {
        for (label, prepare) in [
            ("extra", prepare_extra_entry as fn(&Path)),
            ("directory", prepare_directory_entry as fn(&Path)),
        ] {
            let root = test_root(label);
            let output_directory = root.join("outputs");
            fs::create_dir_all(&output_directory).unwrap();
            prepare(&output_directory);

            assert_eq!(
                validate_outputs(&output_directory, vec![worker_output("output.png")]),
                Err(LocalImageExecutionError::InvalidOutput)
            );
            assert!(output_directory.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_and_multiply_linked_worker_outputs() {
        for (label, prepare) in [
            ("symlink", prepare_symlink as fn(&Path, &Path)),
            ("hardlink", prepare_hardlink as fn(&Path, &Path)),
        ] {
            let root = test_root(label);
            let output_directory = root.join("outputs");
            fs::create_dir_all(&output_directory).unwrap();
            let anchor = root.join("anchor.png");
            fs::write(&anchor, b"outside worker output").unwrap();
            prepare(&anchor, &output_directory.join("output.png"));

            assert_eq!(
                validate_outputs(&output_directory, vec![worker_output("output.png")]),
                Err(LocalImageExecutionError::InvalidOutput)
            );
            assert_eq!(fs::read(&anchor).unwrap(), b"outside worker output");
            fs::remove_dir_all(root).unwrap();
        }
    }

    /// Creates one unique native test root without relying on product layout.
    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "bottie-local-output-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    /// Creates one manager-correlated fixture output description.
    fn worker_output(name: &str) -> WorkerOutput {
        WorkerOutput {
            output_name: name.into(),
            width: 512,
            height: 512,
            seed: Some(42),
        }
    }

    /// Creates the expected file plus an undeclared sibling.
    fn prepare_extra_entry(directory: &Path) {
        fs::write(directory.join("output.png"), b"expected").unwrap();
        fs::write(directory.join("extra.png"), b"extra").unwrap();
    }

    /// Creates a directory where a regular output file is required.
    fn prepare_directory_entry(directory: &Path) {
        fs::create_dir(directory.join("output.png")).unwrap();
    }

    #[cfg(unix)]
    /// Creates a symbolic link from the worker output name to an external anchor.
    fn prepare_symlink(anchor: &Path, output: &Path) {
        std::os::unix::fs::symlink(anchor, output).unwrap();
    }

    #[cfg(unix)]
    /// Creates a second hard link whose shared inode must fail closed.
    fn prepare_hardlink(anchor: &Path, output: &Path) {
        fs::hard_link(anchor, output).unwrap();
    }
}
