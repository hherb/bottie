import { describe, expect, it } from "vitest";

import type { ProviderSettings } from "$lib/inference";
import type { Attachment, Message } from "$lib/presentation";

import {
  emailToolsBoundaryNote,
  emailToolsUnavailableReason,
  emailToolsAvailable,
  inferenceStages,
  memoryToolsAvailable,
  messageAttachmentAssociations,
  nextRequestAttachments,
  selectedProviderEndpoint,
  webToolsAvailable,
} from "./page-presentation";

const attachment: Attachment = {
  id: "attachment-1",
  name: "context.png",
  size: "1 KB",
  kind: "image",
  mimeType: "image/png",
  previewUrl: null,
  extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
  indexing: { state: "unsupported" },
  normalization: { state: "ready", format: "png", width: 1, height: 1, byteSize: 68, errorCode: null },
};

describe("page presentation", () => {
  it("offers native memory tools only on mapped providers that advertise tools", () => {
    const model = (providerId: "ollama" | "openai" | "anthropic" | "omlx", tools: boolean) => ({
      providerId,
      providerName: providerId,
      modelId: "model",
      displayName: "Model",
      maxContextTokens: null,
      loadState: "unknown" as const,
      capabilities: { text: true, streaming: true, tools, vision: false, embeddings: false },
    });

    expect(memoryToolsAvailable(model("ollama", true))).toBe(true);
    expect(memoryToolsAvailable(model("openai", true))).toBe(true);
    expect(memoryToolsAvailable(model("anthropic", true))).toBe(true);
    expect(memoryToolsAvailable(model("omlx", true))).toBe(true);
    expect(memoryToolsAvailable(model("openai", false))).toBe(false);
    expect(memoryToolsAvailable(model("omlx", false))).toBe(false);
    expect(memoryToolsAvailable(undefined)).toBe(false);
    expect(webToolsAvailable(model("ollama", true))).toBe(true);
    expect(webToolsAvailable(model("openai", true))).toBe(true);
    expect(webToolsAvailable(model("anthropic", true))).toBe(true);
    expect(webToolsAvailable(model("omlx", true))).toBe(true);
    expect(webToolsAvailable(model("openai", false))).toBe(false);
    expect(webToolsAvailable(model("ollama", false))).toBe(false);
    expect(webToolsAvailable(undefined)).toBe(false);
    expect(emailToolsAvailable(model("ollama", true))).toBe(true);
    expect(emailToolsAvailable(model("openai", true))).toBe(true);
    expect(emailToolsAvailable(model("anthropic", true))).toBe(false);
    expect(emailToolsAvailable(model("omlx", true))).toBe(false);
    expect(emailToolsAvailable(model("ollama", false))).toBe(false);
    expect(emailToolsAvailable(undefined)).toBe(false);

    expect(emailToolsUnavailableReason(model("omlx", true), true)).toBe(
      [
        "Email is currently mapped only for Ollama and OpenAI-compatible models.",
        "Switch to a supported tool-capable model.",
      ].join(" "),
    );
    expect(emailToolsUnavailableReason(model("ollama", false), true)).toBe(
      "The selected Ollama model does not advertise tool support. Choose a tool-capable Ollama model.",
    );
    expect(emailToolsUnavailableReason(model("ollama", true), false)).toBe(
      "Save Localmail certificate trust and a bearer token in Settings before enabling Email.",
    );
    expect(emailToolsUnavailableReason(model("openai", false), true)).toBe(
      [
        "The selected OpenAI-compatible model does not advertise tool support.",
        "Choose a tool-capable OpenAI-compatible model.",
      ].join(" "),
    );
    expect(emailToolsUnavailableReason(model("omlx", true), false)).toBe(
      [
        "Save Localmail certificate trust and a bearer token in Settings, then switch to a tool-capable Ollama",
        "or OpenAI-compatible model.",
      ].join(" "),
    );
    expect(emailToolsUnavailableReason(model("ollama", true), true)).toBe("");
    expect(emailToolsUnavailableReason(model("openai", true), true)).toBe("");
    expect(emailToolsBoundaryNote(model("ollama", true))).toContain("stays with Ollama on loopback");
    expect(emailToolsBoundaryNote(model("openai", true))).toContain(
      "prompt and bounded Localmail tool results go to the selected OpenAI-compatible cloud endpoint",
    );
    expect(emailToolsBoundaryNote(model("openai", true))).toContain(
      "email queries and exact message IDs go only to your pinned Localmail server",
    );
  });

  it("keeps draft, conversation, and message scope identities explicit", () => {
    const messages: Message[] = [
      { id: 1, storageId: "message-1", role: "user", content: "Use context", attachments: [attachment] },
      { id: 2, role: "assistant", content: "Done" },
    ];

    expect(messageAttachmentAssociations(messages)).toEqual([{ messageId: "message-1", attachment }]);
    expect(nextRequestAttachments([attachment], [{ ...attachment, id: "conversation-attachment" }])).toEqual([
      attachment,
      { ...attachment, id: "conversation-attachment" },
    ]);
  });

  it("builds provider route labels without exposing protocol noise", () => {
    const settings: ProviderSettings = {
      omlxBaseUrl: "http://127.0.0.1:8000/",
      ollamaBaseUrl: "http://127.0.0.1:11434/",
      openaiBaseUrl: "https://api.openai.com/v1/",
      anthropicBaseUrl: "https://api.anthropic.com/v1/",
      webSearchProviderId: "brave",
      webNetworkPolicy: { httpsOnly: true, allowedDomains: [], blockedDomains: [] },
      setupCompleted: true,
      lastProviderId: null,
      lastModelId: null,
    };

    expect(selectedProviderEndpoint("ollama", settings)).toBe("127.0.0.1:11434");
    expect(inferenceStages(false, false, "Cloud provider", "Brave Search", "low")).toEqual([
      { icon: "shield", label: "Cloud route confirmed", detail: "Rust → Cloud provider" },
      { icon: "sparkles", label: "Streaming response", detail: "Low reasoning" },
    ]);
    expect(inferenceStages(true, true, "Ollama", "Exa Search", "off")[0]).toEqual({
      icon: "shield",
      label: "Local model with web access",
      detail: "Rust → Ollama + Exa Search",
    });
  });
});
