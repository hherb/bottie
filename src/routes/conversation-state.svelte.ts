/** Reactive durable-conversation state kept separate from provider orchestration. */

import {
  conversationTitle,
  nextResponseRating,
  persistedCompletionMeta,
  persistedMessagePresentation,
} from "$lib/chat";
import { nextMessageId, type Message } from "$lib/presentation";
import {
  appendConversationMessage,
  backupConversationStore,
  branchConversationMessage,
  clearLastOpenConversation,
  createConversation,
  deleteConversation,
  exportConversationMarkdown,
  listConversations,
  loadConversation,
  loadLastOpenConversation,
  rateConversationResponse,
  renameConversation,
  restoreConversation,
  searchConversations,
  selectConversationBranch,
  setConversationArchived,
  storageErrorFromUnknown,
  type ConversationBranch,
  type ConversationSearchResult,
  type ConversationSummary,
  type ProviderRunContext,
  type ResponseRating,
  type StorageError,
  type StoredMessage,
  type StoredConversation,
} from "$lib/storage";

/** Presentation and native request identity produced by one branch fork. */
export type BranchedGeneration = {
  messages: Message[];
  context: ProviderRunContext;
};

/** Owns native conversation navigation, persistence, and presentation reconstruction. */
export class ConversationState {
  conversations = $state<ConversationSummary[]>([]);
  activeConversationId = $state<string | null>(null);
  branches = $state<ConversationBranch[]>([]);
  currentBranchId = $state<string | null>(null);
  storageError = $state<StorageError | null>(null);
  isManaging = $state(false);
  searchQuery = $state("");
  searchResults = $state<ConversationSearchResult[]>([]);
  isSearching = $state(false);
  isExporting = $state(false);
  isBackingUp = $state(false);
  exportFeedback = $state<string | null>(null);
  exportFailed = $state(false);
  backupFeedback = $state<string | null>(null);
  backupFailed = $state(false);

  private searchSequence = 0;

  /** Loads navigation and returns the exact durable profile selection. */
  async initialize(): Promise<Message[]> {
    try {
      const [conversations, selected] = await Promise.all([listConversations(), loadLastOpenConversation()]);
      this.conversations = conversations;
      if (!selected) return [];
      return this.applyConversation(selected);
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return [];
    }
  }

  /** Opens one persisted conversation and returns its presentation messages. */
  async open(conversationId: string): Promise<Message[] | null> {
    try {
      const conversation = await loadConversation(conversationId);
      this.storageError = null;
      return this.applyConversation(conversation);
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    }
  }

  /** Opens the exact preserved branch returned by native conversation search. */
  async openSearchResult(result: ConversationSearchResult): Promise<Message[] | null> {
    if (this.isManaging) return null;
    this.isManaging = true;
    try {
      let conversation = await loadConversation(result.conversationId);
      if (conversation.currentBranchId !== result.branchId) {
        conversation = await selectConversationBranch(result.conversationId, result.branchId);
      }
      this.storageError = null;
      return this.applyConversation(conversation);
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    } finally {
      this.isManaging = false;
    }
  }

  /** Runs one bounded native search while discarding results from superseded queries. */
  async search(query: string): Promise<void> {
    this.searchQuery = query;
    const sequence = ++this.searchSequence;
    if (!query.trim()) {
      this.searchResults = [];
      this.isSearching = false;
      return;
    }
    this.isSearching = true;
    try {
      const results = await searchConversations(query);
      if (sequence !== this.searchSequence) return;
      this.searchResults = results;
      this.storageError = null;
    } catch (error) {
      if (sequence !== this.searchSequence) return;
      this.searchResults = [];
      this.storageError = storageErrorFromUnknown(error);
    } finally {
      if (sequence === this.searchSequence) this.isSearching = false;
    }
  }

  /** Forks a visible user request and returns the new selected lineage plus generation context. */
  async branchFromUserMessage(messageId: string, text: string): Promise<BranchedGeneration | null> {
    if (!this.activeConversationId || this.isManaging) return null;
    this.isManaging = true;
    try {
      const result = await branchConversationMessage(this.activeConversationId, messageId, text);
      this.storageError = null;
      await this.refresh();
      return {
        messages: this.applyConversation(result.conversation),
        context: {
          conversationId: result.conversation.id,
          requestMessageId: result.requestMessageId,
        },
      };
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    } finally {
      this.isManaging = false;
    }
  }

