/** Exhaustive control-grid checks keep labels, bounds, and the Rust server gate in agreement. */
import nativeDimensionsSource from "../../src-tauri/src/local_image_worker/image_dimensions.rs?raw";
import { describe, expect, it } from "vitest";
import {
  IMAGE_ASPECT_CHOICES,
  IMAGE_RESOLUTION_CHOICES,
  LOCAL_IMAGE_21_PACKAGE_ID,
  imageGenerationRequestOptions,
} from "./image-options";

describe("Image settings presets", () => {
  it("uses exact aspect ratios, aligned axes, and bounded areas for every option", () => {
    const ratios = [1, 3 / 2, 2 / 3, 16 / 9, 9 / 16];
    for (const execution of ["local", "cloud"] as const) {
      for (const resolution of IMAGE_RESOLUTION_CHOICES) {
        IMAGE_ASPECT_CHOICES.forEach(({ value }, index) => {
          const { width, height, count } = imageGenerationRequestOptions(
            execution,
            value,
            6,
            resolution,
            LOCAL_IMAGE_21_PACKAGE_ID,
          );
          expect(width / height).toBeCloseTo(ratios[index]);
          expect(width % 32).toBe(0);
          expect(height % 32).toBe(0);
          expect(width * height).toBeGreaterThanOrEqual(512 * 512);
          expect(width * height).toBeLessThanOrEqual(2048 * 2048);
          expect(count).toBe(execution === "local" ? 1 : 6);
        });
      }
    }
  });
  it("offers precisely the dimension set admitted by the native 2.1 gate", () => {
    const nativeDimensions = [...nativeDimensionsSource.matchAll(/\((\d+), (\d+)\)/g)]
      .map((match) => `${match[1]}x${match[2]}`)
      .sort();
    const controls = IMAGE_RESOLUTION_CHOICES.flatMap((resolution) =>
      IMAGE_ASPECT_CHOICES.map(({ value }) => {
        const { width, height } = imageGenerationRequestOptions(
          "local",
          value,
          1,
          resolution,
          LOCAL_IMAGE_21_PACKAGE_ID,
        );
        return `${width}x${height}`;
      }),
    ).sort();
    expect(controls).toEqual(nativeDimensions);
  });
});
