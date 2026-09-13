import { describe, expect, it } from "vitest";

import { imageGenerationDimensions } from "./image-generation";

describe("image generation presentation", () => {
  it("maps every visible aspect choice to the exact bounded provider dimensions", () => {
    expect(imageGenerationDimensions("square")).toEqual({ width: 2_048, height: 2_048 });
    expect(imageGenerationDimensions("landscape")).toEqual({ width: 2_688, height: 1_536 });
    expect(imageGenerationDimensions("portrait")).toEqual({ width: 1_536, height: 2_688 });
  });
});
