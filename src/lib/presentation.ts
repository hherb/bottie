import type { LocalProviderId, ProviderSettings } from "./inference";

/** One message rendered in the prototype conversation. */
export type Message = {
  id: number;
  role: "user" | "assistant";
  content: string;
  reasoning?: string;
  featured?: boolean;
  model?: string;
  meta?: string;
  error?: boolean;
};

/** Browser-side attachment metadata used by the presentation-only attachment preview. */
export type Attachment = {
  id: number;
  name: string;
  size: string;
  kind: "image" | "file";
};

/** Native application identity displayed in the sidebar. */
export type RuntimeInfo = {
  name: string;
  version: string;
  storage: string;
};

/** Local-provider connection state displayed throughout the shell. */
export type ProviderStatus = "checking" | "available" | "offline" | "browser";

/** One normalized stage in the live inference activity presentation. */
export type InferenceStage = {
  icon: "shield" | "sparkles";
  label: string;
  detail: string;
};

/** Local-provider choices displayed in the provider selector. */
export const PROVIDER_OPTIONS: Array<{ id: LocalProviderId; name: string }> = [
  { id: "ollama", name: "Ollama" },
  { id: "omlx", name: "oMLX" },
];

/** Maximum number of attachment chips shown above the composer. */
export const MAX_COMPOSER_ATTACHMENTS = 3;

/** Maximum automatic composer height before internal scrolling begins. */
export const MAX_COMPOSER_HEIGHT_PX = 160;

/** Browser-preview defaults mirrored from the native local-provider configuration. */
export const DEFAULT_LOCAL_PROVIDER_SETTINGS: ProviderSettings = {
  omlxBaseUrl: "http://127.0.0.1:8000/",
  ollamaBaseUrl: "http://127.0.0.1:11434/",
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
      "Absolutely. I’d build bottie as a sequence of small, complete slices—starting with the conversation " +
      "experience, then connecting inference, persistence, and tools behind it.\n\nThe important boundary is " +
      "simple: the WebView presents state; the Rust core owns secrets, files, storage, provider calls, and " +
      "tool execution.",
  },
];
