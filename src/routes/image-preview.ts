/** Development-only fixture for durable generated-image lineage presentation. */

import type { Message } from "$lib/presentation";

import type { PageState } from "./page-state.svelte";

const IMAGE_EDIT_LINEAGE_PREVIEW_VALUE = "edit-lineage";

/** Reports whether a query explicitly requests the generated edit-lineage fixture. */
export function imagePreviewRequested(search: string): boolean {
  return new URLSearchParams(search).get("image") === IMAGE_EDIT_LINEAGE_PREVIEW_VALUE;
}

/** Applies deterministic path-free generated image records to the disconnected browser preview. */
export function applyImagePreview(state: PageState, search: string): boolean {
  if (!imagePreviewRequested(search)) return false;
  const commonAsset = {
    ordinal: 0,
    status: "completed" as const,
    mediaType: "image/png" as const,
    width: 2_048,
    height: 2_048,
    byteSize: 1_572_864,
    providerId: "qwen-image",
    modelId: "qwen-image-2.0-2026-03-03",
    execution: "cloud" as const,
    seed: null,
    errorCode: null,
    createdAtMs: 1_776_000_000_000,
    previewUrl: "/favicon.png",
  };
  const messages: Message[] = [
    {
      id: 1,
      storageId: "preview-edit-response",
      role: "assistant",
      content: "The requested edit is ready.",
      generatedAssets: [
        {
          ...commonAsset,
          id: "preview-edit",
          sources: [
            {
              ordinal: 0,
              sourceType: "attachment",
              sourceId: "preview-attachment-source",
              mediaType: "image/jpeg",
              width: 1_536,
              height: 2_688,
              byteSize: 4_096,
            },
            {
              ordinal: 1,
              sourceType: "generated_asset",
              sourceId: "preview-generated-source",
              mediaType: "image/png",
              width: 2_048,
              height: 2_048,
              byteSize: 1_572_864,
            },
          ],
        },
      ],
    },
    {
      id: 2,
      storageId: "preview-generation-response",
      role: "assistant",
      content: "The ordinary generated image has no edit ancestry.",
      generatedAssets: [{ ...commonAsset, id: "preview-generation", sources: [] }],
    },
  ];
  state.messages = messages;
  return true;
}
