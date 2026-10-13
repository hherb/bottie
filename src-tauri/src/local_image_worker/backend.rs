//! Fresh native capabilities for either an installed image server or an exact private worker.

use super::{
    execution::VerifiedLocalImageInstallation,
    server::{IMAGE_PACKAGE_ID, LocalImageServer},
};

/// One explicitly verified source for a local generation; paths remain in the worker variant.
#[derive(Debug)]
pub(crate) enum VerifiedLocalImageBackend {
    /// The existing hash-verified, app-owned 2512 runtime.
    Worker(VerifiedLocalImageInstallation),
    /// The exact 2.1 package advertised by the loopback MLX image server.
    Server(LocalImageServer),
}

impl VerifiedLocalImageBackend {
    /// Returns the precise model-package provenance for start and retry validation.
    pub(crate) fn model_id(&self) -> &str {
        match self {
            Self::Worker(worker) => worker.model_id(),
            Self::Server(_) => IMAGE_PACKAGE_ID,
        }
    }
}
