/** Pure presentation contracts for explicit Cloud and local image generation. */

import type { ImageEditingSourceRequest } from "./inference";
import type { Attachment, GeneratedAsset } from "./presentation";
import type { StoredGeneratedImageSource } from "./storage";
import { formatBytes } from "./chat";

/** User-selected execution boundary for a newly submitted image request. */
export type ImageGenerationExecution = "cloud" | "local";

/** Fixed Qwen-Image-2.0 size choices exposed by the composer. */
export type ImageGenerationSize = "square" | "landscape" | "portrait";

/** Current generated-image reference selection exposed to conversation presentation. */
export type ImageEditingPresentation = {
  active: boolean;
  execution: ImageGenerationExecution;
  selectedSourceIds: string[];
  canSelectSource: boolean;
};

/** Inert editing controls used outside explicit Image mode. */
export const DEFAULT_IMAGE_EDITING_PRESENTATION: ImageEditingPresentation = {
  active: false,
  execution: "cloud",
  selectedSourceIds: [],
  canSelectSource: false,
};

/** Result of mirroring native image-prompt normalization before durable user-message insertion. */
export type PreparedImageGenerationPrompt = { ok: true; prompt: string } | { ok: false; message: string };

/** Ordered source identities for one hosted edit, or a path-free eligibility explanation. */
export type PreparedImageEditingSources =
  { ok: true; sources: ImageEditingSourceRequest[] } | { ok: false; message: string };

const MAX_IMAGE_PROMPT_CHARACTERS = 1_000;
const MAX_IMAGE_EDITING_SOURCES = 3;
const CONTROL_CHARACTER = /\p{Cc}/u;

/** Returns the compact disclosure label for one durable image-edit ancestry. */
export function imageEditingLineageLabel(sourceCount: number): string {
  return `Edited from ${sourceCount} source${sourceCount === 1 ? "" : "s"}`;
}

/** Formats one path-free edit source without revealing its opaque native identity. */
export function imageEditingSourcePresentation(source: StoredGeneratedImageSource): { kind: string; details: string } {
  return {
    kind: source.sourceType === "attachment" ? "Attachment" : "Generated image",
    details: `${source.width}×${source.height} · ${source.mediaType} · ${formatBytes(source.byteSize)}`,
  };
}

/** Normalizes and bounds a prompt while Rust remains the authoritative request validator. */
export function prepareImageGenerationPrompt(prompt: string): PreparedImageGenerationPrompt {
  const normalized = prompt.replaceAll("\r\n", "\n").trim();
  if (normalized.length === 0) return { ok: false, message: "Enter a non-empty image description." };
  const characters = Array.from(normalized);
  const hasUnsupportedControl = characters.some(
    (character) => CONTROL_CHARACTER.test(character) && character !== "\n" && character !== "\t",
  );
  if (characters.length > MAX_IMAGE_PROMPT_CHARACTERS || hasUnsupportedControl) {
    return {
      ok: false,
      message: "The image description is too long or contains unsupported control characters.",
    };
  }
  return { ok: true, prompt: normalized };
}

/** Validates current-draft image sources and retains their visible opaque-ID order. */
export function prepareImageEditingSources(
  execution: ImageGenerationExecution,
  attachments: Attachment[],
  generatedAssets: GeneratedAsset[] = [],
): PreparedImageEditingSources {
  const sourceCount = attachments.length + generatedAssets.length;
  if (sourceCount === 0) return { ok: true, sources: [] };
  if (execution !== "cloud") {
    return { ok: false, message: "Reference-image editing is available only with Cloud execution." };
  }
  if (sourceCount > MAX_IMAGE_EDITING_SOURCES) {
    return { ok: false, message: "Choose no more than three reference images for one edit." };
  }
  if (attachments.some((attachment) => attachment.kind !== "image")) {
    return { ok: false, message: "Remove non-image attachments before editing an image." };
  }
  if (attachments.some((attachment) => attachment.normalization.state !== "ready")) {
    return { ok: false, message: "Wait for every reference image to finish native normalization." };
  }
  if (generatedAssets.some((asset) => asset.status !== "completed")) {
    return { ok: false, message: "Choose only completed generated images as references." };
  }
  const sourceIds = [...attachments.map(({ id }) => id), ...generatedAssets.map(({ id }) => id)];
  if (new Set(sourceIds).size !== sourceIds.length) {
    return { ok: false, message: "Remove duplicate reference images before editing." };
  }
  return {
    ok: true,
    sources: [
      ...attachments.map(({ id }) => ({ sourceType: "attachment" as const, sourceId: id })),
      ...generatedAssets.map(({ id }) => ({ sourceType: "generated_asset" as const, sourceId: id })),
    ],
  };
}

/** Resolves one route and visible Cloud choice to the exact native request options. */
export function imageGenerationRequestOptions(
  execution: ImageGenerationExecution,
  size: ImageGenerationSize,
  count: number,
): { width: number; height: number; count: number } {
  if (execution === "local") return { width: 512, height: 512, count: 1 };
  switch (size) {
    case "landscape":
      return { width: 2_688, height: 1_536, count };
    case "portrait":
      return { width: 1_536, height: 2_688, count };
    default:
      return { width: 2_048, height: 2_048, count };
  }
}
