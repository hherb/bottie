/** Reactive frontend-only visibility state for path-free memory provenance. */

import { memoryCitationsForMessages, type MemoryCitation } from "$lib/memory-provenance";
import type { Message } from "$lib/presentation";

/** Owns explicit memory-tool enablement and session-local citation dismissals. */
export class MemoryContextState {
  enabled = $state(false);
  private dismissedCitationIds = $state<string[]>([]);

  /** Returns successful native citations that have not been dismissed in this frontend session. */
  citations(messages: Message[]): MemoryCitation[] {
    return memoryCitationsForMessages(messages, new Set(this.dismissedCitationIds));
  }

  /** Disables native memory tools when provider capability or selection changes. */
  disable(): void {
    this.enabled = false;
  }

  /** Toggles native memory tools only for an idle, mapped, capable route. */
  toggle(available: boolean, isGenerating: boolean): void {
    if (available && !isGenerating) this.enabled = !this.enabled;
  }

  /** Hides one derived card without mutating the append-only native tool audit. */
  dismiss(citationId: string): void {
    if (!this.dismissedCitationIds.includes(citationId)) {
      this.dismissedCitationIds = [...this.dismissedCitationIds, citationId];
    }
  }
}
