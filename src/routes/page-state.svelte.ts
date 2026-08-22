/** Reactive presentation state and actions for the Bottie conversation shell. */

import { invoke, isTauri } from "@tauri-apps/api/core";
import { tick } from "svelte";

import { applyAttachmentProcessingUpdateToMessages } from "$lib/attachment";
import {
  chatTurnsForMessages,
  completionMeta,
  draftImageDeliveryBlocker,
  filterUsableModels,
  isCloudProvider,
  modelKey,
  requestMessageForResponse,
  resolveModelSelection,
  toggleReasoningEffort,
} from "$lib/chat";
import {
  cancelChat,
  discoverModels,
  getProviderSettings,
  providerErrorFromUnknown,
  rememberProviderSelection,
  startChat,
  type ModelInfo,
  type ProviderId,
  type ProviderError,
  type ProviderSettings,
  type ReasoningEffort,
  type StreamEvent,
  type Usage,
} from "$lib/inference";
import {
  DEFAULT_PROVIDER_SETTINGS,
  INITIAL_MESSAGES,
  MAX_COMPOSER_HEIGHT_PX,
  nextMessageId,
  type Message,
  type ProviderStatus,
  type RuntimeInfo,
} from "$lib/presentation";

import { ConversationState } from "./conversation-state.svelte";
import { AttachmentState } from "./attachment-state.svelte";
import { RecoveryState } from "./recovery-state.svelte";
import { memoryToolsAvailable } from "./page-presentation";

const IDLE_STAGE = -1;
const STARTING_STAGE = 0;
const STREAMING_STAGE = 1;
const NEXT_EVENT_LOOP_TICK_MS = 0;
/** Owns the reactive state and imperative actions shared by the page's presentation components. */
export class PageState {
  messages = $state<Message[]>(isTauri() ? [] : INITIAL_MESSAGES.map((message) => ({ ...message })));
  prompt = $state("");
  isGenerating = $state(false);
  isPersistingMessage = $state(false);
  activeStage = $state(IDLE_STAGE);
  activeRunId = $state<string | null>(null);
  activeAssistantId = $state<number | null>(null);
  showContext = $state(true);
  showSidebar = $state(false);
  showSettings = $state(false);
  runtime = $state<RuntimeInfo>({ name: "bottie", version: "preview", storage: "local" });
  models = $state<ModelInfo[]>([]);
  selectedProviderId = $state<ProviderId | "">("");
  selectedModelKey = $state("");
  providerStatus = $state<ProviderStatus>(isTauri() ? "checking" : "browser");
  providerError = $state<ProviderError | null>(null);
  currentUsage = $state<Usage | null>(null);
  reasoningEffort = $state<ReasoningEffort>("off");
  memoryEnabled = $state(false);
  providerSettings = $state<ProviderSettings>({ ...DEFAULT_PROVIDER_SETTINGS });
  recovery = new RecoveryState();
  history = new ConversationState();
  attachment = new AttachmentState();

  private generationRun = 0;
  private cancellationRequested = false;
  private messageScroll?: HTMLDivElement;
  private composer?: HTMLTextAreaElement;

  /** Currently selected provider-qualified model, when discovery has produced one. */
  get selectedModel(): ModelInfo | undefined {
    return this.models.find((model) => modelKey(model) === this.selectedModelKey);
  }
  /** Whether the current provider and model selection can accept a message. */
  get canSend(): boolean {
    return this.providerStatus === "available" && Boolean(this.selectedModel) && !this.isPersistingMessage;
  }
  /** Whether every current image has a ready derivative and an explicitly vision-capable route. */
  get attachmentsCanSubmit(): boolean {
    return this.attachment.canSubmit(this.selectedModel, this.history.conversationAttachments);
  }
  /** Whether branch-independent conversation images can be applied to a regenerated request. */
  get conversationAttachmentsCanSubmit(): boolean {
    return draftImageDeliveryBlocker(this.history.conversationAttachments, this.selectedModel) === null;
  }
  /** Whether the selected route keeps prompt traffic on this device. */
  get isLocalRoute(): boolean {
    return !isCloudProvider(this.selectedProviderId);
  }
  /** Whether the selected mapped provider/model pair can accept Bottie's native memory tools. */
  get memoryAvailable(): boolean {
    return memoryToolsAvailable(this.selectedModel);
  }
  /** Loads native runtime information, persisted settings, and available models. */
  async initialize(): Promise<void> {
    if (!isTauri()) {
      this.providerError = {
        code: "unavailable",
        message: "Browser preview is disconnected. Open the native Tauri app to use inference providers.",
        retryable: false,
      };
      return;
    }
    await this.attachment.listenForProcessingUpdates((update) => {
      this.messages = applyAttachmentProcessingUpdateToMessages(this.messages, update);
      this.history.applyAttachmentProcessingUpdate(update);
    });
    try {
      this.runtime = await invoke<RuntimeInfo>("app_info");
    } catch (error) {
      console.warn("Could not read the native runtime information", error);
    }
    try {
      this.providerSettings = await getProviderSettings();
      this.selectedProviderId = this.providerSettings.lastProviderId ?? "";
    } catch (error) {
      console.warn("Could not read provider settings", error);
    }
    if (!(await this.recovery.initialize())) {
      this.providerStatus = "offline";
      return;
    }
    const [messages] = await Promise.all([this.history.initialize(), this.refreshModels()]);
    this.messages = messages;
  }

