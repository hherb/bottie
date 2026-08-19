/** Reactive durable-conversation state kept separate from provider orchestration. */

import { conversationTitle, persistedCompletionMeta, persistedMessagePresentation } from "$lib/chat";
import { nextMessageId, type Message } from "$lib/presentation";
import {
  appendConversationMessage,
  clearLastOpenConversation,
  createConversation,
  deleteConversation,
  listConversations,
  loadConversation,
  loadLastOpenConversation,
  renameConversation,
  restoreConversation,
  setConversationArchived,
  storageErrorFromUnknown,
  type ConversationSummary,
  type ProviderRunContext,
  type StorageError,
  type StoredMessage,
} from "$lib/storage";

/** Owns native conversation navigation, persistence, and presentation reconstruction. */
export class ConversationState {
  conversations = $state<ConversationSummary[]>([]);
  activeConversationId = $state<string | null>(null);
  storageError = $state<StorageError | null>(null);
  isManaging = $state(false);

  /** Loads navigation and returns the exact durable profile selection. */
  async initialize(): Promise<Message[]> {
    try {
      const [conversations, selected] = await Promise.all([listConversations(), loadLastOpenConversation()]);
      this.conversations = conversations;
      if (!selected) return [];
      this.activeConversationId = selected.id;
      return selected.messages.map((message) => this.presentationMessage(message));
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
      const stored = await appendConversationMessage(this.activeConversationId, prompt);
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

  /** Refreshes durable navigation after native generation reaches a terminal state. */
  async refreshAfterGeneration(): Promise<void> {
    await this.refresh();
  }

  /** Clears the active identity and persists the intentional blank new-chat view. */
  async startNew(): Promise<void> {
    this.activeConversationId = null;
    try {
      await clearLastOpenConversation();
      this.storageError = null;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
    }
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
    const hasContent = message.text.length > 0 || Boolean(message.reasoning);
    const presentation = persistedMessagePresentation(
      message.state,
      message.providerRun?.errorCode ?? null,
      hasContent,
    );
    return {
      id: nextMessageId(),
      role: message.role,
      content: message.text || presentation.fallbackText || "",
      reasoning: message.reasoning ?? undefined,
      model,
      meta: presentation.meta ?? completedMeta,
      error: presentation.error,
    };
  }
}
