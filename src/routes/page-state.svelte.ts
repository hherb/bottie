/** Reactive presentation state and actions for the Bottie conversation shell. */

import { invoke, isTauri } from "@tauri-apps/api/core";
import { tick } from "svelte";

import {
  completionMeta,
  displayEndpoint,
  filterUsableModels,
  formatBytes,
  isCloudProvider,
  modelKey,
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
  type ChatTurn,
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
  type Attachment,
  type InferenceStage,
  type Message,
  type ProviderStatus,
  type RuntimeInfo,
} from "$lib/presentation";

const IDLE_STAGE = -1;
const STARTING_STAGE = 0;
const STREAMING_STAGE = 1;
const NEXT_EVENT_LOOP_TICK_MS = 0;

/** Owns the reactive state and imperative actions shared by the page's presentation components. */
export class PageState {
  messages = $state<Message[]>(INITIAL_MESSAGES.map((message) => ({ ...message })));
  attachments = $state<Attachment[]>([
    { id: 1, name: "bottie-notes.md", size: "18 KB", kind: "file" },
    { id: 2, name: "architecture.png", size: "1.2 MB", kind: "image" },
  ]);
  prompt = $state("");
  isGenerating = $state(false);
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
  providerSettings = $state<ProviderSettings>({ ...DEFAULT_PROVIDER_SETTINGS });

  private generationRun = 0;
  private messageSequence = Date.now();
  private messageScroll?: HTMLDivElement;
  private composer?: HTMLTextAreaElement;
  private attachmentInput?: HTMLInputElement;

  /** Currently selected provider-qualified model, when discovery has produced one. */
  get selectedModel(): ModelInfo | undefined {
    return this.models.find((model) => modelKey(model) === this.selectedModelKey);
  }

  /** Whether the current provider and model selection can accept a message. */
  get canSend(): boolean {
    return this.providerStatus === "available" && Boolean(this.selectedModel);
  }

  /** Whether the selected route keeps prompt traffic on this device. */
  get isLocalRoute(): boolean {
    return !isCloudProvider(this.selectedProviderId);
  }

  /** Compact active-provider endpoint used by the privacy-route presentation. */
  get selectedProviderEndpoint(): string {
    const baseUrl = {
      ollama: this.providerSettings.ollamaBaseUrl,
      omlx: this.providerSettings.omlxBaseUrl,
      openai: this.providerSettings.openaiBaseUrl,
      anthropic: this.providerSettings.anthropicBaseUrl,
    }[this.selectedProviderId || "ollama"];
    return displayEndpoint(baseUrl);
  }

  /** Normalized activity stages for the active provider. */
  get inferenceStages(): InferenceStage[] {
    return [
      {
        icon: "shield",
        label: this.isLocalRoute ? "Connected locally" : "Cloud route confirmed",
        detail: `Rust → ${this.selectedModel?.providerName ?? "provider"}`,
      },
      {
        icon: "sparkles",
        label: "Streaming response",
        detail: this.reasoningEffort === "low" ? "Low reasoning" : "Reasoning off",
      },
    ];
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
    await this.refreshModels();
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
    this.selectedProviderId = providerId;
    this.models = [];
    this.selectedModelKey = "";
    await this.refreshModels(providerId);
  }