  /** Releases native event listeners when the page is unmounted. */
  dispose(): void {
    this.attachment.dispose();
  }

  /** Opens one persisted conversation from the sidebar. */
  async openConversation(conversationId: string): Promise<void> {
    if (this.isGenerating) return;
    const messages = await this.history.open(conversationId);
    if (messages) {
      this.messages = messages;
      this.showSidebar = false;
      await this.scrollToBottom("auto");
    }
  }

  /** Opens the preserved branch selected from native conversation-search results. */
  async openSearchResult(result: import("$lib/storage").ConversationSearchResult): Promise<void> {
    if (this.isGenerating) return;
    const messages = await this.history.openSearchResult(result);
    if (messages) {
      this.messages = messages;
      this.showSidebar = false;
      await this.scrollToBottom("auto");
    }
  }

  /** Discovers streaming text models for one provider and resolves a stable selection. */
  async refreshModels(providerId: ProviderId | "" = this.selectedProviderId): Promise<void> {
    if (!isTauri()) return;
    this.providerStatus = "checking";
    this.providerError = null;
    try {
      const usable = filterUsableModels(await discoverModels(providerId || undefined));
      const resolved = resolveModelSelection(
        usable,
        providerId,
        this.providerSettings.lastProviderId,
        this.providerSettings.lastModelId,
      );
      this.selectedProviderId = resolved.providerId;
      this.models = resolved.models;
      const currentSelectionAvailable = this.models.some((model) => modelKey(model) === this.selectedModelKey);
      if (!currentSelectionAvailable) this.selectedModelKey = resolved.selectedModelKey;
      if (!this.memoryAvailable) this.memoryEnabled = false;
      this.providerStatus = this.models.length > 0 ? "available" : "offline";
      if (this.models.length === 0) {
        this.providerError = {
          code: "unavailable",
          message: "The selected provider did not report a streaming text model.",
          retryable: true,
        };
      } else {
        await this.rememberCurrentSelection();
      }
    } catch (error) {
      this.models = [];
      this.selectedModelKey = "";
      this.providerStatus = "offline";
      this.providerError = providerErrorFromUnknown(error);
    }
  }

  /** Switches provider and refreshes only that provider's model list. */
  async changeProvider(providerId: ProviderId): Promise<void> {
    this.memoryEnabled = false;
    this.selectedProviderId = providerId;
    this.models = [];
    this.selectedModelKey = "";
    await this.refreshModels(providerId);
  }

  /** Applies and persists a model selection from the toolbar. */
  async changeModel(selectedModelKey: string): Promise<void> {
    this.selectedModelKey = selectedModelKey;
    if (!this.memoryAvailable) this.memoryEnabled = false;
    await this.rememberCurrentSelection();
  }

  /** Toggles explicit native memory-tool availability for the next compatible provider request. */
  toggleMemory(): void {
    if (this.memoryAvailable && !this.isGenerating) this.memoryEnabled = !this.memoryEnabled;
  }

  /** Applies saved provider settings and rediscovers models. */
  async applyProviderSettings(settings: ProviderSettings): Promise<void> {
    this.providerSettings = settings;
    await this.refreshModels();
  }

