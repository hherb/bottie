//! Native-only discovery of an explicitly provisioned Nano runtime and embedded worker sources.

use crate::speech::SpeechErrorCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: u64 = 4096;
const WORKER_FILES: &[(&str, &str)] = &[
    (
        "worker.py",
        include_str!("../../../../scripts/moss-nano-spike/worker.py"),
    ),
    (
        "assets.py",
        include_str!("../../../../scripts/moss-nano-spike/assets.py"),
    ),
    (
        "contract.py",
        include_str!("../../../../scripts/moss-nano-spike/contract.py"),
    ),
    (
        "assets.lock.json",
        include_str!("../../../../scripts/moss-nano-spike/assets.lock.json"),
    ),
    (
        "bootstrap.py",
        "import sys\nfrom pathlib import Path\nsys.path.insert(0,str(Path(__file__).resolve().parent))\nfrom worker import main\nmain()\n",
    ),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeConfiguration {
    python_path: PathBuf,
    asset_directory: PathBuf,
}

/// Exact native paths never exposed through IPC or editable by a WebView request.
#[derive(Clone, Debug)]
pub(super) struct NanoRuntime {
    pub(super) python: PathBuf,
    pub(super) assets: PathBuf,
    worker_directory: PathBuf,
}

impl NanoRuntime {
    /// Detects only a user-provisioned installation; discovery neither downloads nor starts Python.
    pub(super) fn discover(app_data: &Path) -> Option<Self> {
        let root = app_data.join("nano-speech");
        let config_path = root.join("runtime.json");
        if fs::metadata(&config_path).ok()?.len() > MAX_CONFIG_BYTES {
            return None;
        }
        let config: RuntimeConfiguration =
            serde_json::from_slice(&fs::read(config_path).ok()?).ok()?;
        if !config.python_path.is_absolute()
            || !config.asset_directory.is_absolute()
            || !config.python_path.is_file()
            || !config.asset_directory.is_dir()
        {
            return None;
        }
        let mut digest = Sha256::new();
        for (name, source) in WORKER_FILES {
            digest.update(name);
            digest.update(source);
        }
        Some(Self {
            python: config.python_path,
            assets: config.asset_directory,
            worker_directory: root
                .join("workers")
                .join(format!("{:x}", digest.finalize())),
        })
    }

    /// Materializes this binary's exact worker scripts and verifies existing copies before execution.
    pub(super) fn prepare_worker(&self) -> Result<PathBuf, SpeechErrorCode> {
        fs::create_dir_all(&self.worker_directory).map_err(|_| SpeechErrorCode::Unavailable)?;
        for (name, source) in WORKER_FILES {
            let path = self.worker_directory.join(name);
            if fs::read(&path).ok().as_deref() != Some(source.as_bytes()) {
                let temporary = self
                    .worker_directory
                    .join(format!("{name}.{}.partial", uuid::Uuid::new_v4()));
                fs::write(&temporary, source).map_err(|_| SpeechErrorCode::Unavailable)?;
                if fs::rename(&temporary, &path).is_err() {
                    let _ = fs::remove_file(temporary);
                    return Err(SpeechErrorCode::Unavailable);
                }
            }
        }
        Ok(self.worker_directory.join("bootstrap.py"))
    }
}
