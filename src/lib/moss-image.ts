/** Closed MOSS image package identities and the native UTF-8 prompt limit. */
export const MOSS_IMAGE_MODEL_IDS = [
  "mlx-community/MOSS-Image-SFT-bf16",
  "mlx-community/MOSS-Image-Base-bf16",
  "OpenMOSS-Team/MOSS-Image-SFT",
  "OpenMOSS-Team/MOSS-Image-Base",
] as const;
export const MOSS_IMAGE_MAX_PROMPT_BYTES = 32 * 1024;
/** Unknown aliases retain the existing Qwen limits and cannot opt into MOSS behavior. */
export function isMossImageModel(id?: string): boolean {
  if (!id) return false;
  const [repository, revision, ...extra] = id.split("@");
  if (extra.length || (revision !== undefined && !/^[a-f0-9]{40}$/.test(revision))) return false;
  return MOSS_IMAGE_MODEL_IDS.some((model) => model === repository);
}
