import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import type { Attachment } from "./presentation";
import { INITIAL_MICROPHONE_DEVICE_LIST, INITIAL_MICROPHONE_STATUS } from "./microphone";
import type { LocalImageAcquisitionStatus, LocalImageAvailabilityMetadata } from "./local-image";
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
  imageMode = false,
  localImageAvailability: LocalImageAvailabilityMetadata | null = null,
  localImageAvailabilityFailed = false,
  imageExecution: "cloud" | "local" = "cloud",
  localImageAcquisitionStatus: LocalImageAcquisitionStatus | null = null,
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
      imageMode,
      imageExecution,
      imageSize: "square",
      imageCount: 1,
      imageFeedback: "",
      localImageAvailability,
      localImageAvailabilityFailed,
      localImageAcquisitionStatus,
      localImageAcquisitionFeedback: "",
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
      ontoggleimage: vi.fn(),
      onimageexecution: vi.fn(),
      onimagesize: vi.fn(),
      onimagecount: vi.fn(),
      oninstalllocalimage: vi.fn(),
      oncancellocalimage: vi.fn(),
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
  it("shows exact cloud delivery and cost disclosure only for the explicit image action", () => {
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
      false,
      false,
      true,
    );

    expect(html).toContain('aria-label="Image generation options"');
    expect(html).toContain("qwen-image-2.0-2026-03-03");
    expect(html).toContain("may incur provider charges");
    expect(html).toContain('aria-label="Generate image"');
    expect(html).toMatch(/aria-label="Attach files"[^>]*disabled/);
  });

  it("shows the local route but disables selection until native readiness is exact", () => {
    const local: LocalImageAvailabilityMetadata = {
      modelId: "Qwen/Qwen-Image-2512",
      packageId: "AbstractFramework/qwen-image-2512-4bit",
      runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
      license: "Apache-2.0",
      sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
      expectedDiskBytes: 17_442_350_812,
      requiredMemoryBytes: 29_526_129_448,
      availability: "worker_missing",
    };
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
      false,
      false,
      true,
      local,
    );

    expect(html).toContain('aria-label="Local image availability"');
    expect(html).toContain("Qwen/Qwen-Image-2512");
    expect(html).toContain("AbstractFramework/qwen-image-2512-4bit");
    expect(html).toContain("MLX-Gen");
    expect(html).toContain("Local worker is not installed");
    expect(html).toContain("16.2 GiB model");
    expect(html).toContain('aria-label="Execution"');
    expect(html).toMatch(/<option value="local" disabled/);
    expect(html).toContain("Local 2512 · unavailable");
    expect(html).toContain("No automatic download or cloud fallback");
  });

  it("exposes an explicit ready local route with fixed output options and private disclosure", () => {
    const local: LocalImageAvailabilityMetadata = {
      modelId: "Qwen/Qwen-Image-2512",
      packageId: "AbstractFramework/qwen-image-2512-4bit",
      runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
      license: "Apache-2.0",
      sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
      expectedDiskBytes: 17_442_350_812,
      requiredMemoryBytes: 29_526_129_448,
      availability: "ready",
    };
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
      false,
      false,
      true,
      local,
      false,
      "local",
    );

    expect(html).toMatch(/<option value="local" selected(?:="")?>Local 2512<\/option>/);
    expect(html).toContain("512×512 · one image");
    expect(html).toContain("stay on this device");
    expect(html).toContain("Qwen/Qwen-Image-2512");
    expect(html).not.toContain("may incur provider charges");
  });

  it("offers exact user-approved installation only after the native worker gate passes", () => {
    const local: LocalImageAvailabilityMetadata = {
      modelId: "Qwen/Qwen-Image-2512",
      packageId: "AbstractFramework/qwen-image-2512-4bit",
      runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
      license: "Apache-2.0",
      sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
      expectedDiskBytes: 17_442_350_812,
      requiredMemoryBytes: 29_526_129_448,
      availability: "model_missing",
    };
    const acquisition: LocalImageAcquisitionStatus = {
      ...local,
      phase: "awaiting_approval",
      failure: null,
      downloadedFiles: 0,
      totalFiles: 18,
      downloadedBytes: 0,
      verifiedFiles: 0,
    };
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
      false,
      false,
      true,
      local,
      false,
      "cloud",
      acquisition,
    );

    expect(html).toContain('aria-label="Local image model installation"');
    expect(html).toContain("Download and install 16.2 GiB");
    expect(html).toContain("Hugging Face");
    expect(html).toContain("app-owned cache");
    expect(html).toContain("generation remains offline");
    expect(html).toContain("18 exact files will be verified before activation");
  });

  it("labels active model progress and cancellation as separate controls", () => {
    const acquisition: LocalImageAcquisitionStatus = {
      modelId: "Qwen/Qwen-Image-2512",
      packageId: "AbstractFramework/qwen-image-2512-4bit",
      runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
      license: "Apache-2.0",
      sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
      expectedDiskBytes: 17_442_350_812,
      requiredMemoryBytes: 29_526_129_448,
      availability: "model_missing",
      phase: "downloading",
      failure: null,
      downloadedFiles: 4,
      totalFiles: 18,
      downloadedBytes: 4_360_587_703,
      verifiedFiles: 0,
    };
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
      false,
      false,
      true,
      { ...acquisition },
      false,
      "cloud",
      acquisition,
    );

    expect(html).toContain('aria-label="Local model download progress"');
    expect(html).toContain("Downloading model… 25%");
    expect(html).toContain("Cancel download");
  });
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
