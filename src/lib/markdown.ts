import MarkdownIt from "markdown-it";

/** Protocols that may leave the app through an assistant-authored Markdown link. */
const SAFE_LINK_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);
const WEB_LINK_PROTOCOLS = new Set(["http:", "https:"]);

/** Renderer environment carrying exact durable Web-source URLs for one response. */
type MarkdownEnvironment = {
  webCitationUrls?: ReadonlySet<string>;
};

/** Minimal token surface shared by MarkdownIt's bundled and compatibility typings. */
type MarkdownToken = {
  type: string;
  children: MarkdownToken[] | null;
  attrGet(name: string): string | number | null;
};

/** Escapes text inserted by custom renderer rules into generated HTML. */
function escapeHtml(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

/** Returns whether a Markdown destination is an explicit, permitted external URL. */
function isSafeLink(destination: string): boolean {
  try {
    const url = new URL(destination);
    return SAFE_LINK_PROTOCOLS.has(url.protocol);
  } catch {
    return false;
  }
}

/** Normalizes one HTTP(S) destination for comparison with native Web provenance. */
function normalizedWebDestination(destination: string): string | null {
  try {
    const url = new URL(destination);
    if (!WEB_LINK_PROTOCOLS.has(url.protocol)) return null;
    url.hash = "";
    return url.href;
  } catch {
    return null;
  }
}

/** Adds normalized link destinations from nested Markdown tokens to one result set. */
function collectLinkDestinations(tokens: MarkdownToken[], destinations: Set<string>): void {
  for (const token of tokens) {
    if (token.type === "link_open") {
      const destination = token.attrGet("href");
      const normalized = typeof destination === "string" ? normalizedWebDestination(destination) : null;
      if (normalized) destinations.add(normalized);
    }
    if (token.children) collectLinkDestinations(token.children, destinations);
  }
}

/** Configured Markdown parser that emits only parser-owned, policy-limited markup. */
const markdown = new MarkdownIt({
  breaks: false,
  html: false,
  linkify: true,
  typographer: false,
});

markdown.validateLink = isSafeLink;

const defaultLinkOpen = markdown.renderer.rules.link_open;
markdown.renderer.rules.link_open = (tokens, index, options, environment, renderer) => {
  const token = tokens[index];
  token.attrSet("target", "_blank");
  token.attrSet("rel", "noopener noreferrer");
  const destination = token.attrGet("href");
  const normalized = typeof destination === "string" ? normalizedWebDestination(destination) : null;
  const citationUrls = (environment as MarkdownEnvironment | undefined)?.webCitationUrls;
  if (normalized && citationUrls?.has(normalized)) {
    token.attrJoin("class", "web-citation-link");
    token.attrSet("data-web-citation", "true");
    token.attrSet("title", "Web citation retained with this response");
  }
  return defaultLinkOpen
    ? defaultLinkOpen(tokens, index, options, environment, renderer)
    : renderer.renderToken(tokens, index, options);
};

markdown.renderer.rules.image = (tokens, index) => {
  const alt = tokens[index].content.trim();
  return `<span class="markdown-image-alt">[Image${alt ? `: ${escapeHtml(alt)}` : ""}]</span>`;
};

/** Extracts normalized HTTP(S) link destinations from parser-accepted assistant Markdown. */
export function assistantMarkdownLinkDestinations(source: string): ReadonlySet<string> {
  const destinations = new Set<string>();
  collectLinkDestinations(markdown.parse(source, {}), destinations);
  return destinations;
}

/** Renders an assistant answer as sanitized, parser-owned HTML with exact native citation matches. */
export function renderAssistantMarkdown(source: string, webCitationUrls: ReadonlySet<string> = new Set()): string {
  if (source === "") return "";
  return markdown.render(source, { webCitationUrls });
}
