import { describe, expect, it, vi } from "vitest";

import { copyGeneratedAsset, type BinaryClipboard } from "./generated-asset-actions";

class FixtureClipboardItem {
  constructor(readonly items: Record<string, Blob>) {}
}

describe("generated asset actions", () => {
  it("copies only a successful PNG from the opaque preview protocol", async () => {
    const write = vi.fn().mockResolvedValue(undefined);
    const fetcher = vi.fn().mockResolvedValue(new Response(new Blob(["png"], { type: "image/png" })));

    const copied = await copyGeneratedAsset(
      "bottie-generated-asset://asset-1",
      fetcher,
      { write } as BinaryClipboard,
      FixtureClipboardItem as unknown as typeof ClipboardItem,
    );

    expect(copied).toBe(true);
    expect(fetcher).toHaveBeenCalledWith("bottie-generated-asset://asset-1", {
      method: "GET",
      cache: "no-store",
    });
    expect(write).toHaveBeenCalledOnce();
  });

  it("rejects non-opaque locations, non-PNG bytes, and unavailable clipboards", async () => {
    const fetcher = vi.fn().mockResolvedValue(new Response(new Blob(["text"], { type: "text/plain" })));
    const clipboard = { write: vi.fn() } as BinaryClipboard;
    const item = FixtureClipboardItem as unknown as typeof ClipboardItem;

    expect(await copyGeneratedAsset("https://secret.example/image.png", fetcher, clipboard, item)).toBe(false);
    expect(await copyGeneratedAsset("bottie-generated-asset://asset-1", fetcher, clipboard, item)).toBe(false);
    expect(await copyGeneratedAsset("bottie-generated-asset://asset-1", fetcher, undefined, item)).toBe(false);
    expect(clipboard.write).not.toHaveBeenCalled();
  });
});
