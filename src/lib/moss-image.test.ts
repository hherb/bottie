/** Model-specific image bounds keep MOSS layouts distinct from hosted Qwen requests. */
import nativeMossSource from "../../src-tauri/src/local_image_worker/moss.rs?raw";
import { describe, expect, it } from "vitest";
import { prepareImageGenerationPrompt } from "./image-generation";
import {
  IMAGE_ASPECT_CHOICES,
  IMAGE_RESOLUTION_CHOICES,
  imageGenerationRequestOptions,
  imageDimensionsAdjustable,
} from "./image-options";
import { localImageModelLabel } from "./local-image";

const MODEL = "mlx-community/MOSS-Image-SFT-bf16";
describe("MOSS image controls", () => {
  it("admits bounded long layouts only for MOSS", () => {
    expect(prepareImageGenerationPrompt("a".repeat(2000), MODEL).ok).toBe(true);
    expect(prepareImageGenerationPrompt("a".repeat(2000), MODEL + "@" + "a".repeat(40)).ok).toBe(true);
    expect(prepareImageGenerationPrompt("a".repeat(2000)).ok).toBe(false);
    expect(prepareImageGenerationPrompt("🎨".repeat(8193), MODEL).ok).toBe(false);
  });
  it("offers exact ratios within the port's 1456-square area envelope", () => {
    expect(imageDimensionsAdjustable("local", MODEL)).toBe(true);
    expect(imageGenerationRequestOptions("local", "wide", 6, "large", MODEL)).toEqual({
      width: 1792,
      height: 1008,
      count: 1,
    });
    expect(localImageModelLabel({ modelId: MODEL } as never)).toBe("MOSS Image SFT");
  });
});

describe("MOSS disclosure", () => {
  it("does not describe an unmeasured footprint as zero measured peak memory", async () => {
    const { localImageAvailabilityPresentation } = await import("./local-image");
    const shown = localImageAvailabilityPresentation({
      modelId: MODEL,
      runtimeId: "moss-image-mlx@external",
      availability: "ready",
      expectedDiskBytes: 27219783878,
      requiredMemoryBytes: 0,
    } as never);
    expect(shown.detail).toContain("peak memory not measured");
    expect(shown.detail).not.toContain("0.0 GiB measured");
  });
});

it("offers exactly the MOSS dimensions admitted by Rust, all within the port envelope", () => {
  const native = [...nativeMossSource.matchAll(/\((\d+), (\d+)\)/g)].map((m) => `${m[1]}x${m[2]}`).sort();
  const controls = IMAGE_RESOLUTION_CHOICES.flatMap((tier) =>
    IMAGE_ASPECT_CHOICES.map(({ value }) => {
      const { width, height, count } = imageGenerationRequestOptions("local", value, 6, tier, MODEL);
      expect(width % 16).toBe(0);
      expect(height % 16).toBe(0);
      expect(width * height).toBeLessThanOrEqual(1456 ** 2);
      expect(count).toBe(1);
      return `${width}x${height}`;
    }),
  ).sort();
  expect(controls).toEqual(native);
});
