/** Reactive frontend-only state for Bottie's native Web tools and path-free provenance. */

import type { Message } from "$lib/presentation";
import { webSourcesForMessages, type WebSource } from "$lib/web-provenance";

/** Owns explicit Web-tool availability and visible source-card dismissals. */
export class WebToolState {
  enabled = $state(false);
  private dismissedSourceIds = $state<string[]>([]);

  /** Returns successful native Web sources that have not been dismissed in this frontend session. */
  sources(messages: Message[]): WebSource[] {
    return webSourcesForMessages(messages, new Set(this.dismissedSourceIds));
  }

  /** Disables web search when provider capability or selection changes. */
  disable(): void {
    this.enabled = false;
  }

  /** Applies one already capability-checked remembered preference. */
  restore(enabled: boolean): void {
    this.enabled = enabled;
  }

  /** Toggles native web search only for an idle, mapped, capable route. */
  toggle(available: boolean, isGenerating: boolean): void {
    if (available && !isGenerating) this.enabled = !this.enabled;
  }

  /** Hides one derived source card without mutating the append-only native tool audit. */
  dismiss(sourceId: string): void {
    if (!this.dismissedSourceIds.includes(sourceId)) {
      this.dismissedSourceIds = [...this.dismissedSourceIds, sourceId];
    }
  }
}
