import { describe, expect, it } from "vitest";

import {
  commandForKeyboardEvent,
  commandPaletteItems,
  filterCommandPaletteItems,
  isCommandPaletteShortcut,
  nextEnabledCommandIndex,
} from "./command-palette";

describe("command palette registry", () => {
  it("builds the bounded action registry with platform-specific shortcuts and busy reasons", () => {
    const items = commandPaletteItems({
      busy: true,
      contextOpen: false,
      platform: "MacIntel",
      storageAvailable: true,
    });

    expect(items.map(({ id, label, shortcut }) => ({ id, label, shortcut }))).toEqual([
      { id: "new-chat", label: "New conversation", shortcut: "⌘ N" },
      { id: "search-conversations", label: "Search conversations", shortcut: "⌘ ⇧ F" },
      { id: "focus-navigation", label: "Focus conversation navigation", shortcut: "⌘ ⇧ B" },
      { id: "toggle-context", label: "Show context panel", shortcut: "⌘ ⇧ C" },
      { id: "open-settings", label: "Open Settings", shortcut: "⌘ ," },
    ]);
    expect(items[0].disabledReason).toBe("Finish the current response before starting a new conversation.");
    expect(items.slice(1).every((item) => item.disabledReason === undefined)).toBe(true);
  });

  it("filters locally across labels, descriptions, and keywords without exposing disabled actions as enabled", () => {
    const items = commandPaletteItems({
      busy: false,
      contextOpen: true,
      platform: "Win32",
      storageAvailable: false,
    });

    expect(filterCommandPaletteItems(items, "history").map((item) => item.id)).toEqual([
      "search-conversations",
      "focus-navigation",
    ]);
    expect(filterCommandPaletteItems(items, "context")[0].label).toBe("Hide context panel");
    expect(items[1].shortcut).toBe("Ctrl ⇧ F");
    expect(items[1].disabledReason).toBe("Conversation search is unavailable while local storage is unavailable.");
  });

  it("recognizes exact direct shortcuts without intercepting unrelated modified input", () => {
    expect(isCommandPaletteShortcut({ key: "K", metaKey: true, ctrlKey: false, shiftKey: false, altKey: false })).toBe(
      true,
    );
    expect(isCommandPaletteShortcut({ key: "k", metaKey: false, ctrlKey: true, shiftKey: false, altKey: false })).toBe(
      true,
    );
    expect(commandForKeyboardEvent({ key: "n", metaKey: true, ctrlKey: false, shiftKey: false, altKey: false })).toBe(
      "new-chat",
    );
    expect(commandForKeyboardEvent({ key: "f", metaKey: true, ctrlKey: false, shiftKey: true, altKey: false })).toBe(
      "search-conversations",
    );
    expect(commandForKeyboardEvent({ key: "k", metaKey: true, ctrlKey: false, shiftKey: true, altKey: false })).toBe(
      null,
    );
  });

  it("wraps keyboard selection while skipping disabled commands", () => {
    const items = commandPaletteItems({
      busy: true,
      contextOpen: true,
      platform: "MacIntel",
      storageAvailable: false,
    });

    expect(nextEnabledCommandIndex(items, -1, 1)).toBe(2);
    expect(nextEnabledCommandIndex(items, 2, -1)).toBe(4);
    expect(nextEnabledCommandIndex(items, 4, 1)).toBe(2);
  });
});
