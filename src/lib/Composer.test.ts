import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import type { Attachment } from "./presentation";
import { INITIAL_MICROPHONE_DEVICE_LIST, INITIAL_MICROPHONE_STATUS } from "./microphone";
import Composer from "./Composer.svelte";

/** Renders the composer with inert callbacks and the requested interaction eligibility. */
function renderedComposer(
  canCompose: boolean,
  canSend: boolean,
  attachments: Attachment[] = [],
  memoryAvailable = false,
  memoryEnabled = false,
  webAvailable = false,
  webEnabled = false,
  emailAvailable = false,
  emailEnabled = false,
  emailBoundaryNote = [
    "Your prompt stays with Ollama on loopback; model-selected email queries, exact message IDs, and attachment",
    "selections go only to your pinned Localmail server.",
  ].join(" "),
  emailUnavailableReason = "Save Localmail certificate trust and a bearer token in Settings before enabling Email.",
  isGenerating = false,
  microphoneWillInterrupt = false,
): string {
  return render(Composer, {
    props: {
      attachments,
      prompt: "Describe this image",
      isGenerating,
      canCompose,
      canSend,
      attachmentNote: "Wait for image normalization to finish before sending.",
      providerStatus: "available",
      memoryAvailable,
      memoryEnabled,
      webAvailable,
      webEnabled,
      emailAvailable,
      emailEnabled,
      emailBoundaryNote,
      emailUnavailableReason,
      microphoneStatus: INITIAL_MICROPHONE_STATUS,
      microphoneAvailable: true,
      microphoneWillInterrupt,
      microphoneAudioAvailable: false,
      microphoneAudioUnavailableReason: "Choose an audio-capable model to send this recording.",
      microphoneSendAudio: false,
      microphoneRetainAudio: false,
      microphoneDeviceList: INITIAL_MICROPHONE_DEVICE_LIST,
      microphoneDevicesLoaded: false,
      microphoneDeviceListFailed: false,
      onprompt: vi.fn(),
      oninput: vi.fn(),
      onkeydown: vi.fn(),
      onsend: vi.fn(),
      onadd: vi.fn(),
      onfiles: vi.fn(),
      onremove: vi.fn(),
      ontogglememory: vi.fn(),
      ontoggleweb: vi.fn(),
      ontoggleemail: vi.fn(),
      onstartmicrophone: vi.fn(),
      onstopmicrophone: vi.fn(),
      ondiscardmicrophone: vi.fn(),
      oncorrectmicrophone: vi.fn(),
      ontogglesendmicrophoneaudio: vi.fn(),
      ontoggleretainmicrophoneaudio: vi.fn(),
      onloadmicrophonedevices: vi.fn(),
      onselectmicrophonedevice: vi.fn(),
      onusemicrophonetranscript: vi.fn(),
      microphoneTranscriptDraftFeedback: "",
      microphoneTranscriptDraftError: false,
      oncomposerready: vi.fn(),
      onattachmentinputready: vi.fn(),
    },
  }).body;
}

describe("Composer", () => {
  it("keeps text input enabled when an attachment blocks only submission", () => {
    const html = renderedComposer(true, false);

    expect(html).toMatch(/<textarea(?![^>]* disabled)/);
    expect(html).toContain('aria-describedby="composer-guidance composer-email-guidance"');
    expect(html).toContain('id="composer-guidance"');
    expect(html).toContain('aria-live="polite"');
    expect(html).toMatch(/<button[^>]*class="send-button"[^>]* disabled/);
  });

  it("disables text input when the provider and model cannot accept a prompt", () => {
    expect(renderedComposer(false, false)).toMatch(/<textarea[^>]* disabled/);
  });

  it("keeps explicit voice barge-in available during provider generation", () => {
    const html = renderedComposer(
      true,
      true,
      [],
      false,
      false,
      false,
      false,
      false,
      false,
      "Email unavailable.",
      "Email unavailable.",
      true,
      true,
    );

    expect(html).toContain('aria-label="Interrupt Bottie and record voice locally"');
    expect(html).not.toMatch(/aria-label="Interrupt Bottie and record voice locally"[^>]*disabled/);
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
    const enabled = renderedComposer(true, true, [], true, true, true, true, true, true);
    const unavailable = renderedComposer(true, true);

    expect(enabled).toMatch(/aria-label="Disable memory tools"[^>]*aria-pressed="true"/);
    expect(enabled).toMatch(/aria-label="Disable web search"[^>]*aria-pressed="true"/);
    expect(enabled).toMatch(/aria-label="Disable email tools"[^>]*aria-pressed="true"/);
    expect(enabled).toContain("Your prompt stays with Ollama on loopback");
    expect(enabled).toMatch(/attachment\s+selections go only to your pinned Localmail server/);
    expect(unavailable).toMatch(/aria-label="Memory tools require a supported tool-capable model"[^>]* disabled/);
    expect(unavailable).toMatch(/aria-label="Web search requires a supported tool-capable model"[^>]* disabled/);
    expect(unavailable).toMatch(
      /aria-label="Save Localmail certificate trust and a bearer token in Settings before enabling Email\."[^>]* disabled/,
    );
    expect(unavailable).toContain(
      "Save Localmail certificate trust and a bearer token in Settings before enabling Email.",
    );
  });

  it("discloses cloud prompt and bounded result delivery for OpenAI-compatible Email", () => {
    const boundary = [
      "Your prompt and bounded Localmail tool results go to the selected OpenAI-compatible cloud endpoint;",
      "model-selected email queries, exact message IDs, and attachment selections go only to your pinned Localmail server.",
    ].join(" ");
    const html = renderedComposer(true, true, [], false, false, false, false, true, true, boundary);

    expect(html).toContain("selected OpenAI-compatible cloud endpoint");
    expect(html).toContain("bounded Localmail tool results");
    expect(html).toMatch(/attachment selections go only to your\s+pinned Localmail server/);
  });
});
