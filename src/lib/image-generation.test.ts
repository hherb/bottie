import { describe, expect, it } from "vitest";

import {
  imageGenerationRequestOptions,
  prepareImageEditingSources,
  prepareImageGenerationPrompt,
} from "./image-generation";
import type { Attachment } from "./presentation";

/** Builds one path-free attachment fixture for image-editing eligibility tests. */
function attachment(id: string, state: Attachment["normalization"]["state"] = "ready"): Attachment {
  return {
    id,
    name: `${id}.png`,
    size: "4 KB",
    kind: "image",
    mimeType: "image/png",
    previewUrl: null,
    extraction: {
      state: "unsupported",
      format: null,
      characterCount: null,
      pageCount: null,
      errorCode: null,
    },
    indexing: { state: "unsupported" },
    normalization: {
      state,
      format: state === "ready" ? "png" : null,
      width: state === "ready" ? 64 : null,
      height: state === "ready" ? 64 : null,
      byteSize: state === "ready" ? 4_096 : null,
      errorCode: null,
    },
  };
}

describe("image generation presentation", () => {
  it("maps every Cloud aspect choice to exact bounded provider options", () => {
    expect(imageGenerationRequestOptions("cloud", "square", 2)).toEqual({
      width: 2_048,
      height: 2_048,
      count: 2,
    });
    expect(imageGenerationRequestOptions("cloud", "landscape", 3)).toEqual({
      width: 2_688,
      height: 1_536,
      count: 3,
    });
    expect(imageGenerationRequestOptions("cloud", "portrait", 4)).toEqual({
      width: 1_536,
      height: 2_688,
      count: 4,
    });
  });

  it("forces the selected local route to its one proved output shape", () => {
    expect(imageGenerationRequestOptions("local", "landscape", 6)).toEqual({
      width: 512,
      height: 512,
      count: 1,
    });
  });

  it("normalizes the same bounded prompt shape before native persistence", () => {
    expect(prepareImageGenerationPrompt("  Draw Bottie\r\nwith a lantern\t ")).toEqual({
      ok: true,
      prompt: "Draw Bottie\nwith a lantern",
    });
  });

  it("rejects empty oversized and unsupported control-character prompts", () => {
    expect(prepareImageGenerationPrompt(" \n\t ")).toEqual({
      ok: false,
      message: "Enter a non-empty image description.",
    });
    expect(prepareImageGenerationPrompt("🎨".repeat(1_001))).toEqual({
      ok: false,
      message: "The image description is too long or contains unsupported control characters.",
    });
    expect(prepareImageGenerationPrompt("Draw\u0000Bottie")).toEqual({
      ok: false,
      message: "The image description is too long or contains unsupported control characters.",
    });
  });

  it("preserves one to three ready Cloud attachment IDs in visible order", () => {
    expect(prepareImageEditingSources("cloud", [])).toEqual({ ok: true, sources: [] });
    expect(prepareImageEditingSources("cloud", [attachment("second"), attachment("first")])).toEqual({
      ok: true,
      sources: [
        { sourceType: "attachment", sourceId: "second" },
        { sourceType: "attachment", sourceId: "first" },
      ],
    });
  });

  it("fails closed for local, unready, non-image, duplicate, or excessive editing sources", () => {
    expect(prepareImageEditingSources("local", [attachment("source")])).toMatchObject({ ok: false });
    expect(prepareImageEditingSources("cloud", [attachment("pending", "pending")])).toMatchObject({ ok: false });
    expect(prepareImageEditingSources("cloud", [{ ...attachment("document"), kind: "file" }])).toMatchObject({
      ok: false,
    });
    expect(prepareImageEditingSources("cloud", [attachment("same"), attachment("same")])).toMatchObject({
      ok: false,
    });
    expect(
      prepareImageEditingSources("cloud", [
        attachment("one"),
        attachment("two"),
        attachment("three"),
        attachment("four"),
      ]),
    ).toMatchObject({ ok: false });
  });

  it("appends completed generated-image identities after visible draft attachments", () => {
    const generated = {
      id: "generated-source",
      ordinal: 0,
      status: "completed" as const,
      mediaType: "image/png" as const,
      width: 64,
      height: 64,
      byteSize: 4_096,
      providerId: "qwen-image",
      modelId: "qwen-image-2.0-2026-03-03",
      execution: "cloud" as const,
      seed: null,
      errorCode: null,
      createdAtMs: 1,
      sources: [],
      previewUrl: "bottie-generated-asset://generated-source",
    };

    expect(prepareImageEditingSources("cloud", [attachment("attachment-source")], [generated])).toEqual({
      ok: true,
      sources: [
        { sourceType: "attachment", sourceId: "attachment-source" },
        { sourceType: "generated_asset", sourceId: "generated-source" },
      ],
    });
    expect(prepareImageEditingSources("cloud", [], [{ ...generated, status: "pending" }])).toMatchObject({
      ok: false,
    });
    expect(
      prepareImageEditingSources("cloud", [attachment("one"), attachment("two"), attachment("three")], [generated]),
    ).toMatchObject({ ok: false });
  });
});