  /** Records the active provider/model pair when it differs from persisted settings. */
  private async rememberCurrentSelection(): Promise<void> {
    const model = this.selectedModel;
    if (!model || !this.selectedProviderId) return;
    if (
      this.providerSettings.lastProviderId === this.selectedProviderId &&
      this.providerSettings.lastModelId === model.modelId
    ) {
      return;
    }
    try {
      this.providerSettings = await rememberProviderSelection(this.selectedProviderId, model.modelId);
    } catch (error) {
      console.warn("Could not remember the provider and model selection", error);
    }
  }

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
  handleComposerKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void this.sendMessage();
    }
  }

  /** Starts one provider-qualified chat stream and normalizes its events into message state. */
  async sendMessage(): Promise<void> {
    const submittedPrompt = this.prompt.trim();
    if (!submittedPrompt || this.isGenerating || !this.canSend || !this.attachmentsCanSubmit) {
      return;
    }
    const submittedAttachments = this.attachment.beginSubmission();
    this.isPersistingMessage = true;
    const runContext = await this.history.persistUserMessage(
      submittedPrompt,
      submittedAttachments.map((attachment) => attachment.id),
    );
    this.isPersistingMessage = false;
    if (!runContext) {
      this.attachment.cancelSubmission();
      return;
    }
    const completedAttachments = this.attachment.finishSubmission();
    this.messages.push({
      id: nextMessageId(),
      storageId: runContext.requestMessageId,
      role: "user",
      content: submittedPrompt,
      attachments: completedAttachments,
    });
    this.prompt = "";
    this.attachment.clear();
    this.resizeComposer();
    await this.startGeneration(runContext);
  }

  /** Removes one durable selected-lineage association while preserving retained bytes. */
  async removeMessageAttachment(messageId: string, attachmentId: string): Promise<void> {
    if (this.isGenerating || this.isPersistingMessage) return;
    await this.history.removeMessageAttachment(this.messages, messageId, attachmentId);
  }

  /** Promotes one retained draft item into branch-independent conversation context. */
  async keepDraftAttachmentInConversation(attachmentId: string): Promise<void> {
    if (this.isGenerating || this.isPersistingMessage) return;
    const added = await this.history.addAttachmentsToConversation([attachmentId]);
    if (added) this.attachment.remove(attachmentId);
  }

  /** Removes one branch-independent association while preserving retained content. */
  async removeConversationAttachment(attachmentId: string): Promise<void> {
    if (this.isGenerating || this.isPersistingMessage) return;
    await this.history.removeAttachmentFromConversation(attachmentId);
  }

  /** Forks one durable user request and generates a response on the new selected branch. */
  async editAndRegenerate(message: Message, text: string): Promise<void> {
    if (!message.storageId || this.isGenerating || !this.canSend || !this.conversationAttachmentsCanSubmit) return;
    const branched = await this.history.branchFromUserMessage(message.storageId, text);
    if (!branched) return;
    this.messages = branched.messages;
    await this.startGeneration(branched.context);
  }

  /** Regenerates one response, optionally requiring a retryable terminal state. */
  async regenerateResponse(responseId: number, retryOnly = false): Promise<void> {
    const response = this.messages.find((message) => message.id === responseId && message.role === "assistant");
    if (retryOnly && !response?.retryable) return;
    const request = requestMessageForResponse(this.messages, responseId);
    if (request) await this.editAndRegenerate(request, request.content);
  }

  /** Starts provider generation from one already-persisted request on the selected branch. */
  private async startGeneration(runContext: import("$lib/storage").ProviderRunContext): Promise<void> {
    this.isGenerating = true;
    const run = ++this.generationRun;
    this.activeStage = STARTING_STAGE;
    this.activeRunId = null;
    this.cancellationRequested = false;
    this.currentUsage = null;
    this.providerError = null;
    const model = this.selectedModel;
    const requestMessages = chatTurnsForMessages(this.messages);
    const assistantId = nextMessageId();
    this.activeAssistantId = assistantId;
    this.messages.push({
      id: assistantId,
      role: "assistant",
      content: "",
      model: model ? `${model.displayName} · ${model.providerName}` : "Selected model",
      retryable: false,
    });
    const startedAt = performance.now();
    await this.scrollToBottom();

    try {
      const chatRun = await startChat(
        {
          providerId: model!.providerId,
          modelId: model!.modelId,
          messages: requestMessages,
          memoryEnabled: this.memoryEnabled,
          settings: { reasoningEffort: this.reasoningEffort },
        },
        runContext,
        (event) => this.handleStreamEvent(event, run, assistantId, startedAt),
      );
      if (run === this.generationRun) {
        this.activeRunId = chatRun.runId;
        if (this.cancellationRequested) await cancelChat(chatRun.runId);
      } else await cancelChat(chatRun.runId);
    } catch (error) {
      if (run !== this.generationRun) return;
      const normalized = providerErrorFromUnknown(error);
      const reply = this.messages.find((message) => message.id === assistantId);
      if (reply) {
        reply.content = normalized.message;
        reply.error = true;
        reply.retryable = normalized.retryable;
      }
      this.providerError = normalized;
      if (normalized.code === "unavailable") this.providerStatus = "offline";
      await this.finalizeNativeGeneration(run);
    }
  }

  /** Selects one preserved branch when generation is idle. */
  async selectConversationBranch(branchId: string): Promise<void> {
    if (this.isGenerating || this.isPersistingMessage || branchId === this.history.currentBranchId) return;
    const messages = await this.history.selectBranch(branchId);
    if (messages) {
      this.messages = messages;
      await this.scrollToBottom("auto");
    }
  }

  /** Applies one normalized provider event to the active assistant response. */
  private handleStreamEvent(event: StreamEvent, run: number, assistantId: number, startedAt: number): void {
    if (run !== this.generationRun) return;
    this.activeRunId = event.runId;
    const reply = this.messages.find((message) => message.id === assistantId);
    if (!reply) return;
    if (event.type === "started") {
      this.activeStage = STREAMING_STAGE;
    } else if (event.type === "text_delta") {
      reply.content += event.delta;
      void this.scrollToBottom("auto");
    } else if (event.type === "reasoning_delta") {
      reply.reasoning = (reply.reasoning ?? "") + event.delta;
      void this.scrollToBottom("auto");
    } else if (event.type === "usage_updated") {
      this.currentUsage = event.usage;
    } else if (event.type === "completed") {
      this.currentUsage = event.usage ?? this.currentUsage;
      reply.meta = completionMeta(startedAt, performance.now(), this.currentUsage);
      reply.retryable = false;
      void this.finalizeNativeGeneration(run);
    } else if (event.type === "cancelled") {
      if (reply.content === "") reply.content = "Generation stopped.";
      reply.meta = "Stopped · partial response";
      reply.retryable = true;
      void this.finalizeNativeGeneration(run);
    } else if (event.type === "failed") {
      reply.error = true;
      reply.retryable = event.error.retryable;
      reply.content = reply.content
        ? `${reply.content}\n\nGeneration stopped: ${event.error.message}`
        : event.error.message;
      this.providerError = event.error;
      if (event.error.code === "unavailable") this.providerStatus = "offline";
      void this.finalizeNativeGeneration(run);
    }
  }

  /** Refreshes navigation after Rust has durably closed the response and provider run. */
  private async finalizeNativeGeneration(run: number): Promise<void> {
    this.isPersistingMessage = true;
    const messages = await this.history.refreshAfterGeneration();
    if (messages) this.messages = messages;
    this.isPersistingMessage = false;
    this.finishGeneration(run);
  }

  /** Toggles the next request between no reasoning and low reasoning. */
  toggleReasoning(): void {
    if (!this.isGenerating) this.reasoningEffort = toggleReasoningEffort(this.reasoningEffort);
  }

  /** Clears transient state when the active generation ends. */
  private finishGeneration(run: number): void {
    if (run !== this.generationRun) return;
    this.isGenerating = false;
    this.activeStage = IDLE_STAGE;
    this.activeRunId = null;
    this.activeAssistantId = null;
    this.cancellationRequested = false;
  }

  /** Requests cancellation while native orchestration retains the latest durable checkpoint. */
  stopGenerating(): void {
    if (this.cancellationRequested) return;
    this.cancellationRequested = true;
    const runId = this.activeRunId;
    const reply = this.messages.find((message) => message.id === this.activeAssistantId);
    if (reply) reply.meta = "Stopping · saving partial response";
    if (runId) void cancelChat(runId);
  }
  /** Sends the draft or cancels the current stream according to generation state. */
  handleSendButton(): void {
    if (this.isGenerating) this.stopGenerating();
    else void this.sendMessage();
  }
  /** Clears the active thread; its first submitted prompt creates durable storage. */
  async startNewChat(): Promise<void> {
    if (this.activeRunId) void cancelChat(this.activeRunId);
    this.messages = [];
    await this.history.startNew();
    this.activeStage = IDLE_STAGE;
    this.generationRun += 1;
    this.isGenerating = false;
    this.activeRunId = null;
    this.activeAssistantId = null;
    this.cancellationRequested = false;
    this.prompt = "";
    this.showSidebar = false;
    setTimeout(() => this.composer?.focus(), NEXT_EVENT_LOOP_TICK_MS);
  }
}
