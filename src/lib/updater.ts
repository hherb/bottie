/** Path-free WebView contracts for Bottie's Rust-owned production updater. */

import { invoke } from "@tauri-apps/api/core";

/** Bounded metadata returned after one explicit native update check. */
export type UpdateCheckResult = {
  status: "noUpdate" | "updateAvailable";
  currentVersion: string;
  version: string | null;
  notes: string | null;
};

/** Successful native installer acceptance for the reviewed version. */
export type UpdateInstallResult = {
  status: "installed";
  version: string;
};

/** Fixed error shape emitted after native updater details are redacted. */
export type UpdateError = {
  code: string;
  message: string;
  retryable: boolean;
};

/** Checks Bottie's one fixed native update endpoint. */
export async function checkForUpdate(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>("check_for_update");
}

/** Installs only the exact candidate returned by the preceding explicit check. */
export async function installUpdate(): Promise<UpdateInstallResult> {
  return invoke<UpdateInstallResult>("install_update");
}

/** Requests cancellation of the one active native update operation. */
export async function cancelUpdateOperation(): Promise<boolean> {
  const result = await invoke<{ cancellationRequested: boolean }>("cancel_update_operation");
  return result.cancellationRequested;
}

/** Reduces unexpected IPC failures to the same path-free presentation contract. */
export function updateErrorFromUnknown(error: unknown): UpdateError {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string" &&
    "code" in error &&
    typeof error.code === "string"
  ) {
    return {
      code: error.code,
      message: error.message,
      retryable: "retryable" in error && error.retryable === true,
    };
  }
  return { code: "unavailable", message: "Bottie could not complete the update action.", retryable: true };
}
