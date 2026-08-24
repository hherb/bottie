//! Non-blocking application-start orchestration for native credential warming.

use std::sync::Arc;

use crate::{
    credentials::SystemCredentialStore,
    diagnostics::{Diagnostics, record_diagnostic},
};

/// Warms configured vault secrets behind one app-start authentication without delaying launch.
pub(crate) fn schedule_session_unlock(
    credentials: Arc<SystemCredentialStore>,
    diagnostics: Diagnostics,
) {
    tauri::async_runtime::spawn(async move {
        let unlock = tauri::async_runtime::spawn_blocking(move || credentials.warm_session()).await;
        let (level, event, detail) = match unlock {
            Ok(Ok(0)) => return,
            Ok(Ok(_)) => (
                "info",
                "Credential session unlocked",
                "Configured credentials are retained only in native process memory",
            ),
            Ok(Err(_)) | Err(_) => (
                "warning",
                "Credential session remained locked",
                "Bottie could not warm the configured credential vault at startup",
            ),
        };
        record_diagnostic(&diagnostics, level, event, None, Some(detail)).await;
    });
}
