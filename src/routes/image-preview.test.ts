import { describe, expect, it } from "vitest";

import { PageState } from "./page-state.svelte";
import { applyImagePreview, imagePreviewRequested } from "./image-preview";

describe("image preview", () => {
  it("enables only the explicit edit-lineage development fixture", () => {
    expect(imagePreviewRequested("?image=edit-lineage")).toBe(true);
    expect(imagePreviewRequested("?image=other")).toBe(false);
    expect(imagePreviewRequested("")).toBe(false);
  });

  it("applies one path-free mixed-source edit beside an ordinary generated image", () => {
    const state = new PageState();

    expect(applyImagePreview(state, "?image=edit-lineage")).toBe(true);
    expect(state.messages).toHaveLength(2);
    expect(state.messages[0].generatedAssets?.[0].sources.map((source) => source.sourceType)).toEqual([
      "attachment",
      "generated_asset",
    ]);
    expect(state.messages[1].generatedAssets?.[0].sources).toEqual([]);
  });
});
