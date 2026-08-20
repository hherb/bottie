/** Reactive draft state for native-owned attachment ingestion. */

import { isTauri } from "@tauri-apps/api/core";

import { formatBytes } from "$lib/chat";
import type { Attachment } from "$lib/presentation";
import {
  ingestAttachments,
  mergeIngestedAttachments,
  storageErrorFromUnknown,
  type AttachmentIngestOutcome,
} from "$lib/storage";

const PREVIEW_ATTACHMENTS: Attachment[] = [
  {
    id: "preview-notes",
    name: "bottie-notes.md",
    size: "18 KB",
    kind: "file",
    mimeType: "text/plain",
    sha256: "preview",
  },
  {
    id: "preview-architecture",
    name: "architecture.png",
    size: "1.2 MB",
    kind: "image",
    mimeType: "image/png",
    sha256: "preview",
  },
];

/** Owns current-draft attachment metadata and path-redacted picker feedback. */
export class AttachmentState {
  items = $state<Attachment[]>(isTauri() ? [] : PREVIEW_ATTACHMENTS.map((item) => ({ ...item })));
  isIngesting = $state(false);
  feedback = $state<string | null>(null);
  failed = $state(false);

  private browserInput?: HTMLInputElement;

  /** Registers the browser-preview file input after its component mounts. */
  setBrowserInput(element: HTMLInputElement): void {
    this.browserInput = element;
  }

  /** Opens native ingestion or the metadata-only browser-preview picker. */
  async openPicker(): Promise<void> {
    if (!isTauri()) {
      this.browserInput?.click();
      return;
    }
    if (this.isIngesting) return;
    this.isIngesting = true;
    this.feedback = null;
    this.failed = false;
    try {
      const outcome = await ingestAttachments();
      if (outcome.status === "cancelled") return;
      const priorIds = new Set(this.items.map((item) => item.id));
      this.items = mergeIngestedAttachments(this.items, outcome.attachments);
      this.feedback = attachmentOutcomeFeedback(outcome, priorIds);
      this.failed = outcome.rejections.length > 0;
    } catch (error) {
      this.feedback = storageErrorFromUnknown(error).message;
      this.failed = true;
    } finally {
      this.isIngesting = false;
    }
  }

  /** Adds metadata-only fixtures when exercising the disconnected browser preview. */
  addBrowserFiles(event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    for (const file of Array.from(input.files ?? [])) {
      this.items.push({
        id: `preview-${Date.now()}-${this.items.length}`,
        name: file.name,
        size: formatBytes(file.size),
        kind: file.type.startsWith("image/") ? "image" : "file",
        mimeType: file.type || "application/octet-stream",
        sha256: "preview",
      });
    }
    input.value = "";
  }

  /** Removes one attachment from the current draft without deleting retained content. */
  remove(id: string): void {
    this.items = this.items.filter((attachment) => attachment.id !== id);
  }

  /** Clears draft association while leaving native content available for deduplication. */
  clear(): void {
    this.items = [];
    this.feedback = null;
    this.failed = false;
  }

  /** Explains the intentionally absent provider-delivery step. */
  explainUnavailableDelivery(): void {
    this.feedback = "Remove attachments before sending; provider delivery is not available yet.";
    this.failed = false;
  }
}

/** Summarizes accepted, reused, already-selected, and rejected native results. */
function attachmentOutcomeFeedback(outcome: AttachmentIngestOutcome, priorIds: Set<string>): string {
  const unique = new Map<string, (typeof outcome.attachments)[number]>();
  for (const attachment of outcome.attachments) {
    if (!unique.has(attachment.id)) unique.set(attachment.id, attachment);
  }
  const added = Array.from(unique.values()).filter((attachment) => !priorIds.has(attachment.id));
  const alreadySelected = Array.from(unique.values()).filter((attachment) => priorIds.has(attachment.id)).length;
  const reused = added.filter((attachment) => attachment.duplicate).length;
  const parts = [];
  if (added.length > 0) parts.push(`${added.length} stored locally`);
  if (reused > 0) parts.push(`${reused} reused`);
  if (alreadySelected > 0) parts.push(`${alreadySelected} already selected`);
  if (outcome.rejections.length > 0) parts.push(`${outcome.rejections.length} rejected`);
  return parts.join(" · ") || "No attachments changed";
}
