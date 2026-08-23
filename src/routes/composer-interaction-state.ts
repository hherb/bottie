/** DOM-only interaction helpers kept separate from durable conversation and provider state. */

import { tick } from "svelte";

import { MAX_COMPOSER_HEIGHT_PX } from "$lib/presentation";

const NEXT_EVENT_LOOP_TICK_MS = 0;

/** Owns registered composer/scroll elements and their bounded presentation behavior. */
export class ComposerInteractionState {
  private messageScroll?: HTMLDivElement;
  private composer?: HTMLTextAreaElement;

  /** Registers the conversation's scroll container after its component mounts. */
  setMessageScroll(element: HTMLDivElement): void {
    this.messageScroll = element;
  }

  /** Registers the composer textarea after its component mounts. */
  setComposer(element: HTMLTextAreaElement): void {
    this.composer = element;
  }

  /** Scrolls the conversation to its newest content after pending DOM updates. */
  async scrollToBottom(behavior: ScrollBehavior = "smooth"): Promise<void> {
    await tick();
    this.messageScroll?.scrollTo({ top: this.messageScroll.scrollHeight, behavior });
  }

  /** Resizes the composer up to its named maximum presentation height. */
  resizeComposer(): void {
    if (!this.composer) return;
    this.composer.style.height = "0";
    this.composer.style.height = `${Math.min(this.composer.scrollHeight, MAX_COMPOSER_HEIGHT_PX)}px`;
  }

  /** Sends on unmodified Enter while preserving Shift+Enter for newlines. */
  handleKeydown(event: KeyboardEvent, send: () => void): void {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }

  /** Focuses the composer after the blank-chat state reaches the DOM. */
  focusAfterUpdate(): void {
    setTimeout(() => this.composer?.focus(), NEXT_EVENT_LOOP_TICK_MS);
  }
}
