import { describe, expect, it } from "vitest";

import { imageGenerationRequestOptions, prepareImageGenerationPrompt } from "./image-generation";

describe("image generation presentation", () => {
  it("maps every Cloud aspect choice to exact bounded provider options", () => {
    expect(imageGenerationRequestOptions("cloud", "square", 2)).toEqual({
      width: 2_048,
      height: 2_048,
      count: 2,
    });
    expect(imageGenerationRequestOptions("cloud", "landscape", 3)).toEqual({
      width: 2_688,
      height: 1_536,
      count: 3,
    });
    expect(imageGenerationRequestOptions("cloud", "portrait", 4)).toEqual({
      width: 1_536,
      height: 2_688,
      count: 4,
    });
  });

  it("forces the selected local route to its one proved output shape", () => {
    expect(imageGenerationRequestOptions("local", "landscape", 6)).toEqual({
      width: 512,
      height: 512,
      count: 1,
    });
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
