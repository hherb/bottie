import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import type { Attachment } from "./presentation";
import AttachmentVisual from "./AttachmentVisual.svelte";

/** Builds one ready image with an ID-only preview URL. */
function imageAttachment(previewUrl: string | null): Attachment {
  return {
    id: "attachment-1",
    name: "diagram.png",
    size: "4 KB",
    kind: "image",
    mimeType: "image/png",
    previewUrl,
    extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
    indexing: { state: "unsupported" },
    normalization: { state: "ready", format: "png", width: 320, height: 180, byteSize: 4_096, errorCode: null },
  };
}

describe("AttachmentVisual", () => {
  it("renders a labelled thumbnail only when Rust made a ready preview URL available", () => {
    const html = render(AttachmentVisual, {
      props: {
        attachment: imageAttachment("bottie-attachment://localhost/attachment-1"),
        className: "visual",
        iconSize: 18,
      },
    }).body;

    expect(html).toContain('src="bottie-attachment://localhost/attachment-1"');
    expect(html).toContain('alt="Preview of diagram.png"');
    expect(html).toContain('loading="lazy"');
  });

  it("keeps the file-type icon when no ready preview exists", () => {
    const html = render(AttachmentVisual, {
      props: { attachment: imageAttachment(null), className: "visual", iconSize: 18 },
    }).body;

    expect(html).not.toContain("<img");
    expect(html).toContain("<svg");
  });
});
