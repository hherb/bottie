/** Pure path-free presentation of successful native Web-tool provenance. */

import type { Message } from "./presentation";
import type { StoredToolInvocation } from "./storage";

/** One removable web source shown in the active conversation's Context panel. */
export type WebSource = {
  id: string;
  kind: "search" | "fetch";
  label: "Search result" | "Fetched page";
  title: string;
  url: string;
  host: string;
  excerpt: string;
  publishedAt: string | null;
};

type UnknownRecord = Record<string, unknown>;

const NON_PUBLIC_HOST_SUFFIXES = [
  ".home.arpa",
  ".internal",
  ".invalid",
  ".lan",
  ".local",
  ".localdomain",
  ".localhost",
  ".onion",
  ".test",
];

/** Derives unique visible sources from successful selected-lineage native Web results. */
export function webSourcesForMessages(messages: Message[], dismissedIds: ReadonlySet<string> = new Set()): WebSource[] {
  const sources = new Map<string, WebSource>();
  for (const message of messages.toReversed()) {
    for (const tool of (message.toolInvocations ?? []).toReversed()) {
      for (const source of sourcesForTool(tool)) {
        if (!dismissedIds.has(source.id) && !sources.has(source.id)) sources.set(source.id, source);
      }
    }
  }
  return [...sources.values()];
}

/** Extracts sources only from a completed, successful native execution envelope. */
function sourcesForTool(tool: StoredToolInvocation): WebSource[] {
  const result = successfulResult(tool);
  if (!result) return [];
  if (tool.toolName === "web_search") return searchSources(result);
  if (tool.toolName === "web_fetch") {
    const source = fetchSource(result);
    return source ? [source] : [];
  }
  return [];
}

/** Returns the structured result inside an exact successful dispatcher envelope. */
function successfulResult(tool: StoredToolInvocation): UnknownRecord | null {
  if (!tool.result || tool.result.isError || !isRecord(tool.result.output)) return null;
  if (tool.result.output.ok !== true || !isRecord(tool.result.output.result)) return null;
  return tool.result.output.result;
}

/** Parses one normalized native search result set without retaining provider or query fields. */
function searchSources(result: UnknownRecord): WebSource[] {
  if (!requiredString(result.providerId) || !Array.isArray(result.results)) return [];
  return result.results.flatMap((item) => {
    if (!isRecord(item)) return [];
    const title = requiredString(item.title);
    const excerpt = requiredString(item.snippet);
    const location = safeWebLocation(item.url);
    if (!title || !excerpt || !location) return [];
    return [sourceCard("search", "Search result", title, excerpt, optionalString(item.publishedAt), location)];
  });
}

/** Parses one normalized native fetch result only when its untrusted-content marker is present. */
function fetchSource(result: UnknownRecord): WebSource | null {
  if (result.untrusted !== true) return null;
  const location = safeWebLocation(result.sourceUrl);
  const excerpt = requiredString(result.content);
  if (!location || !excerpt) return null;
  return sourceCard(
    "fetch",
    "Fetched page",
    optionalString(result.title) ?? location.host,
    excerpt,
    optionalString(result.publishedAt),
    location,
  );
}

/** Builds one card from the deliberately narrow inert presentation fields. */
function sourceCard(
  kind: WebSource["kind"],
  label: WebSource["label"],
  title: string,
  excerpt: string,
  publishedAt: string | null,
  location: { url: string; host: string },
): WebSource {
  return {
    id: `web:${location.url}`,
    kind,
    label,
    title,
    url: location.url,
    host: location.host,
    excerpt,
    publishedAt,
  };
}

/** Accepts only credential-free public-looking HTTP(S) locations and removes fragments for stable identity. */
function safeWebLocation(value: unknown): { url: string; host: string } | null {
  if (typeof value !== "string") return null;
  try {
    const parsed = new URL(value);
    const usesWebProtocol = parsed.protocol === "http:" || parsed.protocol === "https:";
    if (!usesWebProtocol || parsed.username || parsed.password || parsed.port) return null;
    const host = parsed.hostname.toLowerCase().replace(/\.$/, "");
    if (!host || !host.includes(".") || host === "localhost" || isNonPublicHost(host) || isIpLiteral(host)) return null;
    parsed.hostname = host;
    parsed.hash = "";
    return { url: parsed.href, host };
  } catch {
    return null;
  }
}

/** Rejects common reserved or local-only DNS suffixes that cannot represent native public-Web results. */
function isNonPublicHost(host: string): boolean {
  return NON_PUBLIC_HOST_SUFFIXES.some((suffix) => host.endsWith(suffix));
}

/** Rejects IPv4 and bracketed IPv6 literals so legacy payloads cannot create local navigation targets. */
function isIpLiteral(host: string): boolean {
  return /^\d{1,3}(?:\.\d{1,3}){3}$/.test(host) || (host.startsWith("[") && host.endsWith("]"));
}

/** Narrows unknown JSON into a non-array object. */
function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Accepts only non-empty native strings for visible presentation. */
function requiredString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

/** Keeps optional native strings while dropping empty or malformed values. */
function optionalString(value: unknown): string | null {
  return requiredString(value);
}
