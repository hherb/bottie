/** Session-only frontend state for Bottie's explicitly configured Localmail tools. */

import { getLocalmailConnectionStatus, localmailToolsConfigured } from "$lib/localmail";

/** Owns off-by-default Email enablement and secret-free connector readiness. */
export class EmailToolState {
  enabled = $state(false);
  configured = $state(false);

  /** Refreshes whether pinned trust and a vault credential are both configured. */
  async refresh(): Promise<void> {
    try {
      const status = await getLocalmailConnectionStatus();
      this.configured = localmailToolsConfigured(status);
    } catch {
      this.configured = false;
    }
    if (!this.configured) this.disable();
  }

  /** Disables Email whenever routing, capability, or connector readiness no longer permits it. */
  disable(): void {
    this.enabled = false;
  }

  /** Toggles Email only for an idle, configured, capable Ollama route. */
  toggle(available: boolean, isGenerating: boolean): void {
    if (available && !isGenerating) this.enabled = !this.enabled;
  }
}
