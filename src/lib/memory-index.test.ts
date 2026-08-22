import { describe, expect, it } from "vitest";

import { memoryIndexCopy, memoryIndexIsActive, memoryIndexPercent, type SemanticIndexProgress } from "./memory-index";

const ready: SemanticIndexProgress = {
  state: "ready",
  completedChunks: 84,
  totalChunks: 84,
  errorCode: null,
};

describe("semantic memory index presentation", () => {
  it("reports durable completion without exposing native details", () => {
    expect(memoryIndexCopy(ready)).toBe("Ready · 84 of 84 chunks");
    expect(memoryIndexPercent(ready)).toBe(100);
    expect(memoryIndexIsActive(ready)).toBe(false);
  });

  it("keeps bounded progress honest while model work is active", () => {
    const progress: SemanticIndexProgress = {
      state: "indexing",
      completedChunks: 9,
      totalChunks: 20,
      errorCode: null,
    };

    expect(memoryIndexCopy(progress)).toBe("Indexing · 9 of 20 chunks");
    expect(memoryIndexPercent(progress)).toBe(45);
    expect(memoryIndexIsActive(progress)).toBe(true);
  });

  it("describes model preparation, empty indexes, and path-free failures", () => {
    expect(memoryIndexCopy({ ...ready, state: "loading_model", completedChunks: 0 })).toBe(
      "Preparing local model · 0 of 84 chunks",
    );
    expect(memoryIndexCopy({ ...ready, completedChunks: 0, totalChunks: 0 })).toBe("Ready · no eligible chunks");
    expect(memoryIndexCopy({ ...ready, state: "failed", completedChunks: 12, errorCode: "model_runtime" })).toBe(
      "Paused · 12 of 84 chunks · local model unavailable",
    );
  });

  it("clamps percentages from defensive native progress values", () => {
    expect(memoryIndexPercent({ ...ready, completedChunks: 90 })).toBe(100);
    expect(memoryIndexPercent({ ...ready, completedChunks: 0, totalChunks: 0 })).toBe(100);
  });
});
