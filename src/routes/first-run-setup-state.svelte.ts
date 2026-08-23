/** Reactive lifecycle for the native first-run provider and privacy gate. */

import { isTauri } from "@tauri-apps/api/core";

import { completeFirstRunSetup, providerErrorFromUnknown, type ProviderSettings } from "$lib/inference";

/** Owns setup visibility, persistence progress, and path-redacted failure feedback. */
export class FirstRunSetupState {
  isSaving = $state(false);
  error = $state("");

  /** Whether this native installation still requires its first provider/privacy acknowledgement. */
  requiresSetup(settings: ProviderSettings): boolean {
    return isTauri() && !settings.setupCompleted;
  }

  /** Completes setup only after discovery produced an explicit provider/model route. */
  async complete(canComplete: boolean, onsaved: (settings: ProviderSettings) => void): Promise<void> {
    if (!canComplete || this.isSaving) return;
    this.isSaving = true;
    this.error = "";
    try {
      onsaved(await completeFirstRunSetup());
    } catch (error) {
      this.error = providerErrorFromUnknown(error).message;
    } finally {
      this.isSaving = false;
    }
  }
}
