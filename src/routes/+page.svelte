<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import Icon from "$lib/Icon.svelte";
  import {
    cancelChat,
    discoverModels,
    getDiagnostics,
    getProviderSettings,
    providerErrorFromUnknown,
    rememberProviderSelection,
    startChat,
    testProviderConnection,
    updateProviderSettings,
    type ChatTurn,
    type DiagnosticEntry,
    type LocalProviderId,
    type ModelInfo,
    type ProviderError,
    type ProviderSettings,
    type StreamEvent,
    type Usage,
  } from "$lib/inference";

  type Message = {
    id: number;
    role: "user" | "assistant";
    content: string;
    featured?: boolean;
    model?: string;
    meta?: string;
    error?: boolean;
  };

  type Attachment = {
    id: number;
    name: string;
    size: string;
    kind: "image" | "file";
  };

  type RuntimeInfo = {
    name: string;
    version: string;
    storage: string;
  };

  type ConnectionTestState = {
    status: "idle" | "testing" | "success" | "error";
    message: string;
  };

  const conversationGroups = [
    {
      label: "Today",
      items: [
        { title: "Bottie architecture", active: true },
        { title: "Local model benchmarks", active: false },
      ],
    },
    {
      label: "Yesterday",
      items: [
        { title: "Weekend reading list", active: false },
        { title: "SQLite search notes", active: false },
      ],
    },
    {
      label: "Previous 7 days",
      items: [
        { title: "Rust async patterns", active: false },
        { title: "Kyoto in autumn", active: false },
        { title: "Camera comparison", active: false },
      ],
    },
  ];

  const initialMessages: Message[] = [
    {
      id: 1,
      role: "user",
      content: "Can you turn our bottie notes into a focused implementation plan?",
    },
    {
      id: 2,
      role: "assistant",
      featured: true,
      model: "Product shell fixture",
      content:
        "Absolutely. I’d build bottie as a sequence of small, complete slices—starting with the conversation experience, then connecting inference, persistence, and tools behind it.\n\nThe important boundary is simple: the WebView presents state; the Rust core owns secrets, files, storage, provider calls, and tool execution.",
    },
  ];

  const providerOptions: Array<{ id: LocalProviderId; name: string }> = [
    { id: "ollama", name: "Ollama" },
    { id: "omlx", name: "oMLX" },
  ];

  let messages = $state<Message[]>(initialMessages.map((message) => ({ ...message })));
  let attachments = $state<Attachment[]>([
    { id: 1, name: "bottie-notes.md", size: "18 KB", kind: "file" },
    { id: 2, name: "architecture.png", size: "1.2 MB", kind: "image" },
  ]);
  let prompt = $state("");
  let isGenerating = $state(false);
  let activeStage = $state(-1);
  let generationRun = 0;
  let activeRunId = $state<string | null>(null);
  let activeAssistantId = $state<number | null>(null);
  let messageSequence = Date.now();
  let showContext = $state(true);
  let showSidebar = $state(false);
  let showSettings = $state(false);
  let messageScroll: HTMLDivElement;
  let composer: HTMLTextAreaElement;
  let attachmentInput: HTMLInputElement;
  let runtime = $state<RuntimeInfo>({ name: "bottie", version: "preview", storage: "local" });
  let models = $state<ModelInfo[]>([]);
  let selectedProviderId = $state<LocalProviderId | "">("");
  let selectedModelKey = $state("");
  let providerStatus = $state<"checking" | "available" | "offline" | "browser">(
    isTauri() ? "checking" : "browser",
  );
  let providerError = $state<ProviderError | null>(null);
  let currentUsage = $state<Usage | null>(null);
  let providerSettings = $state<ProviderSettings>({
    omlxBaseUrl: "http://127.0.0.1:8000/",
    ollamaBaseUrl: "http://127.0.0.1:11434/",
    lastProviderId: null,
    lastModelId: null,
  });
  let settingsDraft = $state<ProviderSettings>({
    omlxBaseUrl: "http://127.0.0.1:8000/",
    ollamaBaseUrl: "http://127.0.0.1:11434/",
    lastProviderId: null,
    lastModelId: null,
  });
  let settingsError = $state("");
  let settingsSaving = $state(false);
  let diagnostics = $state<DiagnosticEntry[]>([]);
  let connectionTests = $state<Record<"omlx" | "ollama", ConnectionTestState>>({
    omlx: { status: "idle", message: "" },
    ollama: { status: "idle", message: "" },
  });
  const selectedModel = $derived(models.find((model) => modelKey(model) === selectedModelKey));
  const canSend = $derived(providerStatus === "available" && Boolean(selectedModel));
  const selectedProviderEndpoint = $derived(
    displayEndpoint(
      selectedProviderId === "ollama"
        ? providerSettings.ollamaBaseUrl
        : providerSettings.omlxBaseUrl,
    ),
  );
  const inferenceStages = $derived([
    {
      icon: "shield" as const,
      label: "Connected locally",
      detail: `Rust → ${selectedModel?.providerName ?? "provider"}`,
    },
    { icon: "sparkles" as const, label: "Streaming response", detail: "Text only" },
  ]);

  function modelKey(model: Pick<ModelInfo, "providerId" | "modelId">) {
    return `${model.providerId}:${model.modelId}`;
  }

  function displayEndpoint(baseUrl: string) {
    return baseUrl.replace(/^https?:\/\//, "").replace(/\/$/, "");
  }

  function diagnosticTime(timestampMs: number) {
    return new Date(timestampMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  onMount(async () => {
    if (isTauri()) {
      try {
        runtime = await invoke<RuntimeInfo>("app_info");
      } catch (error) {
        console.warn("Could not read the native runtime information", error);
      }
      try {
        providerSettings = await getProviderSettings();
        settingsDraft = { ...providerSettings };
        selectedProviderId = providerSettings.lastProviderId ?? "";
      } catch (error) {
        console.warn("Could not read local provider settings", error);
      }
      await refreshModels();
    } else {
      providerError = {
        code: "unavailable",
        message: "Browser preview is disconnected. Open the native Tauri app to use local inference.",
        retryable: false,
      };
    }
  });

  async function refreshModels(providerId: LocalProviderId | "" = selectedProviderId) {
    if (!isTauri()) return;
    providerStatus = "checking";
    providerError = null;
    try {
      const discovered = await discoverModels(providerId || undefined);
      const usable = discovered.filter(
        (model) => model.capabilities.text && model.capabilities.streaming,
      );
      const resolvedProvider =
        providerId ||
        (providerSettings.lastProviderId &&
        usable.some((model) => model.providerId === providerSettings.lastProviderId)
          ? providerSettings.lastProviderId
          : (usable[0]?.providerId as LocalProviderId | undefined)) ||
        "";
      selectedProviderId = resolvedProvider;
      models = usable.filter((model) => model.providerId === resolvedProvider);
      if (!models.some((model) => modelKey(model) === selectedModelKey)) {
        const remembered = models.find(
          (model) =>
            providerSettings.lastProviderId === resolvedProvider &&
            model.modelId === providerSettings.lastModelId,
        );
        selectedModelKey = remembered
          ? modelKey(remembered)
          : models[0]
            ? modelKey(models[0])
            : "";
      }
      providerStatus = models.length > 0 ? "available" : "offline";
      if (models.length === 0) {
        providerError = {
          code: "unavailable",
          message: "The local providers did not report a streaming text model.",
          retryable: true,
        };
      } else {
        await rememberCurrentSelection();
      }
    } catch (error) {
      models = [];
      selectedModelKey = "";
      providerStatus = "offline";
      providerError = providerErrorFromUnknown(error);
    }
  }

  async function handleProviderChange() {
    models = [];
    selectedModelKey = "";
    await refreshModels(selectedProviderId);
  }

  async function handleModelChange() {
    await rememberCurrentSelection();
  }

  async function rememberCurrentSelection() {
    const model = selectedModel;
    if (!model || !selectedProviderId) return;
    if (
      providerSettings.lastProviderId === selectedProviderId &&
      providerSettings.lastModelId === model.modelId
    ) {
      return;
    }
    try {
      providerSettings = await rememberProviderSelection(selectedProviderId, model.modelId);
    } catch (error) {
      console.warn("Could not remember the provider and model selection", error);
    }
  }

  async function openProviderSettings() {
    settingsDraft = { ...providerSettings };
    settingsError = "";
    connectionTests = {
      omlx: { status: "idle", message: "" },
      ollama: { status: "idle", message: "" },
    };
    showSettings = true;
    showSidebar = false;
    diagnostics = await getDiagnostics().catch(() => []);
  }

  function closeProviderSettings() {
    if (settingsSaving) return;
    showSettings = false;
  }

  async function testConnection(providerId: LocalProviderId) {
    const baseUrl =
      providerId === "omlx" ? settingsDraft.omlxBaseUrl : settingsDraft.ollamaBaseUrl;
    connectionTests[providerId] = { status: "testing", message: "Testing connection…" };
    settingsError = "";
    try {
      const result = await testProviderConnection(providerId, baseUrl);
      connectionTests[providerId] = {
        status: "success",
        message: `${result.message} ${result.elapsedMs} ms.`,
      };
      if (providerId === "omlx") settingsDraft.omlxBaseUrl = result.baseUrl;
      else settingsDraft.ollamaBaseUrl = result.baseUrl;
    } catch (error) {
      const normalized = providerErrorFromUnknown(error);
      connectionTests[providerId] = { status: "error", message: normalized.message };
    }
    diagnostics = await getDiagnostics().catch(() => diagnostics);
  }

  async function saveProviderSettings(event: SubmitEvent) {
    event.preventDefault();
    if (!isTauri() || isGenerating || settingsSaving) return;
    settingsSaving = true;
    settingsError = "";
    try {
      providerSettings = await updateProviderSettings({ ...settingsDraft });
      settingsDraft = { ...providerSettings };
      await refreshModels();
      showSettings = false;
    } catch (error) {
      settingsError = providerErrorFromUnknown(error).message;
    } finally {
      settingsSaving = false;
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && showSettings) closeProviderSettings();
  }

  async function scrollToBottom(behavior: ScrollBehavior = "smooth") {
    await tick();
    messageScroll?.scrollTo({ top: messageScroll.scrollHeight, behavior });
  }

  function resizeComposer() {
    if (!composer) return;
    composer.style.height = "0";
    composer.style.height = `${Math.min(composer.scrollHeight, 160)}px`;
  }

  function handleComposerKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void sendMessage();
    }
  }

  async function sendMessage() {
    const submittedPrompt = prompt.trim();
    if (!submittedPrompt || isGenerating || !canSend) return;

    messages.push({ id: ++messageSequence, role: "user", content: submittedPrompt });
    prompt = "";
    resizeComposer();
    isGenerating = true;
    const run = ++generationRun;
    activeStage = 0;
    activeRunId = null;
    currentUsage = null;
    providerError = null;
    const model = selectedModel;
    const requestMessages: ChatTurn[] = messages
      .filter((message) => message.content.trim() !== "" && !message.error)
      .map((message) => ({
        role: message.role,
        content: [{ type: "text", text: message.content }],
      }));
    const assistantId = ++messageSequence;
    activeAssistantId = assistantId;
    messages.push({
      id: assistantId,
      role: "assistant",
      content: "",
      model: model ? `${model.displayName} · ${model.providerName}` : "Local model",
    });
    const startedAt = performance.now();
    await scrollToBottom();

    function handleEvent(event: StreamEvent) {
      if (run !== generationRun) return;
      activeRunId = event.runId;
      const reply = messages.find((message) => message.id === assistantId);
      if (!reply) return;

      if (event.type === "started") {
        activeStage = 1;
      } else if (event.type === "text_delta") {
        reply.content += event.delta;
        void tick().then(() =>
          messageScroll?.scrollTo({ top: messageScroll.scrollHeight, behavior: "auto" }),
        );
      } else if (event.type === "usage_updated") {
        currentUsage = event.usage;
      } else if (event.type === "completed") {
        currentUsage = event.usage ?? currentUsage;
        reply.meta = completionMeta(startedAt, currentUsage);
        finishGeneration(run);
      } else if (event.type === "cancelled") {
        if (reply.content === "") reply.content = "Generation stopped.";
        reply.meta = "Stopped · partial response";
        finishGeneration(run);
      } else if (event.type === "failed") {
        reply.error = true;
        reply.content = reply.content
          ? `${reply.content}\n\nGeneration stopped: ${event.error.message}`
          : event.error.message;
        providerError = event.error;
        if (event.error.code === "unavailable") providerStatus = "offline";
        finishGeneration(run);
      }
    }

    try {
      const chatRun = await startChat(
        { providerId: model!.providerId, modelId: model!.modelId, messages: requestMessages },
        handleEvent,
      );
      if (run === generationRun) {
        activeRunId = chatRun.runId;
      } else {
        await cancelChat(chatRun.runId);
      }
    } catch (error) {
      if (run !== generationRun) return;
      const normalized = providerErrorFromUnknown(error);
      const reply = messages.find((message) => message.id === assistantId);
      if (reply) {
        reply.content = normalized.message;
        reply.error = true;
      }
      providerError = normalized;
      if (normalized.code === "unavailable") providerStatus = "offline";
      finishGeneration(run);
    }
  }

  function completionMeta(startedAt: number, usage: Usage | null) {
    const seconds = ((performance.now() - startedAt) / 1000).toFixed(1);
    const output = usage?.outputTokens;
    return output == null ? `${seconds}s · local` : `${seconds}s · ${output} tokens`;
  }

  function finishGeneration(run: number) {
    if (run !== generationRun) return;
    isGenerating = false;
    activeStage = -1;
    activeRunId = null;
    activeAssistantId = null;
  }

  function stopGenerating() {
    const runId = activeRunId;
    const reply = messages.find((message) => message.id === activeAssistantId);
    generationRun += 1;
    isGenerating = false;
    activeStage = -1;
    activeRunId = null;
    activeAssistantId = null;
    if (reply) {
      if (reply.content === "") reply.content = "Generation stopped.";
      reply.meta = "Stopped · partial response";
    }
    if (runId) void cancelChat(runId);
  }

  function handleSendButton() {
    if (isGenerating) {
      stopGenerating();
    } else {
      void sendMessage();
    }
  }

  function startNewChat() {
    if (activeRunId) void cancelChat(activeRunId);
    messages = [
      {
        id: ++messageSequence,
        role: "assistant",
        model: "bottie",
        content: "Fresh local thread. What would you like to explore?",
      },
    ];
    activeStage = -1;
    generationRun += 1;
    isGenerating = false;
    activeRunId = null;
    activeAssistantId = null;
    prompt = "";
    showSidebar = false;
    setTimeout(() => composer?.focus(), 0);
  }

  function addAttachments(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    for (const file of files) {
      attachments.push({
        id: Date.now() + attachments.length,
        name: file.name,
        size: formatBytes(file.size),
        kind: file.type.startsWith("image/") ? "image" : "file",
      });
    }
    input.value = "";
  }

  function removeAttachment(id: number) {
    attachments = attachments.filter((attachment) => attachment.id !== id);
  }

  function formatBytes(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<svelte:head>
  <title>bottie — local-first conversations</title>
  <meta
    name="description"
    content="A private, provider-flexible conversation space with persistent memory."
  />
</svelte:head>

<svelte:window onkeydown={handleWindowKeydown} />

<div class="app-shell">
  <div class="ambient ambient-one"></div>
  <div class="ambient ambient-two"></div>

  {#if showSidebar}
    <button class="mobile-scrim" aria-label="Close conversations" onclick={() => (showSidebar = false)}></button>
  {/if}

  <aside class:mobile-open={showSidebar} class="sidebar" aria-label="Conversation navigation">
    <div class="brand-row">
      <div class="brand-mark" aria-hidden="true">
        <span class="brand-core"></span>
      </div>
      <span class="brand-name">bottie</span>
      <span class="alpha-label">alpha</span>
    </div>

    <button class="new-chat" onclick={startNewChat}>
      <Icon name="new-chat" size={17} />
      <span>New conversation</span>
      <kbd>⌘ N</kbd>
    </button>

    <button class="search-memory">
      <Icon name="search" size={17} />
      <span>Search memory</span>
      <kbd>⌘ K</kbd>
    </button>

    <nav class="conversation-list" aria-label="Past conversations">
      {#each conversationGroups as group}
        <section class="conversation-group">
          <h2>{group.label}</h2>
          {#each group.items as conversation}
            <button class:active={conversation.active} class="conversation-item">
              <span>{conversation.title}</span>
              {#if conversation.active}
                <Icon name="more" size={16} />
              {/if}
            </button>
          {/each}
        </section>
      {/each}
    </nav>

    <div class="sidebar-footer">
      <button class="settings-button" onclick={openProviderSettings}>
        <Icon name="settings" size={18} />
        <span>Settings</span>
      </button>
      <button class="profile-button" aria-label="Open profile settings">
        <span class="avatar">HH</span>
        <span class="profile-copy">
          <strong>Local profile</strong>
          <small>{runtime.version === "preview" ? "Browser preview" : `bottie ${runtime.version}`}</small>
        </span>
        <Icon name="more" size={17} />
      </button>
    </div>
  </aside>

  <main class="workspace">
    <header class="topbar">
      <button
        class="icon-button mobile-menu"
        aria-label="Open conversations"
        onclick={() => (showSidebar = true)}
      >
        <Icon name="menu" size={19} />
      </button>

      <div class="provider-selectors">
        <span
          class:checking={providerStatus === "checking"}
          class:offline={providerStatus === "offline" || providerStatus === "browser"}
          class="provider-pip"
        ></span>
        <label class="provider-pulldown">
          <span>Provider</span>
          <select
            bind:value={selectedProviderId}
            disabled={providerStatus === "browser" || isGenerating}
            aria-label="Choose local provider"
            onchange={handleProviderChange}
          >
            <option value="" disabled>{providerStatus === "browser" ? "Native only" : "Choose provider"}</option>
            {#each providerOptions as provider}
              <option value={provider.id}>{provider.name}</option>
            {/each}
          </select>
        </label>
        <label class="model-pulldown">
          <span>Model</span>
          <select
            bind:value={selectedModelKey}
            disabled={providerStatus !== "available" || isGenerating}
            aria-label="Choose model"
            onchange={handleModelChange}
          >
            {#if models.length === 0}
              <option value="">
                {providerStatus === "checking"
                  ? "Refreshing…"
                  : providerStatus === "browser"
                    ? "Native unavailable"
                    : "No models available"}
              </option>
            {/if}
            {#each models as model (modelKey(model))}
              <option value={modelKey(model)}>{model.displayName}{model.loadState === "loaded" ? " · loaded" : model.loadState === "unloaded" ? " · on demand" : ""}</option>
            {/each}
          </select>
        </label>
      </div>

      <div class="topbar-actions">
        <div class="privacy-pill" title="Messages stay on this device">
          <Icon name="shield" size={14} />
          <span>Local only</span>
        </div>
        <button
          class:active={showContext}
          class="icon-button"
          aria-label="Toggle context panel"
          aria-pressed={showContext}
          onclick={() => (showContext = !showContext)}
        >
          <Icon name="panel" size={18} />
        </button>
        <button class="icon-button" aria-label="Conversation options">
          <Icon name="more" size={19} />
        </button>
      </div>
    </header>

    <div class="message-scroll" bind:this={messageScroll}>
      {#if providerStatus !== "available"}
        <div class:offline={providerStatus === "offline"} class="provider-banner" role="status">
          <Icon name="shield" size={16} />
          <span>
            <strong>{providerStatus === "checking" ? "Connecting to local providers…" : providerError?.message}</strong>
            {#if providerError?.diagnostic}<small>{providerError.diagnostic}</small>{/if}
          </span>
          {#if providerStatus === "offline"}
            <button onclick={() => refreshModels()}>Retry</button>
          {/if}
        </div>
      {/if}
      <div class="conversation-canvas">
        <div class="date-divider"><span>Today · 19:42</span></div>

        {#each messages as message (message.id)}
          <article class:assistant={message.role === "assistant"} class:error={message.error} class="message">
            <div class="message-avatar" class:user-avatar={message.role === "user"}>
              {#if message.role === "assistant"}
                <span class="mini-core"></span>
              {:else}
                <span>HH</span>
              {/if}
            </div>
            <div class="message-content">
              <div class="message-author">
                <strong>{message.role === "assistant" ? "bottie" : "You"}</strong>
                {#if message.role === "assistant"}
                  <span>{message.model ?? selectedModel?.displayName ?? "Local model"}</span>
                {/if}
              </div>

              <div class="message-text">
                {#each message.content.split("\n\n") as paragraph}
                  <p>{paragraph}</p>
                {/each}
                {#if message.content === "" && isGenerating}
                  <span class="typing-caret"></span>
                {/if}
              </div>

              {#if message.featured}
                <div class="architecture-flow" aria-label="Implementation sequence">
                  <div class="flow-step">
                    <span class="step-number">01</span>
                    <span><strong>Conversation shell</strong><small>Streaming, branching, attachments</small></span>
                  </div>
                  <div class="flow-line"></div>
                  <div class="flow-step">
                    <span class="step-number">02</span>
                    <span><strong>Rust orchestration</strong><small>Providers, tools, permissions</small></span>
                  </div>
                  <div class="flow-line"></div>
                  <div class="flow-step">
                    <span class="step-number">03</span>
                    <span><strong>Persistent memory</strong><small>SQLite, FTS5, vector search</small></span>
                  </div>
                </div>

                <div class="source-row">
                  <button><Icon name="brain" size={14} /> 3 memories</button>
                  <button><Icon name="file" size={14} /> 2 attachments</button>
                </div>
              {/if}

              {#if message.role === "assistant" && message.content !== ""}
                <div class="message-actions">
                  <button aria-label="Copy response"><Icon name="copy" size={15} /></button>
                  <button aria-label="Good response"><Icon name="thumbs-up" size={15} /></button>
                  <button aria-label="Poor response"><Icon name="thumbs-down" size={15} /></button>
                  <button aria-label="Regenerate response"><Icon name="refresh" size={15} /></button>
                  {#if message.meta}<span class="response-meta">{message.meta}</span>{/if}
                </div>
              {/if}
            </div>
          </article>
        {/each}

        {#if activeStage >= 0}
          <div class="activity-card" aria-live="polite">
            <div class="activity-heading">
              <span class="activity-orbit"><span></span></span>
              <strong>{activeStage === 0 ? "Starting local inference" : `${selectedModel?.providerName ?? "Provider"} is responding`}</strong>
            </div>
            <div class="activity-stages">
              {#each inferenceStages as stage, index}
                <div class:current={index === activeStage} class:complete={index < activeStage} class="activity-stage">
                  <span class="stage-icon">
                    {#if index < activeStage}
                      <Icon name="check" size={13} strokeWidth={2.4} />
                    {:else}
                      <Icon name={stage.icon} size={14} />
                    {/if}
                  </span>
                  <span><strong>{stage.label}</strong><small>{stage.detail}</small></span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    </div>

    <footer class="composer-zone">
      <div class="composer-shell" class:busy={isGenerating}>
        {#if attachments.length > 0}
          <div class="composer-attachments">
            {#each attachments.slice(0, 3) as attachment (attachment.id)}
              <div class="attachment-chip">
                <span class:image={attachment.kind === "image"} class="chip-icon">
                  <Icon name={attachment.kind} size={14} />
                </span>
                <span>{attachment.name}</span>
                <button aria-label={`Remove ${attachment.name}`} onclick={() => removeAttachment(attachment.id)}>
                  <Icon name="x" size={13} />
                </button>
              </div>
            {/each}
            {#if attachments.length > 3}<span class="more-files">+{attachments.length - 3}</span>{/if}
          </div>
        {/if}

        <textarea
          bind:this={composer}
          bind:value={prompt}
          oninput={resizeComposer}
          onkeydown={handleComposerKeydown}
          rows="1"
          disabled={!canSend && !isGenerating}
          placeholder={providerStatus === "available" ? "Message the local model…" : "Connect a local provider to send a message"}
          aria-label="Message bottie"
        ></textarea>

        <div class="composer-toolbar">
          <div class="composer-tools">
            <input
              class="visually-hidden"
              bind:this={attachmentInput}
              onchange={addAttachments}
              type="file"
              multiple
              tabindex="-1"
            />
            <button aria-label="Attach files" onclick={() => attachmentInput?.click()}>
              <Icon name="paperclip" size={18} />
            </button>
            <button class="tool-toggle" aria-label="Memory search is not available yet" disabled>
              <Icon name="brain" size={17} />
              <span>Memory</span>
            </button>
            <button class="tool-toggle" aria-label="Web search is not available yet" disabled>
              <Icon name="globe" size={17} />
              <span>Web</span>
            </button>
          </div>

          <button
            class="send-button"
            class:enabled={(prompt.trim().length > 0 && canSend) || isGenerating}
            disabled={(!prompt.trim() || !canSend) && !isGenerating}
            aria-label={isGenerating ? "Stop generating" : "Send message"}
            onclick={handleSendButton}
          >
            {#if isGenerating}<span class="stop-square"></span>{:else}<Icon name="arrow-up" size={19} strokeWidth={2.2} />{/if}
          </button>
        </div>
      </div>
      <p class="composer-note">Bottie can make mistakes. Check important information.</p>
    </footer>
  </main>

  <aside class:closed={!showContext} class="context-panel" aria-label="Conversation context">
    <div class="context-header">
      <div>
        <span class="eyebrow">This conversation</span>
        <h2>Context</h2>
      </div>
      <button class="icon-button" aria-label="Close context panel" onclick={() => (showContext = false)}>
        <Icon name="x" size={18} />
      </button>
    </div>

    <section class="context-section">
      <div class="section-heading">
        <h3>Attachments <span>{attachments.length}</span></h3>
        <button onclick={() => attachmentInput?.click()}>Add</button>
      </div>
      <div class="attachment-list">
        {#each attachments as attachment (attachment.id)}
          <div class="attachment-row">
            <span class:image={attachment.kind === "image"} class="attachment-icon">
              <Icon name={attachment.kind} size={18} />
            </span>
            <span class="attachment-copy">
              <strong>{attachment.name}</strong>
              <small>{attachment.size} · Preview only</small>
            </span>
            <button aria-label={`Remove ${attachment.name}`} onclick={() => removeAttachment(attachment.id)}>
              <Icon name="x" size={15} />
            </button>
          </div>
        {/each}
        {#if attachments.length === 0}
          <button class="empty-attachments" onclick={() => attachmentInput?.click()}>
            <Icon name="paperclip" size={18} />
            <span><strong>Add context</strong><small>Images, documents, or text files</small></span>
          </button>
        {/if}
      </div>
    </section>

    <section class="context-section memory-section">
      <div class="section-heading">
        <h3>Preview memories <span>3 fixtures</span></h3>
        <button disabled>Not active</button>
      </div>
      <div class="memory-card cyan">
        <div class="memory-meta"><Icon name="brain" size={14} /> Architecture discussion <span>92%</span></div>
        <p>Keep secrets, storage, provider calls, and tool execution inside the Rust core.</p>
        <small>Today · Bottie architecture</small>
      </div>
      <div class="memory-card violet">
        <div class="memory-meta"><Icon name="brain" size={14} /> Search design <span>86%</span></div>
        <p>Combine SQLite full-text and vector results with reciprocal-rank fusion.</p>
        <small>Yesterday · SQLite search notes</small>
      </div>
      <div class="memory-card amber">
        <div class="memory-meta"><Icon name="brain" size={14} /> Interface preference <span>79%</span></div>
        <p>Tool activity should be visible, calm, and expandable when details matter.</p>
        <small>Today · Bottie architecture</small>
      </div>
    </section>

    <section class="context-section route-section">
      <div class="section-heading"><h3>Privacy route</h3></div>
      <div class="route-card">
        <div class="route-line">
          <span class="route-node device"><Icon name="shield" size={15} /></span>
          <span class="route-track"></span>
          <span class="route-node model"><span class="tiny-core"></span></span>
        </div>
        <div class="route-labels">
          <span><strong>This Mac</strong><small>Conversation + files</small></span>
          <span><strong>{selectedModel?.providerName ?? "Local provider"}</strong><small>{selectedProviderEndpoint}</small></span>
        </div>
        <div class:offline={providerStatus !== "available"} class="route-status">
          <span></span>
          {providerStatus === "available" ? "Connected over loopback" : "Local provider disconnected"}
        </div>
      </div>
    </section>

    <div class="context-footer">
      <span>Estimated context</span>
      <strong>8.4k <small>/ 64k tokens</small></strong>
      <div class="context-meter"><span></span></div>
    </div>
  </aside>

  {#if showSettings}
    <div class="settings-layer">
      <button class="settings-scrim" aria-label="Close provider settings" onclick={closeProviderSettings}></button>
      <div class="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="provider-settings-title">
        <header class="settings-header">
          <div>
            <span class="eyebrow">Rust-owned configuration</span>
            <h2 id="provider-settings-title">Local providers</h2>
          </div>
          <button class="icon-button" aria-label="Close provider settings" onclick={closeProviderSettings}>
            <Icon name="x" size={18} />
          </button>
        </header>

        <form class="settings-content" onsubmit={saveProviderSettings}>
          <p class="settings-intro">
            Bottie accepts loopback endpoints only. Provider traffic and configuration stay behind the native boundary.
          </p>

          <div class="provider-setting">
            <div class="provider-setting-heading">
              <span><strong>oMLX</strong><small>OpenAI-compatible local runtime</small></span>
              <span class="local-badge"><Icon name="shield" size={12} /> Local</span>
            </div>
            <label for="omlx-endpoint">Endpoint</label>
            <div class="endpoint-row">
              <input
                id="omlx-endpoint"
                bind:value={settingsDraft.omlxBaseUrl}
                disabled={!isTauri() || settingsSaving}
                spellcheck="false"
                autocomplete="off"
              />
              <button
                type="button"
                disabled={!isTauri() || settingsSaving || connectionTests.omlx.status === "testing"}
                onclick={() => testConnection("omlx")}
              >Test</button>
            </div>
            {#if connectionTests.omlx.message}
              <p class:error={connectionTests.omlx.status === "error"} class:success={connectionTests.omlx.status === "success"} class="test-result" aria-live="polite">
                {connectionTests.omlx.message}
              </p>
            {/if}
          </div>

          <div class="provider-setting">
            <div class="provider-setting-heading">
              <span><strong>Ollama</strong><small>Native local API</small></span>
              <span class="local-badge"><Icon name="shield" size={12} /> Local</span>
            </div>
            <label for="ollama-endpoint">Endpoint</label>
            <div class="endpoint-row">
              <input
                id="ollama-endpoint"
                bind:value={settingsDraft.ollamaBaseUrl}
                disabled={!isTauri() || settingsSaving}
                spellcheck="false"
                autocomplete="off"
              />
              <button
                type="button"
                disabled={!isTauri() || settingsSaving || connectionTests.ollama.status === "testing"}
                onclick={() => testConnection("ollama")}
              >Test</button>
            </div>
            {#if connectionTests.ollama.message}
              <p class:error={connectionTests.ollama.status === "error"} class:success={connectionTests.ollama.status === "success"} class="test-result" aria-live="polite">
                {connectionTests.ollama.message}
              </p>
            {/if}
          </div>

          <div class="settings-policy">
            <Icon name="shield" size={15} />
            <span><strong>Connection policy</strong><small>3 s connect · 5 s discovery · 120 s stream idle timeout</small></span>
          </div>

          <section class="diagnostics" aria-label="Recent provider diagnostics">
            <div class="diagnostics-heading">
              <span><strong>Recent diagnostics</strong><small>Structured and secret-redacted</small></span>
              <button type="button" onclick={async () => (diagnostics = await getDiagnostics())}>Refresh</button>
            </div>
            {#if diagnostics.length === 0}
              <p class="diagnostics-empty">No provider activity has been recorded this session.</p>
            {:else}
              <div class="diagnostic-list">
                {#each diagnostics.slice(-6).reverse() as entry}
                  <div class:error={entry.level === "error"} class="diagnostic-row">
                    <span>{diagnosticTime(entry.timestampMs)}</span>
                    <strong>{entry.event}</strong>
                    <small>{entry.providerId ?? "native"}{entry.detail ? ` · ${entry.detail}` : ""}</small>
                  </div>
                {/each}
              </div>
            {/if}
          </section>

          {#if !isTauri()}
            <p class="settings-error">Provider settings are read-only in the browser preview.</p>
          {:else if isGenerating}
            <p class="settings-error">Stop the active generation before changing provider settings.</p>
          {:else if settingsError}
            <p class="settings-error" role="alert">{settingsError}</p>
          {/if}

          <footer class="settings-actions">
            <button type="button" class="secondary" onclick={closeProviderSettings}>Cancel</button>
            <button type="submit" class="primary" disabled={!isTauri() || isGenerating || settingsSaving}>
              {settingsSaving ? "Saving…" : "Save and reconnect"}
            </button>
          </footer>
        </form>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(*) { box-sizing: border-box; }
  :global(:root) {
    font-family: Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    color: #f4f2ee;
    background: #0a0a0e;
    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
    --bg: #0a0a0e;
    --panel: rgba(16, 16, 22, 0.92);
    --raised: #17171f;
    --line: rgba(255, 255, 255, 0.075);
    --line-strong: rgba(255, 255, 255, 0.12);
    --text: #f4f2ee;
    --muted: #9897a4;
    --muted-strong: #bebbc6;
    --violet: #8f7df7;
    --cyan: #5bd8c8;
    --amber: #e9a968;
  }
  :global(html), :global(body) { width: 100%; height: 100%; margin: 0; overflow: hidden; background: var(--bg); }
  :global(button), :global(textarea), :global(input) { font: inherit; }
  :global(button) { color: inherit; }
  :global(button:focus-visible), :global(textarea:focus-visible) { outline: 2px solid rgba(143, 125, 247, 0.9); outline-offset: 2px; }

  .app-shell {
    position: relative;
    isolation: isolate;
    display: grid;
    grid-template-columns: 258px minmax(500px, 1fr) 310px;
    width: 100vw;
    height: 100vh;
    min-width: 720px;
    overflow: hidden;
    background: radial-gradient(circle at 58% 4%, rgba(97, 75, 186, 0.08), transparent 32%), #0a0a0e;
  }
  .ambient { position: absolute; z-index: -1; width: 360px; height: 360px; border-radius: 50%; filter: blur(110px); opacity: 0.09; pointer-events: none; }
  .ambient-one { top: -180px; left: 37%; background: #7658ef; }
  .ambient-two { right: -220px; bottom: -160px; background: #2cc9b2; }
  .sidebar, .context-panel { z-index: 3; min-width: 0; background: var(--panel); backdrop-filter: blur(24px); -webkit-backdrop-filter: blur(24px); }
  .sidebar { display: flex; flex-direction: column; border-right: 1px solid var(--line); padding: 22px 14px 14px; overflow: hidden; }
  .brand-row { display: flex; align-items: center; min-height: 36px; padding: 0 8px; margin-bottom: 20px; }
  .brand-mark, .message-avatar:not(.user-avatar) { display: grid; place-items: center; background: linear-gradient(145deg, rgba(143, 125, 247, 0.18), rgba(91, 216, 200, 0.08)); border: 1px solid rgba(143, 125, 247, 0.25); box-shadow: inset 0 0 12px rgba(255, 255, 255, 0.04); }
  .brand-mark { width: 30px; height: 30px; border-radius: 10px; margin-right: 10px; }
  .brand-core, .mini-core, .tiny-core { display: block; border-radius: 50%; background: radial-gradient(circle at 35% 28%, #fff 0 4%, #bdb1ff 12%, transparent 35%), conic-gradient(from 220deg, #5944d7, #8a7bf2, #52d7c5, #5944d7); box-shadow: 0 0 12px rgba(143, 125, 247, 0.55), inset -3px -3px 6px rgba(0, 0, 0, 0.28); }
  .brand-core { width: 14px; height: 14px; }
  .brand-name { font-size: 18px; font-weight: 710; letter-spacing: -0.04em; }
  .alpha-label { align-self: flex-start; margin: 1px 0 0 7px; color: var(--muted); font-size: 8px; font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; }

  .new-chat, .search-memory, .settings-button, .profile-button, .conversation-item { display: flex; align-items: center; width: 100%; border: 0; cursor: pointer; text-align: left; }
  .new-chat { gap: 10px; min-height: 42px; padding: 0 12px; margin-bottom: 7px; border: 1px solid rgba(143, 125, 247, 0.22); border-radius: 11px; background: linear-gradient(135deg, rgba(143, 125, 247, 0.15), rgba(91, 216, 200, 0.06)); color: #eae6ff; font-size: 13px; transition: 160ms ease; }
  .new-chat:hover { border-color: rgba(143, 125, 247, 0.4); transform: translateY(-1px); }
  kbd { margin-left: auto; color: #777582; font-family: inherit; font-size: 10px; }
  .search-memory, .settings-button { gap: 10px; min-height: 38px; padding: 0 12px; border-radius: 9px; background: transparent; color: var(--muted-strong); font-size: 13px; }
  .search-memory:hover, .settings-button:hover, .conversation-item:hover { background: rgba(255, 255, 255, 0.04); color: var(--text); }
  .conversation-list { flex: 1; padding-top: 19px; overflow: auto; scrollbar-width: none; }
  .conversation-group { margin-bottom: 20px; }
  .conversation-group h2 { padding: 0 11px; margin: 0 0 6px; color: #696874; font-size: 10px; font-weight: 650; letter-spacing: 0.075em; text-transform: uppercase; }
  .conversation-item { justify-content: space-between; min-height: 35px; padding: 0 10px 0 11px; border-radius: 8px; background: transparent; color: #a9a7b2; font-size: 12.5px; }
  .conversation-item span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .conversation-item.active { background: rgba(255, 255, 255, 0.055); color: #f2eff7; }
  .sidebar-footer { padding-top: 10px; border-top: 1px solid var(--line); }
  .profile-button { gap: 9px; min-height: 52px; padding: 7px 9px; border-radius: 10px; background: transparent; }
  .profile-button:hover { background: rgba(255, 255, 255, 0.035); }
  .avatar, .user-avatar { background: linear-gradient(145deg, #d8a676, #a96565); color: #23160f; font-size: 9px; font-weight: 800; }
  .avatar { display: grid; flex: 0 0 auto; place-items: center; width: 28px; height: 28px; border-radius: 9px; }
  .profile-copy { display: flex; flex: 1; flex-direction: column; min-width: 0; }
  .profile-copy strong { font-size: 11.5px; font-weight: 580; }
  .profile-copy small { margin-top: 2px; color: #75737e; font-size: 9.5px; }

  .workspace { position: relative; z-index: 1; display: grid; grid-template-rows: 62px minmax(0, 1fr) auto; min-width: 0; height: 100vh; overflow: hidden; }
  .topbar { display: flex; align-items: center; justify-content: space-between; min-width: 0; padding: 0 22px; border-bottom: 1px solid var(--line); }
  .provider-selectors { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .provider-pip { width: 8px; height: 8px; border-radius: 50%; background: var(--cyan); box-shadow: 0 0 10px rgba(91, 216, 200, 0.7); }
  .provider-pip.checking { background: var(--amber); box-shadow: 0 0 10px rgba(233, 169, 104, 0.6); animation: blink 1.2s ease-in-out infinite; }
  .provider-pip.offline { background: #6e6c77; box-shadow: none; }
  .provider-pulldown, .model-pulldown { display: flex; flex-direction: column; gap: 2px; min-width: 0; color: #66646f; font-size: 7px; font-weight: 650; letter-spacing: 0.055em; text-transform: uppercase; }
  .provider-pulldown { width: 104px; }
  .model-pulldown { width: min(280px, 28vw); }
  .provider-pulldown select, .model-pulldown select { width: 100%; min-width: 0; height: 27px; padding: 0 25px 0 8px; overflow: hidden; border: 1px solid var(--line); border-radius: 8px; outline: 0; background: #121219; color: #d3d0d8; font-size: 10px; font-weight: 540; letter-spacing: normal; text-overflow: ellipsis; text-transform: none; cursor: pointer; }
  .provider-pulldown select:hover, .model-pulldown select:hover { border-color: var(--line-strong); background: #17171f; }
  .provider-pulldown select:focus-visible, .model-pulldown select:focus-visible { border-color: rgba(143, 125, 247, 0.55); box-shadow: 0 0 0 2px rgba(143, 125, 247, 0.08); }
  .provider-pulldown select:disabled, .model-pulldown select:disabled { cursor: not-allowed; opacity: 0.62; }
  .topbar-actions, .composer-tools, .source-row, .message-actions { display: flex; align-items: center; }
  .topbar-actions { gap: 6px; }
  .privacy-pill { display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 10px; margin-right: 5px; border: 1px solid rgba(91, 216, 200, 0.15); border-radius: 20px; background: rgba(91, 216, 200, 0.055); color: #8fded3; font-size: 9.5px; font-weight: 600; }
  .icon-button { display: grid; place-items: center; width: 32px; height: 32px; padding: 0; border: 0; border-radius: 8px; background: transparent; color: #888692; cursor: pointer; }
  .icon-button:hover, .icon-button.active { background: rgba(255, 255, 255, 0.05); color: #dbd8e1; }
  .mobile-menu { display: none; }

  .message-scroll { min-height: 0; overflow-x: hidden; overflow-y: auto; scrollbar-color: #2b2a34 transparent; scrollbar-width: thin; }
  .provider-banner { display: flex; align-items: center; gap: 10px; width: min(760px, calc(100% - 64px)); padding: 10px 12px; margin: 18px auto -11px; border: 1px solid rgba(233, 169, 104, 0.16); border-radius: 11px; background: rgba(233, 169, 104, 0.045); color: #c9a57d; }
  .provider-banner.offline { border-color: rgba(233, 118, 104, 0.18); background: rgba(233, 118, 104, 0.045); color: #d49a91; }
  .provider-banner > span { display: flex; flex: 1; flex-direction: column; min-width: 0; }
  .provider-banner strong { font-size: 9px; font-weight: 590; }
  .provider-banner small { margin-top: 3px; overflow: hidden; color: #706d76; font-size: 7px; text-overflow: ellipsis; white-space: nowrap; }
  .provider-banner button { padding: 5px 8px; border: 1px solid currentColor; border-radius: 7px; background: transparent; color: inherit; font-size: 8px; cursor: pointer; }
  .conversation-canvas { width: min(760px, calc(100% - 64px)); margin: 0 auto; padding: 31px 0 54px; }
  .date-divider { display: flex; align-items: center; gap: 12px; margin: 0 0 30px; color: #5e5d67; font-size: 8.5px; font-weight: 650; letter-spacing: 0.09em; text-transform: uppercase; }
  .date-divider::before, .date-divider::after { height: 1px; background: var(--line); content: ""; }
  .date-divider::before { width: 20px; }
  .date-divider::after { flex: 1; }
  .message { display: grid; grid-template-columns: 32px minmax(0, 1fr); gap: 13px; margin-bottom: 29px; }
  .message-avatar { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 9px; }
  .mini-core { width: 12px; height: 12px; }
  .message-content { min-width: 0; }
  .message-author { display: flex; align-items: baseline; gap: 8px; min-height: 22px; }
  .message-author strong { font-size: 12px; font-weight: 650; letter-spacing: -0.01em; }
  .message-author span { color: #686671; font-size: 8.5px; }
  .message-text { color: #dedbe2; font-size: 13px; line-height: 1.68; }
  .message:not(.assistant) .message-text { color: #f2eff3; font-size: 13.5px; }
  .message.error .message-text { color: #dca39b; }
  .message-text p { margin: 0 0 10px; }
  .typing-caret { display: inline-block; width: 6px; height: 15px; border-radius: 2px; background: var(--violet); animation: blink 900ms steps(2, start) infinite; }

  .architecture-flow { display: grid; grid-template-columns: 1fr 19px 1fr 19px 1fr; align-items: center; padding: 13px; margin-top: 17px; border: 1px solid var(--line); border-radius: 13px; background: linear-gradient(145deg, rgba(255, 255, 255, 0.036), rgba(255, 255, 255, 0.012)); }
  .flow-step { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .flow-step > span:last-child { display: flex; flex-direction: column; min-width: 0; }
  .step-number { display: grid; flex: 0 0 auto; place-items: center; width: 27px; height: 27px; border: 1px solid rgba(143, 125, 247, 0.18); border-radius: 8px; background: rgba(143, 125, 247, 0.08); color: #a99bf5; font-size: 8px; font-weight: 700; }
  .flow-step strong { overflow: hidden; color: #e8e4eb; font-size: 9.5px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
  .flow-step small { overflow: hidden; margin-top: 3px; color: #777581; font-size: 7.5px; text-overflow: ellipsis; white-space: nowrap; }
  .flow-line { height: 1px; margin: 0 5px; background: linear-gradient(90deg, rgba(143, 125, 247, 0.3), rgba(91, 216, 200, 0.25)); }
  .source-row { gap: 7px; margin-top: 10px; }
  .source-row button { display: flex; align-items: center; gap: 5px; height: 25px; padding: 0 8px; border: 1px solid var(--line); border-radius: 7px; background: rgba(255, 255, 255, 0.02); color: #888691; font-size: 8.5px; cursor: pointer; }
  .source-row button:hover { border-color: var(--line-strong); color: var(--muted-strong); }
  .message-actions { gap: 2px; min-height: 28px; margin-top: 7px; opacity: 0.56; transition: opacity 150ms ease; }
  .message:hover .message-actions { opacity: 1; }
  .message-actions button, .attachment-chip button, .attachment-row button, .composer-tools button { display: grid; place-items: center; padding: 0; border: 0; background: transparent; color: #777580; cursor: pointer; }
  .message-actions button { width: 26px; height: 26px; border-radius: 6px; }
  .message-actions button:hover, .composer-tools button:hover { background: rgba(255, 255, 255, 0.05); color: #d8d5dc; }
  .response-meta { margin-left: 6px; color: #5f5d67; font-size: 8px; }

  .activity-card { width: calc(100% - 45px); padding: 13px 14px; margin: -9px 0 24px 45px; border: 1px solid rgba(143, 125, 247, 0.16); border-radius: 12px; background: rgba(143, 125, 247, 0.035); }
  .activity-heading { display: flex; align-items: center; gap: 9px; margin-bottom: 12px; color: #ccc5ef; font-size: 10px; }
  .activity-orbit { display: grid; place-items: center; width: 18px; height: 18px; border: 1px solid rgba(143, 125, 247, 0.35); border-top-color: var(--cyan); border-radius: 50%; animation: spin 1.2s linear infinite; }
  .activity-orbit span { width: 5px; height: 5px; border-radius: 50%; background: var(--violet); }
  .activity-stages { display: grid; grid-template-columns: repeat(2, 1fr); gap: 7px; }
  .activity-stage { display: flex; align-items: center; gap: 7px; min-width: 0; opacity: 0.4; }
  .activity-stage.current, .activity-stage.complete { opacity: 1; }
  .stage-icon { display: grid; flex: 0 0 auto; place-items: center; width: 23px; height: 23px; border-radius: 7px; background: rgba(255, 255, 255, 0.045); color: var(--muted); }
  .activity-stage.current .stage-icon { background: rgba(143, 125, 247, 0.12); color: #a99df1; box-shadow: 0 0 12px rgba(143, 125, 247, 0.12); }
  .activity-stage.complete .stage-icon { color: var(--cyan); }
  .activity-stage > span:last-child { display: flex; flex-direction: column; min-width: 0; }
  .activity-stage strong, .activity-stage small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .activity-stage strong { color: #bdb9c4; font-size: 8.5px; font-weight: 560; }
  .activity-stage small { margin-top: 2px; color: #66646f; font-size: 7px; }

  .composer-zone { position: relative; z-index: 2; padding: 0 30px 12px; background: linear-gradient(180deg, transparent, var(--bg) 25%); }
  .composer-shell { width: min(760px, 100%); margin: 0 auto; border: 1px solid var(--line-strong); border-radius: 16px; background: rgba(23, 23, 31, 0.92); box-shadow: 0 18px 60px rgba(0, 0, 0, 0.32), inset 0 1px rgba(255, 255, 255, 0.025); transition: border-color 160ms ease, box-shadow 160ms ease; }
  .composer-shell:focus-within { border-color: rgba(143, 125, 247, 0.34); box-shadow: 0 18px 60px rgba(0, 0, 0, 0.36), 0 0 0 3px rgba(143, 125, 247, 0.04); }
  .composer-shell.busy { border-color: rgba(91, 216, 200, 0.2); }
  .composer-attachments { display: flex; gap: 6px; padding: 10px 11px 0; overflow: hidden; }
  .attachment-chip { display: flex; align-items: center; gap: 6px; min-width: 0; max-width: 180px; height: 29px; padding: 0 6px; border: 1px solid var(--line); border-radius: 8px; background: rgba(255, 255, 255, 0.025); color: #aaa7b0; font-size: 8.5px; }
  .attachment-chip > span:nth-child(2) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip-icon { display: grid; flex: 0 0 auto; place-items: center; width: 19px; height: 19px; border-radius: 5px; background: rgba(143, 125, 247, 0.1); color: #9f91ee; }
  .chip-icon.image { background: rgba(91, 216, 200, 0.09); color: #75cfc3; }
  .attachment-chip button { flex: 0 0 auto; width: 19px; height: 19px; border-radius: 5px; }
  .attachment-chip button:hover, .attachment-row button:hover { background: rgba(255, 255, 255, 0.06); color: #d5d2d9; }
  .more-files { align-self: center; color: #777581; font-size: 9px; }
  textarea { display: block; width: 100%; min-height: 48px; max-height: 160px; padding: 15px 15px 7px; resize: none; overflow-y: auto; border: 0; outline: 0; background: transparent; color: var(--text); font-size: 12.5px; line-height: 1.5; }
  textarea::placeholder { color: #6e6c77; }
  textarea:disabled { cursor: not-allowed; opacity: 0.7; }
  .composer-toolbar { display: flex; align-items: center; justify-content: space-between; padding: 4px 8px 8px; }
  .composer-tools { gap: 3px; }
  .composer-tools button { height: 29px; border-radius: 8px; }
  .composer-tools button:disabled { cursor: not-allowed; opacity: 0.48; }
  .composer-tools button:not(.tool-toggle) { width: 30px; }
  .tool-toggle { display: flex !important; grid: none !important; gap: 5px; padding: 0 8px !important; font-size: 9px; }
  .send-button { display: grid; place-items: center; width: 31px; height: 31px; padding: 0; border: 0; border-radius: 9px; background: #292832; color: #6c6a75; cursor: default; transition: 150ms ease; }
  .send-button.enabled, .composer-shell.busy .send-button { background: linear-gradient(145deg, #8b78f0, #6d5dd4); color: white; cursor: pointer; box-shadow: 0 4px 16px rgba(109, 93, 212, 0.28); }
  .send-button.enabled:hover { transform: translateY(-1px); filter: brightness(1.08); }
  .stop-square { width: 9px; height: 9px; border-radius: 2px; background: currentColor; }
  .composer-note { margin: 7px auto 0; color: #55535d; font-size: 8px; text-align: center; }

  .context-panel { display: flex; flex-direction: column; width: 310px; border-left: 1px solid var(--line); overflow: hidden; transition: width 220ms ease, opacity 180ms ease, border-color 180ms ease; }
  .context-panel.closed { width: 0; border-color: transparent; opacity: 0; pointer-events: none; }
  .context-header { display: flex; flex: 0 0 auto; align-items: center; justify-content: space-between; min-height: 77px; padding: 16px 17px 12px 19px; border-bottom: 1px solid var(--line); }
  .eyebrow { display: block; margin-bottom: 4px; color: #65636e; font-size: 7.5px; font-weight: 670; letter-spacing: 0.1em; text-transform: uppercase; }
  .context-header h2 { margin: 0; font-size: 15px; font-weight: 620; letter-spacing: -0.02em; }
  .context-section { flex: 0 0 auto; padding: 16px 18px; border-bottom: 1px solid var(--line); }
  .section-heading { display: flex; align-items: center; justify-content: space-between; margin-bottom: 11px; }
  .section-heading h3 { margin: 0; color: #b5b2bb; font-size: 9px; font-weight: 640; letter-spacing: 0.015em; }
  .section-heading h3 span { margin-left: 4px; color: #62606b; font-size: 8px; }
  .section-heading button { padding: 0; border: 0; background: transparent; color: #8378c5; font-size: 8.5px; cursor: pointer; }
  .section-heading button:hover { color: #a99af2; }
  .section-heading button:disabled { color: #62606b; cursor: default; }
  .attachment-list { display: flex; flex-direction: column; gap: 7px; }
  .attachment-row { display: flex; align-items: center; gap: 9px; min-width: 0; padding: 7px; border: 1px solid var(--line); border-radius: 10px; background: rgba(255, 255, 255, 0.018); }
  .attachment-icon { display: grid; flex: 0 0 auto; place-items: center; width: 31px; height: 31px; border-radius: 8px; background: rgba(143, 125, 247, 0.09); color: #9c8ee9; }
  .attachment-icon.image { background: rgba(91, 216, 200, 0.075); color: #6fc8bc; }
  .attachment-copy { display: flex; flex: 1; flex-direction: column; min-width: 0; }
  .attachment-copy strong { overflow: hidden; color: #c6c3cb; font-size: 9px; font-weight: 550; text-overflow: ellipsis; white-space: nowrap; }
  .attachment-copy small { margin-top: 3px; color: #65636d; font-size: 7.5px; }
  .attachment-row button { flex: 0 0 auto; width: 23px; height: 23px; border-radius: 6px; }
  .empty-attachments { display: flex; align-items: center; gap: 9px; width: 100%; padding: 10px; border: 1px dashed var(--line-strong); border-radius: 10px; background: transparent; color: var(--muted); cursor: pointer; text-align: left; }
  .empty-attachments span { display: flex; flex-direction: column; }
  .empty-attachments strong { font-size: 9px; }
  .empty-attachments small { margin-top: 2px; color: #65636d; font-size: 7.5px; }
  .memory-section { min-height: 0; overflow-y: auto; scrollbar-width: none; }
  .memory-card { position: relative; padding: 10px 10px 9px; margin-bottom: 7px; overflow: hidden; border: 1px solid var(--line); border-radius: 10px; background: rgba(255, 255, 255, 0.018); }
  .memory-card::before { position: absolute; top: 0; bottom: 0; left: 0; width: 2px; content: ""; }
  .memory-card.cyan::before { background: var(--cyan); }
  .memory-card.violet::before { background: var(--violet); }
  .memory-card.amber::before { background: var(--amber); }
  .memory-meta { display: flex; align-items: center; gap: 5px; color: #8b8994; font-size: 7.5px; }
  .memory-meta span { margin-left: auto; color: #64626c; }
  .memory-card p { display: -webkit-box; margin: 8px 0 7px; overflow: hidden; color: #bcb9c1; font-size: 9px; line-height: 1.48; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
  .memory-card > small { color: #5f5d67; font-size: 7px; }
  .route-card { padding: 12px; border: 1px solid var(--line); border-radius: 11px; background: rgba(255, 255, 255, 0.018); }
  .route-line { display: flex; align-items: center; padding: 0 7px; }
  .route-node { display: grid; flex: 0 0 auto; place-items: center; width: 27px; height: 27px; border-radius: 8px; }
  .route-node.device { background: rgba(91, 216, 200, 0.09); color: var(--cyan); }
  .route-node.model { background: rgba(143, 125, 247, 0.09); }
  .tiny-core { width: 9px; height: 9px; }
  .route-track { position: relative; flex: 1; height: 1px; background: linear-gradient(90deg, rgba(91, 216, 200, 0.35), rgba(143, 125, 247, 0.35)); }
  .route-track::after { position: absolute; top: -2px; left: 52%; width: 5px; height: 5px; border-radius: 50%; background: #9b8bec; box-shadow: 0 0 8px #8f7df7; content: ""; }
  .route-labels { display: flex; justify-content: space-between; margin-top: 7px; }
  .route-labels > span { display: flex; flex-direction: column; }
  .route-labels > span:last-child { text-align: right; }
  .route-labels strong { color: #b8b5bd; font-size: 8px; font-weight: 570; }
  .route-labels small { margin-top: 2px; color: #5f5d67; font-size: 6.5px; }
  .route-status { display: flex; align-items: center; justify-content: center; gap: 6px; padding-top: 9px; margin-top: 9px; border-top: 1px solid var(--line); color: #78c9bd; font-size: 7px; }
  .route-status span { width: 5px; height: 5px; border-radius: 50%; background: var(--cyan); box-shadow: 0 0 6px rgba(91, 216, 200, 0.6); }
  .route-status.offline { color: #777581; }
  .route-status.offline span { background: #64626c; box-shadow: none; }
  .context-footer { margin-top: auto; padding: 13px 18px 16px; background: rgba(0, 0, 0, 0.08); color: #6e6c76; font-size: 7.5px; }
  .context-footer strong { float: right; color: #a9a6ae; font-size: 8px; font-weight: 570; }
  .context-footer strong small { color: #5d5b65; font-size: 7px; font-weight: 450; }
  .context-meter { height: 3px; margin-top: 9px; overflow: hidden; border-radius: 4px; background: #25242d; }
  .context-meter span { display: block; width: 13%; height: 100%; border-radius: inherit; background: linear-gradient(90deg, var(--violet), var(--cyan)); }

  .mobile-scrim { display: none; }
  .settings-layer { position: absolute; z-index: 20; display: grid; place-items: center; inset: 0; padding: 24px; }
  .settings-scrim { position: absolute; inset: 0; border: 0; background: rgba(3, 3, 7, 0.72); backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px); }
  .settings-dialog { position: relative; display: flex; flex-direction: column; width: min(620px, 100%); max-height: min(760px, calc(100vh - 48px)); overflow: hidden; border: 1px solid var(--line-strong); border-radius: 18px; background: #13131a; box-shadow: 0 28px 90px rgba(0, 0, 0, 0.58); }
  .settings-header { display: flex; align-items: center; justify-content: space-between; min-height: 72px; padding: 15px 18px 13px 22px; border-bottom: 1px solid var(--line); }
  .settings-header h2 { margin: 0; font-size: 17px; font-weight: 630; letter-spacing: -0.025em; }
  .settings-content { padding: 19px 22px 20px; overflow-y: auto; }
  .settings-intro { margin: 0 0 16px; color: var(--muted); font-size: 11px; line-height: 1.55; }
  .provider-setting { padding: 14px; margin-bottom: 10px; border: 1px solid var(--line); border-radius: 12px; background: rgba(255, 255, 255, 0.018); }
  .provider-setting-heading, .diagnostics-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .provider-setting-heading > span:first-child, .diagnostics-heading > span { display: flex; flex-direction: column; }
  .provider-setting-heading strong, .diagnostics-heading strong { color: #dbd8e0; font-size: 11px; font-weight: 620; }
  .provider-setting-heading small, .diagnostics-heading small { margin-top: 3px; color: #696772; font-size: 8px; }
  .local-badge { display: flex; align-items: center; gap: 4px; color: #79cabe; font-size: 8px; font-weight: 620; }
  .provider-setting label { display: block; margin: 13px 0 6px; color: #8d8a96; font-size: 8px; font-weight: 620; text-transform: uppercase; letter-spacing: 0.06em; }
  .endpoint-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; }
  .endpoint-row input { min-width: 0; height: 36px; padding: 0 10px; border: 1px solid var(--line-strong); border-radius: 8px; outline: 0; background: #0d0d12; color: #d9d6dd; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; }
  .endpoint-row input:focus { border-color: rgba(143, 125, 247, 0.5); box-shadow: 0 0 0 3px rgba(143, 125, 247, 0.07); }
  .endpoint-row button, .diagnostics-heading button { border: 1px solid var(--line-strong); border-radius: 8px; background: rgba(255, 255, 255, 0.035); color: #b6b2bd; cursor: pointer; font-size: 9px; }
  .endpoint-row button { min-width: 62px; padding: 0 11px; }
  .endpoint-row button:hover, .diagnostics-heading button:hover { background: rgba(255, 255, 255, 0.065); color: var(--text); }
  .endpoint-row button:disabled { cursor: not-allowed; opacity: 0.48; }
  .test-result { margin: 8px 0 0; color: #8b8994; font-size: 8.5px; }
  .test-result.success { color: #77c7bb; }
  .test-result.error, .settings-error { color: #d89a91; }
  .settings-policy { display: flex; align-items: center; gap: 9px; padding: 10px 12px; margin: 13px 0; border: 1px solid rgba(91, 216, 200, 0.11); border-radius: 10px; background: rgba(91, 216, 200, 0.035); color: #78cabe; }
  .settings-policy span { display: flex; flex-direction: column; }
  .settings-policy strong { font-size: 8.5px; font-weight: 610; }
  .settings-policy small { margin-top: 2px; color: #617d7a; font-size: 7.5px; }
  .diagnostics { padding-top: 13px; border-top: 1px solid var(--line); }
  .diagnostics-heading button { padding: 5px 8px; }
  .diagnostics-empty { margin: 12px 0 2px; color: #65636e; font-size: 8.5px; }
  .diagnostic-list { margin-top: 10px; border: 1px solid var(--line); border-radius: 9px; overflow: hidden; }
  .diagnostic-row { display: grid; grid-template-columns: 48px minmax(0, 1fr) auto; gap: 8px; padding: 7px 9px; border-bottom: 1px solid var(--line); font-size: 7.5px; }
  .diagnostic-row:last-child { border-bottom: 0; }
  .diagnostic-row > span { color: #66646f; }
  .diagnostic-row strong { overflow: hidden; color: #aaa7b0; font-weight: 540; text-overflow: ellipsis; white-space: nowrap; }
  .diagnostic-row small { color: #686672; }
  .diagnostic-row.error strong { color: #d0948c; }
  .settings-error { margin: 12px 0 0; font-size: 9px; }
  .settings-actions { display: flex; justify-content: flex-end; gap: 8px; padding-top: 17px; margin-top: 16px; border-top: 1px solid var(--line); }
  .settings-actions button { height: 34px; padding: 0 13px; border-radius: 8px; cursor: pointer; font-size: 9px; font-weight: 610; }
  .settings-actions .secondary { border: 1px solid var(--line-strong); background: transparent; color: #9b98a3; }
  .settings-actions .primary { border: 1px solid rgba(143, 125, 247, 0.35); background: linear-gradient(145deg, #8070dc, #6555c6); color: white; }
  .settings-actions button:disabled { cursor: not-allowed; opacity: 0.5; }
  .visually-hidden { position: fixed; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; clip-path: inset(50%); }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes blink { 50% { opacity: 0; } }

  @media (max-width: 1180px) {
    .app-shell { grid-template-columns: 232px minmax(500px, 1fr) 290px; }
    .context-panel { width: 290px; }
    .conversation-canvas { width: min(700px, calc(100% - 44px)); }
    .architecture-flow { grid-template-columns: 1fr; gap: 7px; }
    .flow-line { display: none; }
  }
  @media (max-width: 980px) {
    .app-shell { grid-template-columns: 232px minmax(480px, 1fr) auto; }
    .context-panel { position: absolute; top: 0; right: 0; bottom: 0; width: 300px; box-shadow: -24px 0 60px rgba(0, 0, 0, 0.3); }
    .context-panel.closed { width: 0; }
  }
  @media (max-width: 820px) {
    .app-shell { grid-template-columns: minmax(0, 1fr); min-width: 0; }
    .sidebar { position: absolute; top: 0; bottom: 0; left: 0; width: 250px; transform: translateX(-102%); transition: transform 200ms ease; }
    .sidebar.mobile-open { transform: translateX(0); box-shadow: 24px 0 60px rgba(0, 0, 0, 0.4); }
    .mobile-scrim { position: absolute; z-index: 2; display: block; inset: 0; border: 0; background: rgba(0, 0, 0, 0.55); backdrop-filter: blur(3px); }
    .mobile-menu { display: grid; }
    .topbar { gap: 8px; padding: 0 14px; }
    .provider-selectors { margin-right: auto; }
  }
  @media (max-width: 600px) {
    .privacy-pill, .tool-toggle span { display: none; }
    .conversation-canvas { width: calc(100% - 30px); padding-top: 23px; }
    .composer-zone { padding: 0 12px 9px; }
    .activity-stages { grid-template-columns: 1fr; }
    .context-panel { width: min(320px, 92vw); }
    .provider-pulldown { width: 88px; }
    .model-pulldown { width: min(180px, 38vw); }
    .settings-layer { padding: 10px; }
    .settings-dialog { max-height: calc(100vh - 20px); }
    .settings-content { padding: 16px; }
  }
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { scroll-behavior: auto !important; animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; transition-duration: 0.01ms !important; }
  }
</style>
