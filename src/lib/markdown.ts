import MarkdownIt from "markdown-it";

/** Protocols that may leave the app through an assistant-authored Markdown link. */
const SAFE_LINK_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);

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
  tokens[index].attrSet("target", "_blank");
  tokens[index].attrSet("rel", "noopener noreferrer");
  return defaultLinkOpen
    ? defaultLinkOpen(tokens, index, options, environment, renderer)
    : renderer.renderToken(tokens, index, options);
};

markdown.renderer.rules.image = (tokens, index) => {
  const alt = tokens[index].content.trim();
  return `<span class="markdown-image-alt">[Image${alt ? `: ${escapeHtml(alt)}` : ""}]</span>`;
};

/** Renders an assistant answer as sanitized, parser-owned HTML. */
export function renderAssistantMarkdown(source: string): string {
  if (source === "") return "";
  return markdown.render(source);
}
