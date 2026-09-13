import { describe, expect, it } from "vitest";

import { imageGenerationDimensions, prepareImageGenerationPrompt } from "./image-generation";

describe("image generation presentation", () => {
  it("maps every visible aspect choice to the exact bounded provider dimensions", () => {
    expect(imageGenerationDimensions("square")).toEqual({ width: 2_048, height: 2_048 });
    expect(imageGenerationDimensions("landscape")).toEqual({ width: 2_688, height: 1_536 });
    expect(imageGenerationDimensions("portrait")).toEqual({ width: 1_536, height: 2_688 });
  });

  it("normalizes the same bounded prompt shape before native persistence", () => {
    expect(prepareImageGenerationPrompt("  Draw Bottie\r\nwith a lantern\t ")).toEqual({
      ok: true,
      prompt: "Draw Bottie\nwith a lantern",
    });
  });

  it("rejects empty oversized and unsupported control-character prompts", () => {
    expect(prepareImageGenerationPrompt(" \n\t ")).toEqual({
      ok: false,
      message: "Enter a non-empty image description.",
    });
    expect(prepareImageGenerationPrompt("🎨".repeat(1_001))).toEqual({
      ok: false,
      message: "The image description is too long or contains unsupported control characters.",
    });
    expect(prepareImageGenerationPrompt("Draw\u0000Bottie")).toEqual({
      ok: false,
      message: "The image description is too long or contains unsupported control characters.",
    });
  });
});
