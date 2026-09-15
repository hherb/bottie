/** Pure presentation contracts for explicit Cloud and local image generation. */

/** User-selected execution boundary for a newly submitted image request. */
export type ImageGenerationExecution = "cloud" | "local";

/** Fixed Qwen-Image-2.0 size choices exposed by the composer. */
export type ImageGenerationSize = "square" | "landscape" | "portrait";

/** Result of mirroring native image-prompt normalization before durable user-message insertion. */
export type PreparedImageGenerationPrompt = { ok: true; prompt: string } | { ok: false; message: string };

const MAX_IMAGE_PROMPT_CHARACTERS = 1_000;
const CONTROL_CHARACTER = /\p{Cc}/u;

/** Normalizes and bounds a prompt while Rust remains the authoritative request validator. */
export function prepareImageGenerationPrompt(prompt: string): PreparedImageGenerationPrompt {
  const normalized = prompt.replaceAll("\r\n", "\n").trim();
  if (normalized.length === 0) return { ok: false, message: "Enter a non-empty image description." };
  const characters = Array.from(normalized);
  const hasUnsupportedControl = characters.some(
    (character) => CONTROL_CHARACTER.test(character) && character !== "\n" && character !== "\t",
  );
  if (characters.length > MAX_IMAGE_PROMPT_CHARACTERS || hasUnsupportedControl) {
    return {
      ok: false,
      message: "The image description is too long or contains unsupported control characters.",
    };
  }
  return { ok: true, prompt: normalized };
}

/** Resolves one route and visible Cloud choice to the exact native request options. */
export function imageGenerationRequestOptions(
  execution: ImageGenerationExecution,
  size: ImageGenerationSize,
  count: number,
): { width: number; height: number; count: number } {
  if (execution === "local") return { width: 512, height: 512, count: 1 };
  switch (size) {
    case "landscape":
      return { width: 2_688, height: 1_536, count };
    case "portrait":
      return { width: 1_536, height: 2_688, count };
    default:
      return { width: 2_048, height: 2_048, count };
  }
}
