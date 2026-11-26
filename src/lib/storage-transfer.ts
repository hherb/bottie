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
  /** Catalogued attachment or image files left out because they were absent from disk. */
  missingFiles: number;
  /** Catalogued attachment or image files left out because they failed their checksum (possible disk damage). */
  damagedFiles: number;
};

/** Native restore result that returns leaf filenames but never filesystem paths. */
export type RestoreOutcome = {
  status: "restored" | "cancelled";
  fileName: string | null;
  preservedCopyName: string | null;
  /** Catalogued attachment or image files the restored data has no bytes for. */
  missingFiles: number;
  /** The missing-file count failed after the restore, so `missingFiles` is 0 only because it is unknown. */
  missingFilesUnchecked: boolean;
  /** A background worker has stopped, so restored attachments and search wait for a restart. */
  backgroundWorkStopped: boolean;
};

/**
 * Appends missing- and damaged-file warnings to backup or restore feedback when any file was left out.
 * Damage is named apart, since it can mean a failing disk; diagnostics carry the details (#267).
 */
export function missingFilesNote(missingFiles: number, damagedFiles = 0): string {
  const leftOut = (count: number, kind: string) =>
    ` · ${count} ${kind} attachment or image file${count === 1 ? "" : "s"} not included`;
  let note = "";
  if (missingFiles > 0) note += leftOut(missingFiles, "missing");
  if (damagedFiles > 0) note += `${leftOut(damagedFiles, "damaged")}; see Recent diagnostics in Settings`;
  return note;
}

/** Appends a warning to restore feedback when the restored data's missing files could not be counted. */
export function missingFilesUncheckedNote(missingFilesUnchecked: boolean): string {
  return missingFilesUnchecked
    ? " · Missing attachment or image files could not be checked; see Recent diagnostics in Settings"
    : "";
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
