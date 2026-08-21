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
};

/** Native restore result that returns leaf filenames but never filesystem paths. */
export type RestoreOutcome = {
  status: "restored" | "cancelled";
  fileName: string | null;
  preservedCopyName: string | null;
};

/** Builds compact success feedback from a path-redacted native export outcome. */
export function conversationExportFeedback(format: ConversationExportFormat, fileName: string | null): string {
  const fallback = format === "markdown" ? "Markdown export" : format === "json" ? "JSON export" : "all conversations";
  return `Saved ${fileName ?? fallback}`;
}
