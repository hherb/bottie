/** Pure presentation contracts for explicit hosted image generation. */

/** Fixed Qwen-Image-2.0 size choices exposed by the composer. */
export type ImageGenerationSize = "square" | "landscape" | "portrait";

/** Resolves one visible size choice to the exact provider request dimensions. */
export function imageGenerationDimensions(size: ImageGenerationSize): { width: number; height: number } {
  switch (size) {
    case "landscape":
      return { width: 2_688, height: 1_536 };
    case "portrait":
      return { width: 1_536, height: 2_688 };
    default:
      return { width: 2_048, height: 2_048 };
  }
}
