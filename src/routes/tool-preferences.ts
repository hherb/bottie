/** Persistence and effective state for Bottie's three non-secret native-tool preferences. */

import { isTauri } from "@tauri-apps/api/core";

import { updateProviderSettings, type ProviderSettings } from "$lib/inference";
import { EmailToolState } from "./email-tool-state.svelte";
import { MemoryContextState } from "./memory-context-state.svelte";
import { WebToolState } from "./web-tool-state.svelte";

/** Persisted setting field controlled by one composer tool toggle. */
export type ToolPreferenceField = "memoryEnabled" | "webEnabled" | "emailEnabled";
/** Current native readiness for each remembered composer tool. */
export type ToolAvailability = Record<"memory" | "web" | "email", boolean>;

/** Restores one remembered tool preference without bypassing current native-route readiness. */
export function restoredToolPreference(preferred: boolean, available: boolean): boolean {
  return preferred && available;
}

/** Saves one non-secret preference through the existing native settings boundary. */
export class ToolPreferenceState {
  memory = new MemoryContextState();
  web = new WebToolState();
  email = new EmailToolState();
  private saving = false;

  /** Restores effective states from remembered choices plus current native readiness. */
  restore(settings: ProviderSettings, availability: ToolAvailability): void {
    this.memory.restore(restoredToolPreference(settings.memoryEnabled, availability.memory));
    this.web.restore(restoredToolPreference(settings.webEnabled, availability.web));
    this.email.restore(restoredToolPreference(settings.emailEnabled, availability.email));
  }

  /** Toggles one effective state and persists its non-secret preference. */
  async toggle(
    tool: keyof ToolAvailability,
    settings: ProviderSettings,
    availability: ToolAvailability,
    isGenerating: boolean,
  ): Promise<ProviderSettings> {
    if (this.saving) return settings;
    const state = this[tool];
    state.toggle(availability[tool], isGenerating);
    const field = `${tool}Enabled` as ToolPreferenceField;
    if (!isTauri() || settings[field] === state.enabled) return settings;
    this.saving = true;
    try {
      return await updateProviderSettings({ ...settings, [field]: state.enabled });
    } catch (error) {
      this.restore(settings, availability);
      console.warn("Could not remember native tool preferences", error);
      return settings;
    } finally {
      this.saving = false;
    }
  }
}
