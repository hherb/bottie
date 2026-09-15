import { readFileSync } from "node:fs";
import MarkdownIt from "markdown-it";
import { describe, expect, it } from "vitest";

const manualSource = readFileSync(new URL("../docs/manual/developers/index.md", import.meta.url), "utf8");
const markdown = new MarkdownIt();

describe("developer manual Markdown", () => {
  it("keeps the nullable C# translation in one two-column row", () => {
    const rendered = markdown.render(manualSource);

    expect(rendered).toContain(
      "<td>nullable reference</td>\n<td>TypeScript <code>T | null</code>; Rust <code>Option&lt;T&gt;</code></td>",
    );
  });
});