  /** Applies and persists a model selection from the toolbar. */
  async changeModel(selectedModelKey: string): Promise<void> {
    this.selectedModelKey = selectedModelKey;
    await this.rememberCurrentSelection();
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

  /** Registers the hidden attachment input after its component mounts. */
  setAttachmentInput(element: HTMLInputElement): void {
    this.attachmentInput = element;
  }

  /** Opens the native browser file picker used by the attachment preview. */
  openAttachmentPicker(): void {
    this.attachmentInput?.click();
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
    if (!submittedPrompt || this.isGenerating || !this.canSend) return;
    this.messages.push({ id: ++this.messageSequence, role: "user", content: submittedPrompt });
    this.prompt = "";
    this.resizeComposer();
    this.isGenerating = true;
    const run = ++this.generationRun;
    this.activeStage = STARTING_STAGE;
    this.activeRunId = null;
    this.currentUsage = null;
    this.providerError = null;
    const model = this.selectedModel;
    const requestMessages: ChatTurn[] = this.messages
      .filter((message) => message.content.trim() !== "" && !message.error)
      .map((message) => ({
        role: message.role,
        content: [{ type: "text", text: message.content }],
      }));
    const assistantId = ++this.messageSequence;
    this.activeAssistantId = assistantId;
    this.messages.push({
      id: assistantId,
      role: "assistant",
      content: "",
      model: model ? `${model.displayName} · ${model.providerName}` : "Selected model",
    });
    const startedAt = performance.now();
    await this.scrollToBottom();

    try {
      const chatRun = await startChat(
        {
          providerId: model!.providerId,
          modelId: model!.modelId,
          messages: requestMessages,
          settings: { reasoningEffort: this.reasoningEffort },
        },
        (event) => this.handleStreamEvent(event, run, assistantId, startedAt),
      );
      if (run === this.generationRun) this.activeRunId = chatRun.runId;
      else await cancelChat(chatRun.runId);
    } catch (error) {
      if (run !== this.generationRun) return;
      const normalized = providerErrorFromUnknown(error);
      const reply = this.messages.find((message) => message.id === assistantId);
      if (reply) {
        reply.content = normalized.message;
        reply.error = true;
      }
      this.providerError = normalized;
      if (normalized.code === "unavailable") this.providerStatus = "offline";
      this.finishGeneration(run);
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
      this.finishGeneration(run);
    } else if (event.type === "cancelled") {
      if (reply.content === "") reply.content = "Generation stopped.";
      reply.meta = "Stopped · partial response";
      this.finishGeneration(run);
    } else if (event.type === "failed") {
      reply.error = true;
      reply.content = reply.content
        ? `${reply.content}\n\nGeneration stopped: ${event.error.message}`
        : event.error.message;
      this.providerError = event.error;
      if (event.error.code === "unavailable") this.providerStatus = "offline";
      this.finishGeneration(run);
    }
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
  }

  /** Cancels the active native stream and retains any partial response. */
  stopGenerating(): void {
    const runId = this.activeRunId;
    const reply = this.messages.find((message) => message.id === this.activeAssistantId);
    this.generationRun += 1;
    this.isGenerating = false;
    this.activeStage = IDLE_STAGE;
    this.activeRunId = null;
    this.activeAssistantId = null;
    if (reply) {
      if (reply.content === "") reply.content = "Generation stopped.";
      reply.meta = "Stopped · partial response";
    }
    if (runId) void cancelChat(runId);
  }

  /** Sends the draft or cancels the current stream according to generation state. */
  handleSendButton(): void {
    if (this.isGenerating) this.stopGenerating();
    else void this.sendMessage();
  }

  /** Resets the ephemeral conversation fixture and cancels any active stream. */
  startNewChat(): void {
    if (this.activeRunId) void cancelChat(this.activeRunId);
    this.messages = [
      {
        id: ++this.messageSequence,
        role: "assistant",
        model: "bottie",
        content: "Fresh thread. What would you like to explore?",
      },
    ];
    this.activeStage = IDLE_STAGE;
    this.generationRun += 1;
    this.isGenerating = false;
    this.activeRunId = null;
    this.activeAssistantId = null;
    this.prompt = "";
    this.showSidebar = false;
    setTimeout(() => this.composer?.focus(), NEXT_EVENT_LOOP_TICK_MS);
  }

  /** Adds browser-visible attachment metadata without reading any file bytes. */
  addAttachments(event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    for (const file of Array.from(input.files ?? [])) {
      this.attachments.push({
        id: Date.now() + this.attachments.length,
        name: file.name,
        size: formatBytes(file.size),
        kind: file.type.startsWith("image/") ? "image" : "file",
      });
    }
    input.value = "";
  }

  /** Removes one attachment preview by its ephemeral identifier. */
  removeAttachment(id: number): void {
    this.attachments = this.attachments.filter((attachment) => attachment.id !== id);
  }
}
