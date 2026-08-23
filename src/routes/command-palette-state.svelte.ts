/** Reactive shell state for the bounded local command palette. */

import {
  commandPaletteItems,
  commandPaletteShortcutLabel,
  type CommandPaletteItem,
  type CommandPaletteOptions,
} from "$lib/command-palette";

type CommandShellOptions = Omit<CommandPaletteOptions, "platform">;

/** Owns palette visibility and platform presentation without binding commands to side effects. */
export class CommandPaletteState {
  platform = $state("");
  open = $state(false);

  /** Returns the current bounded registry with page-owned availability supplied by the caller. */
  items(options: CommandShellOptions): CommandPaletteItem[] {
    return commandPaletteItems({ ...options, platform: this.platform });
  }

  /** Returns the visible platform-appropriate palette shortcut. */
  shortcutLabel(): string {
    return commandPaletteShortcutLabel(this.platform);
  }
}
