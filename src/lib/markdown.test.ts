import { describe, expect, it } from "vitest";

import { renderAssistantMarkdown } from "./markdown";

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
