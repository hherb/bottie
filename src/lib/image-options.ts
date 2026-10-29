/** Closed size/aspect presets shared by the image controls and submitted request snapshot. */

import { isMossImageModel } from "./moss-image";

/** Visible image shape; dimensions always have the exact labelled ratio. */
export type ImageGenerationSize = "square" | "landscape" | "portrait" | "wide" | "tall";
/** Resolution tier independent from the selected aspect ratio. */
export type ImageGenerationResolution = "small" | "medium" | "large";
/** Execution boundary of the explicit image action. */
export type ImageGenerationExecution = "cloud" | "local";

/** Exact adjustable local pack, matching Rust's model-bound dimension gate. */
export const LOCAL_IMAGE_21_PACKAGE_ID = "ddalcu/Qwen-Image-2.1-MLX-Serve-8bit";
/** Labels describe exact output ratios, including portrait orientations. */
export const IMAGE_ASPECT_CHOICES: readonly { value: ImageGenerationSize; label: string }[] = [
  { value: "square", label: "Square · 1:1" },
  { value: "landscape", label: "Landscape · 3:2" },
  { value: "portrait", label: "Portrait · 2:3" },
  { value: "wide", label: "Wide · 16:9" },
  { value: "tall", label: "Tall · 9:16" },
];
/** Ordered output tiers shown with the selected aspect's concrete dimensions. */
export const IMAGE_RESOLUTION_CHOICES: readonly ImageGenerationResolution[] = ["small", "medium", "large"];

type Dimensions = readonly [number, number];
/** Square, landscape, and wide base dimensions; portrait choices reverse their axes. */
type Tier = readonly [Dimensions, Dimensions, Dimensions];
const LOCAL_TIERS: Record<ImageGenerationResolution, Tier> = {
  small: [
    [512, 512],
    [768, 512],
    [1024, 576],
  ],
  medium: [
    [768, 768],
    [1152, 768],
    [1536, 864],
  ],
  large: [
    [1024, 1024],
    [1536, 1024],
    [2048, 1152],
  ],
};
const MOSS_TIERS: Record<ImageGenerationResolution, Tier> = {
  ...LOCAL_TIERS,
  large: [
    [1024, 1024],
    [1536, 1024],
    [1792, 1008],
  ],
};
const CLOUD_TIERS: Record<ImageGenerationResolution, Tier> = {
  small: [
    [1024, 1024],
    [1152, 768],
    [1536, 864],
  ],
  medium: [
    [1536, 1536],
    [1728, 1152],
    [2048, 1152],
  ],
  large: [
    [2048, 2048],
    [2496, 1664],
    [2560, 1440],
  ],
};

/** Only the preferred server pack admits new sizes; the qualified legacy worker stays fixed. */
export function imageDimensionsAdjustable(execution: ImageGenerationExecution, modelId?: string): boolean {
  return execution === "cloud" || modelId === LOCAL_IMAGE_21_PACKAGE_ID || isMossImageModel(modelId);
}

/** Uses a modest local default and preserves the existing cloud square resolution. */
export function defaultImageResolution(execution: ImageGenerationExecution): ImageGenerationResolution {
  return execution === "local" ? "small" : "large";
}

/** Resolves controls to bounded native options; local requests always produce one image. */
export function imageGenerationRequestOptions(
  execution: ImageGenerationExecution,
  aspect: ImageGenerationSize,
  count: number,
  resolution: ImageGenerationResolution = defaultImageResolution(execution),
  modelId?: string,
): { width: number; height: number; count: number } {
  if (!imageDimensionsAdjustable(execution, modelId)) return { width: 512, height: 512, count: 1 };
  const tiers = execution === "local" ? (isMossImageModel(modelId) ? MOSS_TIERS : LOCAL_TIERS) : CLOUD_TIERS;
  const tier = tiers[resolution];
  const index = aspect === "square" ? 0 : aspect === "wide" || aspect === "tall" ? 2 : 1;
  const [width, height] = tier[index];
  return {
    width: aspect === "portrait" || aspect === "tall" ? height : width,
    height: aspect === "portrait" || aspect === "tall" ? width : height,
    count: execution === "local" ? 1 : count,
  };
}
