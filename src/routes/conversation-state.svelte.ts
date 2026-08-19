/** Reactive durable-conversation state kept separate from provider orchestration. */

import { conversationTitle } from "$lib/chat";
import type { ModelInfo } from "$lib/inference";
import { nextMessageId, type Message } from "$lib/presentation";
import {
  appendConversationMessage,
  createConversation,
  listConversations,
  loadConversation,
  storageErrorFromUnknown,
  type ConversationSummary,
  type StorageError,
  type StoredMessage,
  type StoredMessageState,
} from "$lib/storage";

/** Owns native conversation navigation, persistence, and presentation reconstruction. */
export class ConversationState {
  conversations = $state<ConversationSummary[]>([]);
  activeConversationId = $state<string | null>(null);
  storageError = $state<StorageError | null>(null);

  /** Loads recent conversations and returns the newest thread's messages. */
  async initialize(): Promise<Message[]> {
    try {
      this.conversations = await listConversations();
      const newest = this.conversations[0];
      if (!newest) return [];
      return (await this.open(newest.id)) ?? [];
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return [];
    }
  }

  /** Opens one persisted conversation and returns its presentation messages. */
  async open(conversationId: string): Promise<Message[] | null> {
    try {
      const conversation = await loadConversation(conversationId);
      this.activeConversationId = conversation.id;
      this.storageError = null;
      return conversation.messages.map((message) => this.presentationMessage(message));
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    }
  }

  /** Creates a conversation when needed and durably appends the submitted prompt. */
  async persistUserMessage(prompt: string): Promise<string | null> {
    try {
      if (!this.activeConversationId) {
        const conversation = await createConversation(conversationTitle(prompt));
        this.activeConversationId = conversation.id;
      }
      await appendConversationMessage({
        conversationId: this.activeConversationId,
        role: "user",
        text: prompt,
        reasoning: null,
        state: "final",
        providerId: null,
        modelId: null,
      });
      await this.refresh();
      return this.activeConversationId;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    }
  }

  /** Persists one terminal assistant response and refreshes conversation ordering. */
  async persistAssistantMessage(
    conversationId: string,
    message: Message | undefined,
    state: StoredMessageState,
    model: ModelInfo,
  ): Promise<void> {
    if (!message) return;
    try {
      await appendConversationMessage({
        conversationId,
        role: "assistant",
        text: message.content,
        reasoning: message.reasoning ?? null,
        state,
        providerId: model.providerId,
        modelId: model.modelId,
      });
      await this.refresh();
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
    }
  }

  /** Clears the active identity so the next prompt starts a new durable thread. */
  startNew(): void {
    this.activeConversationId = null;
  }

  /** Refreshes durable navigation without changing the open conversation. */
  private async refresh(): Promise<void> {
    try {
      this.conversations = await listConversations();
      this.storageError = null;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
    }
  }

  /** Maps one durable record into the richer ephemeral presentation shape. */
  private presentationMessage(message: StoredMessage): Message {
    const model = message.modelId && message.providerId ? `${message.modelId} · ${message.providerId}` : undefined;
    return {
      id: nextMessageId(),
      role: message.role,
      content: message.text,
      reasoning: message.reasoning ?? undefined,
      model,
      meta: message.state === "cancelled" ? "Stopped · saved partial response" : undefined,
      error: message.state === "failed",
    };
  }
}
