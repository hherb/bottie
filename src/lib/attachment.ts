/** Pure path-free presentation policy for retained attachment processing state. */

import type { Attachment, Message } from "./presentation";
import {
  attachmentExtractionLabel,
  storedAttachmentToPresentation,
  type AttachmentExtraction,
  type AttachmentIndexing,
  type ImageNormalization,
  type StoredAttachment,
} from "./storage";

/** Native event carrying path-free metadata after durable background processing. */
export const ATTACHMENT_PROCESSING_EVENT = "attachment-processing-updated";

const IMAGE_NORMALIZATION_FAILURE_LABELS: Record<string, string> = {
  image_decode_failed: "Image could not be decoded",
  image_decode_limit_exceeded: "Image decoding exceeds local limit",
  image_dimension_limit_exceeded: "Image dimensions exceed local limit",
  image_missing_content: "Retained image content is unavailable",
  image_output_too_large: "Normalized image exceeds local size limit",
  image_pixel_limit_exceeded: "Image pixel count exceeds local limit",
  image_write_failed: "Image normalization could not be saved",
};

/** One explicit local-only consequence for failed attachment preparation. */
export type AttachmentFailure = {
  title: string;
  detail: string;
};

/** Describes a failed extraction or normalization without exposing native diagnostics. */
export function attachmentFailure(attachment: Attachment): AttachmentFailure | null {
  if (attachment.normalization.state === "failed") {
    return {
      title:
        IMAGE_NORMALIZATION_FAILURE_LABELS[attachment.normalization.errorCode ?? ""] ?? "Image normalization failed",
      detail: "The original file is still stored locally, but Bottie cannot preview or send this image.",
    };
  }
  if (attachment.extraction.state === "failed") {
    return {
      title: attachmentExtractionLabel(attachment.extraction),
      detail: "The original file is still stored locally, but its text is unavailable for later indexing.",
    };
  }
  return null;
}

/** Describes image normalization, falling back to text extraction for non-image attachments. */
export function attachmentStatusLabel(
  normalization: ImageNormalization,
  extraction?: AttachmentExtraction,
  indexing?: AttachmentIndexing,
): string {
  if (normalization.state === "ready") {
    const format = normalization.format === "jpeg" ? "JPEG" : "PNG";
    return `${format} normalized locally · ${normalization.width ?? 0} × ${normalization.height ?? 0}`;
  }
  if (normalization.state === "pending") return "Image normalization pending";
  if (normalization.state === "failed") {
    return IMAGE_NORMALIZATION_FAILURE_LABELS[normalization.errorCode ?? ""] ?? "Image normalization failed";
  }
  if (!extraction) return "No image normalization";
  const extractionLabel = attachmentExtractionLabel(extraction);
  return indexing?.state === "indexable" ? `${extractionLabel} · Ready for indexing` : extractionLabel;
}

/** Replaces one matching attachment with its latest path-free native processing state. */
export function applyAttachmentProcessingUpdate(current: Attachment[], updated: StoredAttachment): Attachment[] {
  if (!current.some((attachment) => attachment.id === updated.id)) return current;
  const presentation = storedAttachmentToPresentation(updated);
  return current.map((attachment) => (attachment.id === updated.id ? presentation : attachment));
}

/** Applies one native processing result to matching attachments in visible messages. */
export function applyAttachmentProcessingUpdateToMessages(messages: Message[], updated: StoredAttachment): Message[] {
  return messages.map((message) => {
    if (!message.attachments) return message;
    const attachments = applyAttachmentProcessingUpdate(message.attachments, updated);
    return attachments === message.attachments ? message : { ...message, attachments };
  });
}
