import { describe, expect, it, vi } from "vitest";

import { assistantResponseMarkdown, copyAssistantResponse } from "./clipboard";

describe("assistant response copying", () => {
  it("keeps responses without reasoning byte-for-byte unchanged", () => {
    const response = "## Result\n\nUse **local** inference.";

    expect(assistantResponseMarkdown({ content: response })).toBe(response);
  });

  it("builds labelled Markdown sections when reasoning is present", () => {
    expect(
      assistantResponseMarkdown({
        content: "## Result\n\nUse **local** inference.",
        reasoning: "Checked the **local-first** requirement.",
      }),
    ).toBe(
      "## Reasoning\n\nChecked the **local-first** requirement.\n\n" +
        "## Response\n\n## Result\n\nUse **local** inference.",
    );
  });

  it("copies the composed Markdown without rendered HTML or metadata", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const response = { content: "Answer", reasoning: "Private working" };

    await expect(copyAssistantResponse(response, { writeText })).resolves.toBe(true);
    expect(writeText).toHaveBeenCalledOnce();
    expect(writeText).toHaveBeenCalledWith("## Reasoning\n\nPrivate working\n\n## Response\n\nAnswer");
  });

  it("reports an unavailable clipboard without throwing", async () => {
    await expect(copyAssistantResponse({ content: "Answer" }, undefined)).resolves.toBe(false);
  });

  it("reports a rejected clipboard write without throwing", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("permission denied"));

    await expect(copyAssistantResponse({ content: "Answer" }, { writeText })).resolves.toBe(false);
  });
});
