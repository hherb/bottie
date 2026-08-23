import type { ProviderId, ProviderSettings, WebSearchProviderId } from "./inference";
import type {
  AttachmentExtraction,
  AttachmentIndexing,
  ImageNormalization,
  ResponseRating,
  StoredToolInvocation,
} from "./storage";

let messageSequence = Date.now();

/** Returns the disclosure name for one fixed native search route. */
export function webSearchProviderName(providerId: WebSearchProviderId): string {
  return providerId === "exa" ? "Exa Search" : "Brave Search";
}

/** Returns a process-unique numeric key for ephemeral message presentation. */
export function nextMessageId(): number {
  return ++messageSequence;
}

/** One message rendered in the prototype conversation. */
export type Message = {
  id: number;
  storageId?: string;
  role: "user" | "assistant";
  content: string;
  reasoning?: string;
  featured?: boolean;
  model?: string;
  meta?: string;
  error?: boolean;
  retryable?: boolean;
  rating?: ResponseRating;
  toolInvocations?: StoredToolInvocation[];
  attachments?: Attachment[];
};

/** Browser-side attachment metadata used by the presentation-only attachment preview. */
export type Attachment = {
  id: string;
  name: string;
  size: string;
  kind: "image" | "file";
  mimeType: string;
  /** ID-only native protocol URL for a bounded ready image thumbnail. */
  previewUrl: string | null;
  extraction: AttachmentExtraction;
  indexing: AttachmentIndexing;
  normalization: ImageNormalization;
};

/** One visible selected-lineage attachment linked to its durable user message. */
export type MessageAttachment = {
  messageId: string;
  attachment: Attachment;
};

/** Native application identity displayed in the sidebar. */
export type RuntimeInfo = {
  name: string;
  version: string;
  storage: string;
};

/** Provider connection state displayed throughout the shell. */
export type ProviderStatus = "checking" | "available" | "offline" | "browser";

/** One normalized stage in the live inference activity presentation. */
export type InferenceStage = {
  icon: "shield" | "sparkles";
  label: string;
  detail: string;
};

/** Provider choices displayed in the provider selector. */
export const PROVIDER_OPTIONS: Array<{ id: ProviderId; name: string; route: "local" | "cloud" }> = [
  { id: "ollama", name: "Ollama", route: "local" },
  { id: "omlx", name: "oMLX", route: "local" },
  { id: "openai", name: "OpenAI compatible", route: "cloud" },
  { id: "anthropic", name: "Anthropic compatible", route: "cloud" },
];

/** Maximum number of attachment chips shown above the composer. */
export const MAX_COMPOSER_ATTACHMENTS = 3;

/** Maximum automatic composer height before internal scrolling begins. */
export const MAX_COMPOSER_HEIGHT_PX = 160;

/** Browser-preview defaults mirrored from the native provider configuration. */
export const DEFAULT_PROVIDER_SETTINGS: ProviderSettings = {
  omlxBaseUrl: "http://127.0.0.1:8000/",
  ollamaBaseUrl: "http://127.0.0.1:11434/",
  openaiBaseUrl: "https://api.openai.com/v1/",
  anthropicBaseUrl: "https://api.anthropic.com/v1/",
  webSearchProviderId: "brave",
  lastProviderId: null,
  lastModelId: null,
};

/** Navigation fixtures retained until durable conversations are implemented. */
export const CONVERSATION_GROUPS = [
  {
    label: "Today",
    items: [
      { title: "Bottie architecture", active: true },
      { title: "Local model benchmarks", active: false },
    ],
  },
  {
    label: "Yesterday",
    items: [
      { title: "Weekend reading list", active: false },
      { title: "SQLite search notes", active: false },
    ],
  },
  {
    label: "Previous 7 days",
    items: [
      { title: "Rust async patterns", active: false },
      { title: "Kyoto in autumn", active: false },
      { title: "Camera comparison", active: false },
    ],
  },
];

/** Opening message fixtures retained until durable conversations are implemented. */
export const INITIAL_MESSAGES: Message[] = [
  {
    id: 1,
    role: "user",
    content: "Can you turn our bottie notes into a focused implementation plan?",
  },
  {
    id: 2,
    role: "assistant",
    featured: true,
    model: "Product shell fixture",
    content:
      "## A focused sequence\n\n1. Build the conversation experience.\n2. Connect inference and persistence.\n" +
      "3. Add tools behind the native boundary.\n\nThe important boundary is simple: the `WebView` presents " +
      "state; the **Rust core** owns secrets, files, storage, provider calls, and tool execution.",
    toolInvocations: [
      {
        ordinal: 0,
        toolName: "search_memory",
        arguments: { query: "bottie architecture boundary" },
        result: {
          output: {
            ok: true,
            result: {
              matches: [
                {
                  rank: 1,
                  excerpt: "Keep secrets, storage, provider calls, and tool execution inside the Rust core.",
                  provenance: {
                    sourceKind: "message",
                    conversationId: "preview-conversation",
                    conversationTitle: "Bottie architecture",
                    messageId: "preview-message",
                    role: "assistant",
                    createdAtMs: 1_776_000_000_000,
                  },
                },
              ],
            },
          },
          isError: false,
          createdAtMs: 2,
        },
        audit: {
          policy: "safe",
          outcome: "success",
          durationMs: 18,
        },
        createdAtMs: 1,
      },
      {
        ordinal: 1,
        toolName: "web_search",
        arguments: { query: "Bottie local-first architecture" },
        result: {
          output: {
            ok: true,
            result: {
              providerId: "brave",
              results: [
                {
                  title: "Tauri security guidance",
                  url: "https://v2.tauri.app/security/",
                  snippet: "Keep privileged capabilities behind narrow native commands.",
                  publishedAt: null,
                },
              ],
            },
          },
          isError: false,
          createdAtMs: 4,
        },
        audit: {
          policy: "safe",
          outcome: "success",
          durationMs: 24,
        },
        createdAtMs: 3,
      },
    ],
  },
];
