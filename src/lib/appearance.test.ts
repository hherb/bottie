import { describe, expect, it, vi } from "vitest";

import {
  APPEARANCE_STORAGE_KEY,
  DEFAULT_APPEARANCE,
  createAppearanceController,
  normalizeAppearance,
  readAppearance,
  resolveTheme,
} from "./appearance";

describe("appearance preferences", () => {
  it("keeps dark and comfortable as the migration-safe default", () => {
    expect(normalizeAppearance(null)).toEqual(DEFAULT_APPEARANCE);
    expect(normalizeAppearance({ theme: "sepia", density: "tiny" })).toEqual(DEFAULT_APPEARANCE);
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  it("reads only the closed persisted preference shape", () => {
    const storage = {
      getItem: vi.fn(() => JSON.stringify({ theme: "system", density: "compact", ignored: true })),
      setItem: vi.fn(),
    };

    expect(readAppearance(storage)).toEqual({ theme: "system", density: "compact" });
    expect(storage.getItem).toHaveBeenCalledWith(APPEARANCE_STORAGE_KEY);

    storage.getItem.mockReturnValueOnce("not json");
    expect(readAppearance(storage)).toEqual(DEFAULT_APPEARANCE);
  });

  it("persists updates and listens to system changes only in system mode", () => {
    const attributes = new Map<string, string>();
    const root = {
      setAttribute: vi.fn((name: string, value: string) => attributes.set(name, value)),
      style: { colorScheme: "" },
    };
    const storage = {
      getItem: vi.fn(() => JSON.stringify({ theme: "system", density: "comfortable" })),
      setItem: vi.fn(),
    };
    const listeners = new Set<() => void>();
    const mediaQuery = {
      matches: false,
      addEventListener: vi.fn((_name: string, listener: () => void) => listeners.add(listener)),
      removeEventListener: vi.fn((_name: string, listener: () => void) => listeners.delete(listener)),
    };
    const onChange = vi.fn();

    const controller = createAppearanceController({ root, storage, mediaQuery, onChange });
    expect(attributes).toEqual(
      new Map([
        ["data-theme", "light"],
        ["data-theme-preference", "system"],
        ["data-density", "comfortable"],
      ]),
    );
    expect(mediaQuery.addEventListener).toHaveBeenCalledOnce();

    mediaQuery.matches = true;
    listeners.forEach((listener) => listener());
    expect(attributes.get("data-theme")).toBe("dark");

    controller.update({ theme: "light", density: "compact" });
    expect(storage.setItem).toHaveBeenLastCalledWith(
      APPEARANCE_STORAGE_KEY,
      JSON.stringify({ theme: "light", density: "compact" }),
    );
    expect(attributes.get("data-theme")).toBe("light");
    expect(attributes.get("data-density")).toBe("compact");
    expect(mediaQuery.removeEventListener).toHaveBeenCalledOnce();

    mediaQuery.matches = false;
    listeners.forEach((listener) => listener());
    expect(attributes.get("data-theme")).toBe("light");
    expect(onChange).toHaveBeenLastCalledWith({ theme: "light", density: "compact" });
    controller.dispose();
  });
});
