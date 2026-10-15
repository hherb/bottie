/** Reactive controller for `@kastellan:` requests: dispatch without holding the composer, reload on completion. */

import { providerErrorFromUnknown } from "$lib/inference";
import {
  KASTELLAN_SENDING_META,
  kastellanWaitingMeta,
  listenForKastellanTasks,
  startKastellanTask,
  type KastellanTaskFinished,
} from "$lib/kastellan";
import { nextMessageId, type Message } from "$lib/presentation";
import type { ProviderRunContext } from "$lib/storage";

/** What the controller needs from the page that owns the conversation timeline. */
export type KastellanPage = {
  /** The live, reactive timeline of the open conversation. */
  messages(): Message[];
  /** Replaces the timeline after a durable reload. */
  replaceMessages(messages: Message[]): void;
  /** The open conversation, if any. */
  activeConversationId(): string | null;
  /** Whether another run or a write owns the timeline right now; that run reloads it when it finishes. */
  isBusy(): boolean;
  /** Reloads the open conversation from storage. */
  reload(): Promise<Message[] | null>;
  /** Scrolls the conversation to its latest message. */
  scrollToBottom(behavior?: ScrollBehavior): Promise<void>;
};

/** Owns the completion listener and the optimistic waiting response for Kastellan requests. */
export class KastellanState {
  private stopUpdates?: () => void;

  constructor(private readonly page: KastellanPage) {}

  /** Starts listening for background completions for the page's lifetime. */
  async listen(): Promise<void> {
    this.stopUpdates = await listenForKastellanTasks((event) => void this.applyCompletion(event));
  }

  /** Releases the native listener. */
  dispose(): void {
    this.stopUpdates?.();
    this.stopUpdates = undefined;
  }

  /** Shows a waiting response and hands one already-persisted request to Kastellan. */
  async dispatch(runContext: ProviderRunContext, prompt: string): Promise<void> {
    const replyId = nextMessageId();
    this.page.messages().push({
      id: replyId,
      role: "assistant",
      content: "",
      model: "Kastellan",
      meta: KASTELLAN_SENDING_META,
    });
    await this.page.scrollToBottom();
    const reply = () => this.page.messages().find((message) => message.id === replyId);
    try {
      const run = await startKastellanTask(runContext, prompt);
      const waiting = reply();
      if (waiting && !waiting.content) {
        waiting.model = `Kastellan · ${run.host}`;
        waiting.meta = kastellanWaitingMeta(run.host);
      }
    } catch (error) {
      const failed = reply();
      if (failed) {
        failed.content = providerErrorFromUnknown(error).message;
        failed.error = true;
        failed.meta = undefined;
      }
    }
  }

  /** Reloads the open conversation when a background reply has been saved into it. */
  async applyCompletion(event: KastellanTaskFinished): Promise<void> {
    // Another conversation reads the reply from storage when opened; a busy page reloads when its run finishes.
    const applies = () => event.conversationId === this.page.activeConversationId() && !this.page.isBusy();
    if (!applies()) return;
    const messages = await this.page.reload();
    // The user may have switched conversation or started a run while the reload was in flight.
    if (!messages || !applies()) return;
    this.page.replaceMessages(messages);
    await this.page.scrollToBottom("auto");
  }
}
