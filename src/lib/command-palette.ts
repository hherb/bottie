/** Pure command registry, filtering, and keyboard matching for the in-app command palette. */

export type CommandId = "new-chat" | "search-conversations" | "focus-navigation" | "toggle-context" | "open-settings";

/** One rendered command whose execution remains owned by the page shell. */
export type CommandPaletteItem = {
  id: CommandId;
  label: string;
  description: string;
  shortcut: string;
  keywords: string[];
  disabledReason?: string;
};

/** State used to derive dynamic command labels and existing availability gates. */
export type CommandPaletteOptions = {
  busy: boolean;
  contextOpen: boolean;
  platform: string;
  storageAvailable: boolean;
};

/** Minimal keyboard shape accepted by shortcut helpers and unit tests. */
export type CommandKeyboardEvent = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">;

type Shortcut = {
  id: CommandId;
  key: string;
  shift: boolean;
};

const BUSY_NEW_CHAT_REASON = "Finish the current response before starting a new conversation.";
const STORAGE_SEARCH_REASON = "Conversation search is unavailable while local storage is unavailable.";
const MAC_PLATFORM_PATTERN = /Mac|iPhone|iPad|iPod/i;
const MODIFIER_SHORTCUTS: Shortcut[] = [
  { id: "new-chat", key: "n", shift: false },
  { id: "search-conversations", key: "f", shift: true },
  { id: "focus-navigation", key: "b", shift: true },
  { id: "toggle-context", key: "c", shift: true },
  { id: "open-settings", key: ",", shift: false },
];

/** Returns the visible primary modifier for the active desktop platform. */
export function commandModifierLabel(platform: string): "⌘" | "Ctrl" {
  return MAC_PLATFORM_PATTERN.test(platform) ? "⌘" : "Ctrl";
}

/** Returns the platform-appropriate visible shortcut for opening the palette. */
export function commandPaletteShortcutLabel(platform: string): string {
  return `${commandModifierLabel(platform)} K`;
}

/** Builds the complete bounded registry without binding commands to side effects. */
export function commandPaletteItems(options: CommandPaletteOptions): CommandPaletteItem[] {
  const modifier = commandModifierLabel(options.platform);
  return [
    {
      id: "new-chat",
      label: "New conversation",
      description: "Start a blank local conversation.",
      shortcut: `${modifier} N`,
      keywords: ["chat", "compose", "blank"],
      disabledReason: options.busy ? BUSY_NEW_CHAT_REASON : undefined,
    },
    {
      id: "search-conversations",
      label: "Search conversations",
      description: "Find titles and visible message text in local history.",
      shortcut: `${modifier} ⇧ F`,
      keywords: ["find", "history", "messages"],
      disabledReason: options.storageAvailable ? undefined : STORAGE_SEARCH_REASON,
    },
    {
      id: "focus-navigation",
      label: "Focus conversation navigation",
      description: "Move keyboard focus to the conversation sidebar.",
      shortcut: `${modifier} ⇧ B`,
      keywords: ["sidebar", "history", "conversations"],
    },
    {
      id: "toggle-context",
      label: options.contextOpen ? "Hide context panel" : "Show context panel",
      description: "Toggle attachments, citations, and route context.",
      shortcut: `${modifier} ⇧ C`,
      keywords: ["panel", "attachments", "citations", "sources"],
    },
    {
      id: "open-settings",
      label: "Open Settings",
      description: "Review providers, privacy, Localmail, and diagnostics.",
      shortcut: `${modifier} ,`,
      keywords: ["preferences", "providers", "privacy", "diagnostics"],
    },
  ];
}

/** Filters commands locally using every non-empty query token. */
export function filterCommandPaletteItems(items: CommandPaletteItem[], query: string): CommandPaletteItem[] {
  const tokens = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
  if (tokens.length === 0) return items;
  return items.filter((item) => {
    const searchable = [item.label, item.description, ...item.keywords].join(" ").toLocaleLowerCase();
    return tokens.every((token) => searchable.includes(token));
  });
}

/** Selects the next enabled command index with wrapping keyboard navigation. */
export function nextEnabledCommandIndex(items: CommandPaletteItem[], current: number, direction: -1 | 1): number {
  if (items.length === 0) return -1;
  for (let offset = 1; offset <= items.length; offset += 1) {
    const index = (current + direction * offset + items.length) % items.length;
    if (!items[index].disabledReason) return index;
  }
  return -1;
}

/** Recognizes the exact Command/Ctrl+K palette shortcut. */
export function isCommandPaletteShortcut(event: CommandKeyboardEvent): boolean {
  return matchesShortcut(event, "k", false);
}

/** Resolves an exact direct shortcut to its command without executing it. */
export function commandForKeyboardEvent(event: CommandKeyboardEvent): CommandId | null {
  return MODIFIER_SHORTCUTS.find((shortcut) => matchesShortcut(event, shortcut.key, shortcut.shift))?.id ?? null;
}

/** Matches one primary-modifier shortcut while rejecting ambiguous extra modifiers. */
function matchesShortcut(event: CommandKeyboardEvent, key: string, shift: boolean): boolean {
  const hasOnePrimaryModifier = event.metaKey !== event.ctrlKey;
  return (
    hasOnePrimaryModifier &&
    !event.altKey &&
    event.shiftKey === shift &&
    event.key.toLocaleLowerCase() === key.toLocaleLowerCase()
  );
}
