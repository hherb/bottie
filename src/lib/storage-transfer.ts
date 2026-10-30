/** Path-redacted contracts for native backup, restore, and conversation export flows. */

/** Native Save-dialog result that deliberately omits the selected filesystem path. */
export type ConversationExportOutcome = {
  status: "saved" | "cancelled";
  fileName: string | null;
};

/** Portable selected and multi-conversation formats supported by native Save flows. */
export type ConversationExportFormat = "markdown" | "json" | "batch-json";

/** Native backup result that deliberately omits the selected filesystem path. */
export type BackupOutcome = {
  status: "saved" | "cancelled";
  fileName: string | null;
  /** Catalogued attachment or image files left out because they were missing from disk. */
  missingFiles: number;
};

/** Native restore result that returns leaf filenames but never filesystem paths. */
export type RestoreOutcome = {
  status: "restored" | "cancelled";
  fileName: string | null;
  preservedCopyName: string | null;
  /** Catalogued attachment or image files the restored data has no bytes for. */
  missingFiles: number;
  /** A background worker has stopped, so restored attachments and search wait for a restart. */
  backgroundWorkStopped: boolean;
};

/** Appends a missing-file warning to backup or restore feedback when any file was left out. */
export function missingFilesNote(missingFiles: number): string {
  if (missingFiles === 0) return "";
  return ` · ${missingFiles} missing attachment or image file${missingFiles === 1 ? "" : "s"} not included`;
}

/** Appends a restart request to restore feedback when a background worker has stopped. */
export function backgroundWorkNote(backgroundWorkStopped: boolean): string {
  return backgroundWorkStopped ? " · Restart Bottie to finish processing restored data" : "";
}

/** Builds compact success feedback from a path-redacted native export outcome. */
export function conversationExportFeedback(format: ConversationExportFormat, fileName: string | null): string {
  const fallback = format === "markdown" ? "Markdown export" : format === "json" ? "JSON export" : "all conversations";
  return `Saved ${fileName ?? fallback}`;
}
