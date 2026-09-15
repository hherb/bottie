import { expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (id: string, protocol: string) => `${protocol}://${id}`,
}));

import type { StoredMessage } from "$lib/storage";

import { storedMessageToPresentation } from "./conversation-presentation";

it("retains path-free generated edit lineage when rebuilding a reopened message", () => {
  const stored: StoredMessage = {
    id: "assistant-edit",
    role: "assistant",
    text: "Edited image.",
    reasoning: null,
    state: "final",
    providerId: "qwen-image",
    modelId: "qwen-image-2.0-2026-03-03",
    providerRun: null,
    rating: null,
    attachments: [],
    generatedAssets: [
      {
        id: "generated-edit",
        ordinal: 0,
        status: "completed",
        mediaType: "image/png",
        width: 2_048,
        height: 2_048,
        byteSize: 8_192,
        providerId: "qwen-image",
        modelId: "qwen-image-2.0-2026-03-03",
        execution: "cloud",
        seed: null,
        errorCode: null,
        createdAtMs: 1,
        sources: [
          {
            ordinal: 0,
            sourceType: "attachment",
            sourceId: "source-attachment",
            mediaType: "image/jpeg",
            width: 1_536,
            height: 2_688,
            byteSize: 4_096,
          },
          {
            ordinal: 1,
            sourceType: "generated_asset",
            sourceId: "source-generated",
            mediaType: "image/png",
            width: 2_048,
            height: 2_048,
            byteSize: 1_572_864,
          },
        ],
      },
    ],
    createdAtMs: 1,
  };

  expect(storedMessageToPresentation(stored).generatedAssets?.[0].sources).toEqual(stored.generatedAssets[0].sources);
});
