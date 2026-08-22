/** Reactive session-only enablement for Bottie's native web-search tool. */

/** Owns explicit web-search availability for the next compatible request. */
export class WebToolState {
  enabled = $state(false);

  /** Disables web search when provider capability or selection changes. */
  disable(): void {
    this.enabled = false;
  }

  /** Toggles native web search only for an idle, mapped, capable route. */
  toggle(available: boolean, isGenerating: boolean): void {
    if (available && !isGenerating) this.enabled = !this.enabled;
  }
}
