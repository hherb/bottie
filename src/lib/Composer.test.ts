import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import type { Attachment } from "./presentation";
import Composer from "./Composer.svelte";

/** Renders the composer with inert callbacks and the requested interaction eligibility. */
function renderedComposer(
  canCompose: boolean,
  canSend: boolean,
  attachments: Attachment[] = [],
  memoryAvailable = false,
  memoryEnabled = false,
): string {
  return render(Composer, {
    props: {
      attachments,
      prompt: "Describe this image",
      isGenerating: false,
      canCompose,
      canSend,
      attachmentNote: "Wait for image normalization to finish before sending.",
      providerStatus: "available",
      memoryAvailable,
      memoryEnabled,
      onprompt: vi.fn(),
      oninput: vi.fn(),
      onkeydown: vi.fn(),
      onsend: vi.fn(),
      onadd: vi.fn(),
      onfiles: vi.fn(),
      onremove: vi.fn(),
      ontogglememory: vi.fn(),
      oncomposerready: vi.fn(),
      onattachmentinputready: vi.fn(),
    },
  }).body;
}

describe("Composer", () => {
  it("keeps text input enabled when an attachment blocks only submission", () => {
    const html = renderedComposer(true, false);

    expect(html).toMatch(/<textarea(?![^>]* disabled)/);
    expect(html).toMatch(/<button[^>]*class="send-button"[^>]* disabled/);
  });

  it("disables text input when the provider and model cannot accept a prompt", () => {
    expect(renderedComposer(false, false)).toMatch(/<textarea[^>]* disabled/);
  });

  it("keeps one ready thumbnail and failure explanation attached to its draft chip", () => {
    const failedImage: Attachment = {
      id: "image",
      name: "broken.png",
      size: "4 KB",
      kind: "image",
      mimeType: "image/png",
      previewUrl: null,
      extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
      indexing: { state: "unsupported" },
      normalization: {
        state: "failed",
        format: null,
        width: null,
        height: null,
        byteSize: null,
        errorCode: "image_decode_failed",
      },
    };

    const html = renderedComposer(true, false, [failedImage]);
    expect(html).toContain('class="attachment-chip failed"');
    expect(html).toContain("Image could not be decoded");
    expect(html).toContain("cannot preview or send this image");
  });

  it("exposes explicit pressed state only for a mapped tool-capable selection", () => {
    const enabled = renderedComposer(true, true, [], true, true);
    const unavailable = renderedComposer(true, true);

    expect(enabled).toMatch(/aria-label="Disable memory tools"[^>]*aria-pressed="true"/);
    expect(unavailable).toMatch(/aria-label="Memory tools require a supported tool-capable model"[^>]* disabled/);
  });
});