  /** Selects one existing branch and returns its reconstructed presentation lineage. */
  async selectBranch(branchId: string): Promise<Message[] | null> {
    if (!this.activeConversationId || this.isManaging) return null;
    this.isManaging = true;
    try {
      const conversation = await selectConversationBranch(this.activeConversationId, branchId);
      this.storageError = null;
      return this.applyConversation(conversation);
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    } finally {
      this.isManaging = false;
    }
  }

  /** Toggles one durable assistant-response rating through the native storage boundary. */
  async rateResponse(messages: Message[], responseId: number, selected: ResponseRating): Promise<void> {
    const response = messages.find((message) => message.id === responseId && message.role === "assistant");
    if (!this.activeConversationId || !response?.storageId || this.isManaging) return;
    this.isManaging = true;
    try {
      const rating = nextResponseRating(response.rating ?? null, selected);
      const stored = await rateConversationResponse(this.activeConversationId, response.storageId, rating);
      response.rating = stored ?? undefined;
      this.storageError = null;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
    } finally {
      this.isManaging = false;
    }
  }

  /** Opens the native Save dialog for the selected lineage and reports only its leaf filename. */
  async exportMarkdown(): Promise<void> {
    if (!this.activeConversationId || this.isManaging) return;
    this.isManaging = true;
    this.isExporting = true;
    this.exportFeedback = null;
    this.exportFailed = false;
    this.backupFeedback = null;
    this.backupFailed = false;
    try {
      const outcome = await exportConversationMarkdown(this.activeConversationId);
      if (outcome.status === "saved") this.exportFeedback = `Saved ${outcome.fileName ?? "Markdown export"}`;
      this.storageError = null;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      this.exportFeedback = this.storageError.message;
      this.exportFailed = true;
    } finally {
      this.isExporting = false;
      this.isManaging = false;
    }
  }

  /** Opens the native Save dialog for a complete SQLite snapshot and reports only its leaf filename. */
  async backup(): Promise<void> {
    if (this.isManaging) return;
    this.isManaging = true;
    this.isBackingUp = true;
    this.backupFeedback = null;
    this.backupFailed = false;
    this.exportFeedback = null;
    this.exportFailed = false;
    try {
      const outcome = await backupConversationStore();
      if (outcome.status === "saved") this.backupFeedback = `Backed up ${outcome.fileName ?? "local data"}`;
      this.storageError = null;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      this.backupFeedback = this.storageError.message;
      this.backupFailed = true;
    } finally {
      this.isBackingUp = false;
      this.isManaging = false;
    }
  }

  /** Creates a conversation when needed and durably appends the submitted prompt. */
  async persistUserMessage(prompt: string): Promise<ProviderRunContext | null> {
    try {
      let conversationId = this.activeConversationId;
      if (!conversationId) {
        const conversation = await createConversation(conversationTitle(prompt));
        this.applyConversation(conversation);
        conversationId = conversation.id;
      }
      const stored = await appendConversationMessage(conversationId, prompt);
      await this.refresh();
      return {
        conversationId,
        requestMessageId: stored.id,
      };
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    }
  }

  /** Reloads the completed native response identity and refreshes navigation after generation. */
  async refreshAfterGeneration(): Promise<Message[] | null> {
    const conversationId = this.activeConversationId;
    if (!conversationId) {
      await this.refresh();
      return null;
    }
    try {
      const conversation = await loadConversation(conversationId);
      const messages = this.applyConversation(conversation);
      await this.refresh();
      return messages;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
      return null;
    }
  }

  /** Clears the active identity and persists the intentional blank new-chat view. */
  async startNew(): Promise<void> {
    this.activeConversationId = null;
    this.branches = [];
    this.currentBranchId = null;
    this.exportFeedback = null;
    this.exportFailed = false;
    try {
      await clearLastOpenConversation();
      this.storageError = null;
    } catch (error) {
      this.storageError = storageErrorFromUnknown(error);
    }
  }

  /** Applies one reconstructed native conversation to durable navigation state. */
  private applyConversation(conversation: StoredConversation): Message[] {
    this.exportFeedback = null;
    this.exportFailed = false;
    this.activeConversationId = conversation.id;
    this.branches = conversation.branches;
    this.currentBranchId = conversation.currentBranchId;
    return conversation.messages.map((message) => this.presentationMessage(message));
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
      if (this.searchQuery.trim()) await this.search(this.searchQuery);
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
      storageId: message.id,
      role: message.role,
      content: message.text || presentation.fallbackText || "",
      reasoning: message.reasoning ?? undefined,
      model,
      meta: presentation.meta ?? completedMeta,
      error: presentation.error,
      retryable: presentation.retryable,
      rating: message.rating ?? undefined,
    };
  }
}
