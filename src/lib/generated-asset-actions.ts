/** Clipboard helpers for already-bounded generated-image preview bytes. */

/** Minimal binary clipboard surface used by generated-image copying. */
export type BinaryClipboard = {
  write(items: ClipboardItem[]): Promise<void>;
};

/** Copies one normalized PNG preview without accepting native paths or provider URLs. */
export async function copyGeneratedAsset(
  previewUrl: string,
  fetcher: typeof fetch = fetch,
  clipboard: BinaryClipboard | undefined = globalThis.navigator?.clipboard,
  clipboardItem: typeof ClipboardItem | undefined = globalThis.ClipboardItem,
): Promise<boolean> {
  if (!clipboard || !clipboardItem || !previewUrl.startsWith("bottie-generated-asset://")) return false;
  try {
    const response = await fetcher(previewUrl, { method: "GET", cache: "no-store" });
    if (!response.ok) return false;
    const blob = await response.blob();
    if (blob.type !== "image/png") return false;
    await clipboard.write([new clipboardItem({ "image/png": blob })]);
    return true;
  } catch {
    return false;
  }
}
