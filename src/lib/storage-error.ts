/** Pure normalization for failures returned by native conversation storage commands. */

import type { StorageError } from "./storage";

/** Converts an unknown native error into Bottie's stable storage error shape. */
export function storageErrorFromUnknown(error: unknown): StorageError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const candidate = error as Partial<StorageError>;
    if (typeof candidate.message === "string") {
      return { code: candidate.code ?? "internal", message: candidate.message };
    }
  }
  return {
    code: "internal",
    message: typeof error === "string" ? error : "Bottie could not access local conversation history.",
  };
}
