import { describe, expect, it } from "vitest";

import { assistantMarkdownLinkDestinations, renderAssistantMarkdown } from "./markdown";

describe("assistant Markdown rendering", () => {
  it("renders common answer structure and fenced code", () => {
    const rendered = renderAssistantMarkdown(
      "## Plan\n\n- Keep **history**\n- Add `rendering`\n\n```ts\nconst safe = true;\n```",
    );

    expect(rendered).toContain("<h2>Plan</h2>");
    expect(rendered).toContain("<strong>history</strong>");
    expect(rendered).toContain("<code>rendering</code>");
    expect(rendered).toContain('<code class="language-ts">const safe = true;');
  });

  it("renders compact tables used in structured answers", () => {
    const rendered = renderAssistantMarkdown("| Item | State |\n| --- | --- |\n| Markdown | Safe |");

    expect(rendered).toContain("<table>");
    expect(rendered).toContain("<th>Item</th>");
    expect(rendered).toContain("<td>Safe</td>");
  });

  it("escapes provider-supplied HTML instead of trusting it", () => {
    const rendered = renderAssistantMarkdown('<script>alert("no")</script><img src=x onerror=alert(1)>');

    expect(rendered).not.toContain("<script>");
    expect(rendered).not.toContain("<img");
    expect(rendered).toContain("&lt;script&gt;");
  });

  it("allows explicit web links with isolated browsing context", () => {
    const rendered = renderAssistantMarkdown("[Bottie](https://example.com/docs?q=1&lang=en)");

    expect(rendered).toContain('href="https://example.com/docs?q=1&amp;lang=en"');
    expect(rendered).toContain('target="_blank"');
    expect(rendered).toContain('rel="noopener noreferrer"');
  });

  it("marks only links backed by retained Web results as claim citations", () => {
    const rendered = renderAssistantMarkdown(
      "Rust 1.90 is stable [release notes](https://blog.rust-lang.org/releases/1.90/#details). " +
        "See [another page](https://example.com/other).",
      new Set(["https://blog.rust-lang.org/releases/1.90/"]),
    );

    expect(rendered).toContain('href="https://blog.rust-lang.org/releases/1.90/#details"');
    expect(rendered).toContain('class="web-citation-link"');
    expect(rendered).toContain('data-web-citation="true"');
    expect(rendered).toContain('title="Web citation retained with this response"');
    expect(rendered.match(/data-web-citation/g)).toHaveLength(1);
  });

  it("extracts normalized HTTP citations without treating unsafe or email links as Web sources", () => {
    expect(
      assistantMarkdownLinkDestinations(
        "[release](https://BLOG.RUST-LANG.ORG/releases/1.90/#details) " +
          "[mail](mailto:team@example.com) [unsafe](javascript:alert(1))",
      ),
    ).toEqual(new Set(["https://blog.rust-lang.org/releases/1.90/"]));
  });

  it("neutralizes unsafe links and remote Markdown images", () => {
    const rendered = renderAssistantMarkdown('[run](javascript:alert("no")) ![tracker](https://example.com/pixel.gif)');

    expect(rendered).not.toContain('href="javascript:');
    expect(rendered).not.toContain("<img");
    expect(rendered).toContain("run");
    expect(rendered).toContain("tracker");
  });

  it("returns an empty string for an empty streamed response", () => {
    expect(renderAssistantMarkdown("")).toBe("");
  });
});
