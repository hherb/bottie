import { describe, expect, it } from "vitest";

import { performancePreviewRequested } from "./performance-preview";

describe("performance preview", () => {
  it("enables only the explicit long-history development fixture query", () => {
    expect(performancePreviewRequested("?performance=long-history")).toBe(true);
    expect(performancePreviewRequested("?performance=other")).toBe(false);
    expect(performancePreviewRequested("")).toBe(false);
  });
});
