/** Pure presentation contracts for explicit hosted image generation. */

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
