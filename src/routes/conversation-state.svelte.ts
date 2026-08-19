/** Reactive durable-conversation state kept separate from provider orchestration. */

import { conversationTitle, persistedCompletionMeta } from "$lib/chat";
import type { ModelInfo } from "$lib/inference";
import { nextMessageId, type Message } from "$lib/presentation";
import {
  appendConversationMessage,
  createConversation,
  deleteConversation,
  listConversations,
  loadConversation,
  renameConversation,
  restoreConversation,
  setConversationArchived,
  storageErrorFromUnknown,
  type ConversationSummary,
  type ProviderRunContext,
  type StorageError,
  type StoredMessage,
  type StoredMessageState,
} from "$lib/storage";

/** Owns native conversation navigation, persistence, and presentation reconstruction. */
export class ConversationState {
  conversations = $state<ConversationSummary[]>([]);
  activeConversationId = $state<string | null>(null);
  storageError = $state<StorageError | null>(null);
  isManaging = $state(false);

  /** Loads recent conversations and returns the newest thread's messages. */
  async initialize(): Promise<Message[]> {
    try {
      this.conversations = await listConversations();
      const newest = this.conversations.find((conversation) => conversation.lifecycle === "active");
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
  async persistUserMessage(prompt: string): Promise<ProviderRunContext | null> {
    try {
      if (!this.activeConversationId) {
        const conversation = await createConversation(conversationTitle(prompt));
        this.activeConversationId = conversation.id;
      }
      const stored = await appendConversationMessage({
        conversationId: this.activeConversationId,
        role: "user",
        text: prompt,
        reasoning: null,
        state: "final",
        providerId: null,
        modelId: null,
        providerRunId: null,
      });
      await this.refresh();
      return {
        conversationId: this.activeConversationId,
        requestMessageId: stored.id,
      };
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
    providerRunId: string | null,
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
        providerRunId,
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

  /** Renames one active or archived conversation and refreshes navigation. */
  async rename(conversationId: string, title: string): Promise<boolean> {
    return this.performLifecycleAction(() => renameConversation(conversationId, title));
  }

  /** Moves one conversation into or out of the archive and reports whether the open thread closed. */
  async setArchived(conversationId: string, archived: boolean): Promise<boolean> {
    const wasActive = archived && this.activeConversationId === conversationId;
    const succeeded = await this.performLifecycleAction(() => setConversationArchived(conversationId, archived));
    if (succeeded && wasActive) this.activeConversationId = null;
    return succeeded && wasActive;
  }

  /** Moves one conversation to recoverable trash and reports whether the open thread closed. */
  async delete(conversationId: string): Promise<boolean> {
    const wasActive = this.activeConversationId === conversationId;
    const succeeded = await this.performLifecycleAction(() => deleteConversation(conversationId));
    if (succeeded && wasActive) this.activeConversationId = null;
    return succeeded && wasActive;
  }

  /** Restores one trashed conversation to the recent list. */
  async restore(conversationId: string): Promise<boolean> {
    return this.performLifecycleAction(() => restoreConversation(conversationId));
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

  /** Runs one narrow lifecycle command and keeps failures in the existing storage-error surface. */
  private async performLifecycleAction(action: () => Promise<ConversationSummary>): Promise<boolean> {
    if (this.isManaging) return false;
    this.isManaging = true;
    try {
      await action();
      await this.refresh();
      return true;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return false;
    } finally {
      this.isManaging = false;
    }
  }

  /** Maps one durable record into the richer ephemeral presentation shape. */
  private presentationMessage(message: StoredMessage): Message {
    const model = message.modelId && message.providerId ? `${message.modelId} · ${message.providerId}` : undefined;
    const completedMeta = message.providerRun
      ? persistedCompletionMeta(
          message.providerRun.startedAtMs,
          message.providerRun.completedAtMs,
          message.providerRun.usage,
        )
      : undefined;
    return {
      id: nextMessageId(),
      role: message.role,
      content: message.text,
      reasoning: message.reasoning ?? undefined,
      model,
      meta: message.state === "cancelled" ? "Stopped · saved partial response" : completedMeta,
      error: message.state === "failed",
    };
  }
}
