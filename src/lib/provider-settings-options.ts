/** Static provider choices and connection-policy copy for the Settings presentation. */

import type { ProviderId, WebSearchProviderId } from "./inference";

/** Provider endpoint cards rendered by the Settings form. */
export const PROVIDER_SETTINGS: Array<{
  id: ProviderId;
  name: string;
  description: string;
  route: "local" | "cloud";
}> = [
  { id: "omlx", name: "oMLX", description: "OpenAI-compatible local runtime", route: "local" },
  { id: "ollama", name: "Ollama", description: "Native local API", route: "local" },
  { id: "openai", name: "OpenAI compatible", description: "Chat Completions over HTTPS", route: "cloud" },
  { id: "anthropic", name: "Anthropic compatible", description: "Messages API over HTTPS", route: "cloud" },
];

/** Fixed search-provider routes rendered by the Settings form. */
export const SEARCH_PROVIDER_SETTINGS: Array<{
  id: WebSearchProviderId;
  name: string;
  hostname: string;
}> = [
  { id: "brave", name: "Brave Search", hostname: "api.search.brave.com" },
  { id: "exa", name: "Exa Search", hostname: "api.exa.ai" },
];

/** Concise native network boundary shown alongside provider configuration. */
export const CONNECTION_POLICY = [
  "Local loopback or remote HTTPS",
  "redirects disabled",
  "3 s connect",
  "5 s discovery",
  "15 s web test",
  "120 s stream idle",
].join(" · ");
