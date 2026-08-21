/** Reactive draft state for native-owned attachment ingestion. */

import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { ATTACHMENT_PROCESSING_EVENT, applyAttachmentProcessingUpdate } from "$lib/attachment";
import { draftImageDeliveryBlocker, formatBytes } from "$lib/chat";
import type { ModelInfo } from "$lib/inference";
import type { Attachment } from "$lib/presentation";
import {
  ingestAttachments,
  mergeIngestedAttachments,
  storageErrorFromUnknown,
  type AttachmentIngestOutcome,
  type StoredAttachment,
} from "$lib/storage";

const PREVIEW_ATTACHMENTS: Attachment[] = [
  {
    id: "preview-notes",
    name: "bottie-notes.md",
    size: "18 KB",
    kind: "file",
    mimeType: "text/plain",
    sha256: "preview",
    extraction: { state: "ready", format: "markdown", characterCount: 18_432, pageCount: null, errorCode: null },
    indexing: { state: "indexable" },
    normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
  },
  {
    id: "preview-architecture",
    name: "architecture.png",
    size: "1.2 MB",
    kind: "image",
    mimeType: "image/png",
    sha256: "preview",
    extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
    indexing: { state: "unsupported" },
    normalization: { state: "ready", format: "png", width: 1_440, height: 900, byteSize: 1_048_576, errorCode: null },
  },
  {
    id: "preview-field-guide",
    name: "field-guide.pdf",
    size: "860 KB",
    kind: "file",
    mimeType: "application/pdf",
    sha256: "preview",
    extraction: { state: "ready", format: "pdf", characterCount: 24_180, pageCount: 12, errorCode: null },
    indexing: { state: "indexable" },
    normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
  },
  {
    id: "preview-review-notes",
    name: "review-notes.docx",
    size: "46 KB",
    kind: "file",
    mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    sha256: "preview",
    extraction: { state: "ready", format: "docx", characterCount: 8_420, pageCount: null, errorCode: null },
    indexing: { state: "indexable" },
    normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
  },
];

/** Owns current-draft attachment metadata and path-redacted picker feedback. */
export class AttachmentState {
  items = $state<Attachment[]>(isTauri() ? [] : PREVIEW_ATTACHMENTS.map((item) => ({ ...item })));
  isIngesting = $state(false);
  feedback = $state<string | null>(null);
  failed = $state(false);

  private browserInput?: HTMLInputElement;
  private queuedProcessingUpdates = new Map<string, StoredAttachment>();
  private submittedItems: Attachment[] | null = null;
  private stopProcessingUpdates?: UnlistenFn;

  /** Returns whether the current draft satisfies native image-delivery prerequisites. */
  canSubmit(model: ModelInfo | undefined, conversationItems: Attachment[] = []): boolean {
    return draftImageDeliveryBlocker([...this.items, ...conversationItems], model) === null;
  }

  /** Listens for path-free native processing results throughout the page lifecycle. */
  async listenForProcessingUpdates(onUpdate: (update: StoredAttachment) => void): Promise<void> {
    try {
      this.stopProcessingUpdates = await listen<StoredAttachment>(ATTACHMENT_PROCESSING_EVENT, (event) => {
        this.applyProcessingUpdate(event.payload);
        onUpdate(event.payload);
      });
    } catch (error) {
      console.warn("Could not listen for attachment processing updates", error);
    }
  }

  /** Releases the native processing listener when the page is unmounted. */
  dispose(): void {
    this.stopProcessingUpdates?.();
    this.stopProcessingUpdates = undefined;
  }

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
      for (const attachment of outcome.attachments) {
        const update = this.queuedProcessingUpdates.get(attachment.id);
        if (!update) continue;
        this.items = applyAttachmentProcessingUpdate(this.items, update);
        this.queuedProcessingUpdates.delete(attachment.id);
      }
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
        extraction: {
          state: file.type.startsWith("text/") ? "ready" : "unsupported",
          format: file.name.toLowerCase().endsWith(".md")
            ? "markdown"
            : file.type.startsWith("text/")
              ? "plain_text"
              : null,
          characterCount: null,
          pageCount: null,
          errorCode: null,
        },
        indexing: {
          state: file.type.startsWith("text/") ? "indexable" : "unsupported",
        },
        normalization: {
          state: "unsupported",
          format: null,
          width: null,
          height: null,
          byteSize: null,
          errorCode: null,
        },
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

  /** Applies or temporarily queues one path-free native background-processing update. */
  applyProcessingUpdate(update: StoredAttachment): void {
    if (this.submittedItems) {
      this.submittedItems = applyAttachmentProcessingUpdate(this.submittedItems, update);
    }
    const updated = applyAttachmentProcessingUpdate(this.items, update);
    if (updated === this.items) {
      if (this.isIngesting) this.queuedProcessingUpdates.set(update.id, update);
      return;
    }
    this.items = updated;
  }

  /** Captures the current draft while background events may still update its processing state. */
  beginSubmission(): Attachment[] {
    this.submittedItems = this.items.map((attachment) => ({ ...attachment }));
    return this.submittedItems;
  }

  /** Returns the latest submitted metadata and closes the in-flight capture. */
  finishSubmission(): Attachment[] {
    const submitted = this.submittedItems ?? [];
    this.submittedItems = null;
    return submitted;
  }

  /** Closes an unsuccessful submission while leaving the draft intact. */
  cancelSubmission(): void {
    this.submittedItems = null;
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
