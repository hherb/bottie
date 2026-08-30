<script lang="ts">
  import { onMount, tick } from "svelte";

  import CommandPalette from "$lib/CommandPalette.svelte";
  import Composer from "$lib/Composer.svelte";
  import ContextPanel from "$lib/ContextPanel.svelte";
  import ConversationView from "$lib/ConversationView.svelte";
  import FirstRunSetup from "$lib/FirstRunSetup.svelte";
  import ProviderSettingsDialog from "$lib/ProviderSettingsDialog.svelte";
  import ProviderToolbar from "$lib/ProviderToolbar.svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import StorageRecovery from "$lib/StorageRecovery.svelte";
  import { composerAttachmentNote } from "$lib/chat";
  import { createAppearanceController, type AppearanceController, type AppearancePreferences } from "$lib/appearance";
  import { webSearchProviderName } from "$lib/presentation";
  import { canBatchExportConversations } from "$lib/storage";
  import {
    commandForKeyboardEvent,
    isCommandPaletteShortcut,
    type CommandId,
    type CommandPaletteItem,
  } from "$lib/command-palette";
  import "$lib/styles/shell.css";
  import "$lib/styles/conversation-nav.css";
  import "$lib/styles/conversation.css";
  import "$lib/styles/conversation-status.css";
  import "$lib/styles/message-attachments.css";
  import "$lib/styles/tool-activity.css";
  import "$lib/styles/markdown.css";
  import "$lib/styles/composer.css";
  import "$lib/styles/command-palette.css";
  import "$lib/styles/context.css";
  import "$lib/styles/first-run.css";
  import "$lib/styles/settings.css";
  import "$lib/styles/diagnostics.css";
  import "$lib/styles/localmail-settings.css";
  import "$lib/styles/recovery.css";
  import "$lib/styles/appearance.css";

  import { PageState } from "./page-state.svelte";
  import { applyPerformancePreview } from "./performance-preview";
  import { applyVoicePreview } from "./voice-preview";
  import {
    inferenceStages,
    messageAttachmentAssociations,
    nextRequestAttachments,
    selectedProviderEndpoint,
  } from "./page-presentation";

  const state = new PageState();
  let appearanceController: AppearanceController | null = null;
  let commandPaletteInvoker: HTMLElement | null = null;
  let settingsInvoker: HTMLElement | null = null;

  onMount(() => {
    if (import.meta.env.DEV) {
      applyPerformancePreview(state, window.location.search);
      applyVoicePreview(state, window.location.search);
    }
    appearanceController = createAppearanceController({
      root: document.documentElement,
      storage: window.localStorage,
      mediaQuery: window.matchMedia("(prefers-color-scheme: dark)"),
      onChange: (preferences) => (state.appearance = preferences),
    });
    state.commandPalette.platform = navigator.platform;
    void state.initialize();
    return () => {
      appearanceController?.dispose();
      state.dispose();
    };
  });

  /** Persists and applies a complete local presentation preference. */
  function updateAppearance(preferences: AppearancePreferences): void {
    appearanceController?.update(preferences);
  }

  /** Completes guided recovery before returning to normal conversation and provider initialization. */
  async function recoverStorage(source: "manual" | "automatic"): Promise<void> {
    const messages = await state.recovery.restore(state.history, source);
    if (messages === null) return;
    state.messages = messages;
    await state.refreshModels();
  }

  /** Opens the palette while retaining the exact invoking control for Escape focus restoration. */
  function openCommandPalette(invoker: HTMLElement | null): void {
    commandPaletteInvoker = invoker;
    state.commandPalette.open = true;
  }

  /** Closes the palette and optionally restores focus to the control that opened it. */
  async function closeCommandPalette(restoreFocus = true): Promise<void> {
    state.commandPalette.open = false;
    await tick();
    if (restoreFocus) commandPaletteInvoker?.focus();
    commandPaletteInvoker = null;
  }

  /** Focuses one existing sidebar surface after responsive navigation becomes visible. */
  async function focusSidebar(target: "navigation" | "search"): Promise<void> {
    state.showSidebar = true;
    await tick();
    const selector = target === "search" ? "#conversation-search" : "#new-conversation-button";
    document.querySelector<HTMLElement>(selector)?.focus();
  }

  /** Opens Settings and retains a still-mounted invoking control for focus restoration. */
  function openSettings(invoker: HTMLElement | null): void {
    settingsInvoker = invoker;
    state.showSettings = true;
    state.showSidebar = false;
  }

  /** Closes Settings, refreshes Email readiness, and restores the invoking control when possible. */
  async function closeSettings(): Promise<void> {
    state.showSettings = false;
    void state.refreshEmailTools();
    await tick();
    if (settingsInvoker?.isConnected && getComputedStyle(settingsInvoker).visibility !== "hidden") {
      settingsInvoker.focus();
    } else if (settingsInvoker) {
      document.querySelector<HTMLElement>("#conversation-sidebar-toggle")?.focus();
    }
    settingsInvoker = null;
  }

  /** Closes responsive conversation navigation and returns focus to its toolbar toggle. */
  async function closeSidebar(): Promise<void> {
    state.showSidebar = false;
    await tick();
    document.querySelector<HTMLElement>("#conversation-sidebar-toggle")?.focus();
  }

  /** Closes Context from its own control and returns focus to the toolbar toggle. */
  async function closeContext(): Promise<void> {
    state.showContext = false;
    await tick();
    document.querySelector<HTMLElement>("#context-panel-toggle")?.focus();
  }

  /** Returns the current command registry from shell-owned reactive availability. */
  function commandItems(): CommandPaletteItem[] {
    return state.commandPalette.items({
      busy: state.isGenerating || state.isPersistingMessage || state.history.isManaging,
      contextOpen: state.showContext,
      storageAvailable: state.runtime.version !== "preview",
    });
  }

  /** Executes one registry command through existing page actions only. */
  async function runCommand(id: CommandId): Promise<void> {
    const command = commandItems().find((item) => item.id === id);
    if (!command || command.disabledReason) return;
    const invoker = commandPaletteInvoker;
    await closeCommandPalette(false);
    if (id === "new-chat") {
      await state.startNewChat();
    } else if (id === "search-conversations") {
      await focusSidebar("search");
    } else if (id === "focus-navigation") {
      await focusSidebar("navigation");
    } else if (id === "toggle-context") {
      state.showContext = !state.showContext;
      await tick();
      document.querySelector<HTMLElement>("#context-panel-toggle")?.focus();
    } else {
      openSettings(invoker);
    }
  }

  /** Handles exact app-shell shortcuts without crossing an active modal boundary. */
  function handleWindowKeydown(event: KeyboardEvent): void {
    if (state.commandPalette.open || state.showSettings || state.firstRun.requiresSetup(state.providerSettings)) return;
    if (isCommandPaletteShortcut(event)) {
      event.preventDefault();
      openCommandPalette(event.target instanceof HTMLElement ? event.target : null);
      return;
    }
    const commandId = commandForKeyboardEvent(event);
    if (!commandId) return;
    event.preventDefault();
    const command = commandItems().find((item) => item.id === commandId);
    if (command?.disabledReason) {
      openCommandPalette(event.target instanceof HTMLElement ? event.target : null);
    } else {
      void runCommand(commandId);
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<svelte:head>
  <title>bottie — local-first conversations</title>
  <meta name="description" content="A private, provider-flexible conversation space with persistent memory." />
</svelte:head>

{#if state.recovery.status === null}
  <main class="storage-startup" aria-label="Checking local data">Checking local data…</main>
{:else if state.recovery.status.state === "recovery_required"}
  <StorageRecovery
    automaticBackupCount={state.recovery.status.automaticBackupCount}
    latestAutomaticBackupAtMs={state.recovery.status.latestAutomaticBackupAtMs}
    isRestoring={state.history.isRestoring}
    feedback={state.history.backupFeedback}
    failed={state.history.backupFailed}
    onrestoreautomatic={() => void recoverStorage("automatic")}
    onrestoremanual={() => void recoverStorage("manual")}
  />
{:else}
  <div class="app-shell">
    <div class="ambient ambient-one"></div>
    <div class="ambient ambient-two"></div>

    <Sidebar
      mobileOpen={state.showSidebar}
      runtimeVersion={state.runtime.version}
      conversations={state.history.conversations}
      activeConversationId={state.history.activeConversationId}
      storageError={state.history.storageError?.message ?? null}
      searchQuery={state.history.searchQuery}
      searchResults={state.history.searchResults}
      isSearching={state.history.isSearching}
      isGenerating={state.isGenerating || state.isPersistingMessage || state.history.isManaging}
      newChatShortcut={commandItems().find((item) => item.id === "new-chat")?.shortcut ?? "Ctrl N"}
      searchShortcut={commandItems().find((item) => item.id === "search-conversations")?.shortcut ?? "Ctrl ⇧ F"}
      onclose={() => void closeSidebar()}
      onnewchat={() => void state.startNewChat()}
      onselectconversation={(conversationId) => void state.openConversation(conversationId)}
      onsearch={(query) => void state.history.search(query)}
      onselectsearchresult={(result) => void state.openSearchResult(result)}
      onrenameconversation={(conversationId, title) => void state.history.rename(conversationId, title)}
      onarchiveconversation={(conversationId, archived) => {
        void state.history.setArchived(conversationId, archived).then((closed) => {
          if (closed) void state.startNewChat();
        });
      }}
      onmemoryexclusion={(conversationId, excluded) => void state.history.setMemoryExcluded(conversationId, excluded)}
      ondeleteconversation={(conversationId) => {
        void state.history.delete(conversationId).then((closed) => {
          if (closed) void state.startNewChat();
        });
      }}
      onrestoreconversation={(conversationId) => void state.history.restore(conversationId)}
      onforgetconversation={(conversationId) => void state.history.forget(conversationId)}
      onopensettings={(invoker) => openSettings(invoker)}
    />

    <main class="workspace">
      <ProviderToolbar
        providerId={state.selectedProviderId}
        selectedModelKey={state.selectedModelKey}
        models={state.models}
        providerStatus={state.providerStatus}
        isGenerating={state.isGenerating || state.isPersistingMessage || state.history.isManaging}
        reasoningEffort={state.reasoningEffort}
        showContext={state.showContext}
        isLocalRoute={state.isLocalRoute}
        webEnabled={state.web.enabled}
        webSearchProviderName={webSearchProviderName(state.providerSettings.webSearchProviderId)}
        canExport={Boolean(state.history.activeConversationId)}
        canBatchExport={canBatchExportConversations(state.history.conversations)}
        canBackup={state.runtime.version !== "preview"}
        canRestore={state.runtime.version !== "preview"}
        isExporting={state.history.isExporting}
        exportFeedback={state.history.exportFeedback}
        exportFailed={state.history.exportFailed}
        isBackingUp={state.history.isBackingUp}
        isRestoring={state.history.isRestoring}
        backupFeedback={state.history.backupFeedback}
        backupFailed={state.history.backupFailed}
        onproviderchange={(providerId) => void state.changeProvider(providerId)}
        onmodelchange={(modelKey) => void state.changeModel(modelKey)}
        ontogglereasoning={() => state.toggleReasoning()}
        onopensidebar={() => (state.showSidebar = true)}
        onopencommands={(invoker) => openCommandPalette(invoker)}
        commandPaletteShortcut={state.commandPalette.shortcutLabel()}
        ontogglecontext={() => (state.showContext = !state.showContext)}
        onexport={() => void state.history.exportMarkdown()}
        onexportjson={() => void state.history.exportJson()}
        onexportbatchjson={() => void state.history.exportBatchJson()}
        onbackup={() => void state.history.backup()}
        onrestore={() => {
          void state.history.restoreBackup().then((messages) => {
            if (messages) state.messages = messages;
          });
        }}
      />

      <ConversationView
        messages={state.messages}
        providerStatus={state.providerStatus}
        providerError={state.providerError}
        selectedModel={state.selectedModel}
        activeStage={state.activeStage}
        inferenceStages={inferenceStages(
          state.isLocalRoute,
          state.web.enabled,
          state.selectedModel?.providerName,
          webSearchProviderName(state.providerSettings.webSearchProviderId),
          state.reasoningEffort,
        )}
        isGenerating={state.isGenerating || state.isPersistingMessage || state.history.isManaging}
        canGenerate={state.canSend && state.conversationAttachmentsCanSubmit}
        branches={state.history.branches}
        currentBranchId={state.history.currentBranchId}
        onretry={() => void state.refreshModels()}
        onselectbranch={(branchId) => void state.selectConversationBranch(branchId)}
        oneditmessage={(message, text) => void state.editAndRegenerate(message, text)}
        onregenerate={(responseId) => void state.regenerateResponse(responseId)}
        onretryresponse={(responseId) => void state.regenerateResponse(responseId, true)}
        onrateresponse={(responseId, rating) => void state.history.rateResponse(state.messages, responseId, rating)}
        onremoveattachment={(messageId, attachmentId) => void state.removeMessageAttachment(messageId, attachmentId)}
        onscrollready={(element) => state.interaction.setMessageScroll(element)}
      />

      <Composer
        attachments={state.attachment.items}
        prompt={state.prompt}
        isGenerating={state.isGenerating}
        canCompose={state.canSend}
        canSend={state.canSend && !state.microphone.isActive && state.attachmentsCanSubmit}
        attachmentNote={composerAttachmentNote(
          nextRequestAttachments(state.attachment.items, state.history.conversationAttachments),
          state.selectedModel,
        )}
        providerStatus={state.providerStatus}
        memoryAvailable={state.memoryAvailable}
        memoryEnabled={state.memory.enabled}
        webAvailable={state.webAvailable}
        webEnabled={state.web.enabled}
        emailAvailable={state.emailAvailable}
        emailEnabled={state.email.enabled}
        emailBoundaryNote={state.emailBoundaryNote}
        emailUnavailableReason={state.emailUnavailableReason}
        microphoneStatus={state.microphone.status}
        microphoneAvailable={state.microphone.available}
        onprompt={(prompt) => (state.prompt = prompt)}
        oninput={() => state.interaction.resizeComposer()}
        onkeydown={(event) => state.interaction.handleKeydown(event, () => void state.sendMessage())}
        onsend={() => state.handleSendButton()}
        onadd={() => void state.attachment.openPicker()}
        onfiles={(event) => state.attachment.addBrowserFiles(event)}
        onremove={(id) => state.attachment.remove(id)}
        ontogglememory={() => void state.toggleTool("memory")}
        ontoggleweb={() => void state.toggleTool("web")}
        ontoggleemail={() => void state.toggleTool("email")}
        onstartmicrophone={() => void state.microphone.start()}
        onstopmicrophone={() => void state.microphone.stop()}
        ondiscardmicrophone={() => void state.microphone.discard()}
        oncorrectmicrophone={(turnIndex, text) => void state.microphone.correct(turnIndex, text)}
        oncomposerready={(element) => state.interaction.setComposer(element)}
        onattachmentinputready={(element) => state.attachment.setBrowserInput(element)}
      />
    </main>

    <ContextPanel
      open={state.showContext}
      attachments={state.attachment.items}
      conversationAttachments={state.history.conversationAttachments}
      messageAttachments={messageAttachmentAssociations(state.messages)}
      canKeepInConversation={Boolean(state.history.activeConversationId)}
      selectedModel={state.selectedModel}
      selectedProviderEndpoint={selectedProviderEndpoint(state.selectedProviderId, state.providerSettings)}
      providerStatus={state.providerStatus}
      isLocalRoute={state.isLocalRoute}
      webEnabled={state.web.enabled}
      webSearchProviderName={webSearchProviderName(state.providerSettings.webSearchProviderId)}
      isAddingAttachments={state.attachment.isIngesting}
      attachmentFeedback={state.attachment.feedback}
      attachmentFailed={state.attachment.failed}
      attachmentActionsDisabled={state.isGenerating || state.isPersistingMessage || state.history.isManaging}
      memoryCitations={state.memory.citations(state.messages)}
      webSources={state.web.sources(state.messages)}
      onclose={() => void closeContext()}
      onadd={() => void state.attachment.openPicker()}
      onremove={(id) => state.attachment.remove(id)}
      onkeep={(attachmentId) => void state.keepDraftAttachmentInConversation(attachmentId)}
      onremoveconversation={(attachmentId) => void state.removeConversationAttachment(attachmentId)}
      onremovemessage={(messageId, attachmentId) => void state.removeMessageAttachment(messageId, attachmentId)}
      onremovememory={(citationId) => state.memory.dismiss(citationId)}
      onremovewebsource={(sourceId) => state.web.dismiss(sourceId)}
    />

    {#if state.commandPalette.open}
      <CommandPalette
        items={commandItems()}
        onclose={() => void closeCommandPalette()}
        onrun={(id) => void runCommand(id)}
      />
    {/if}

    {#if state.firstRun.requiresSetup(state.providerSettings) && !state.showSettings}
      <FirstRunSetup
        providerName={state.selectedModel?.providerName ?? null}
        modelName={state.selectedModel?.displayName ?? null}
        providerEndpoint={selectedProviderEndpoint(state.selectedProviderId, state.providerSettings)}
        isLocalRoute={state.isLocalRoute}
        canComplete={state.canSend}
        isSaving={state.firstRun.isSaving}
        error={state.firstRun.error}
        onopensettings={() => openSettings(null)}
        oncomplete={() =>
          void state.firstRun.complete(state.canSend, (settings) => (state.providerSettings = settings))}
      />
    {/if}

    {#if state.showSettings}
      <ProviderSettingsDialog
        settings={state.providerSettings}
        appearance={state.appearance}
        isGenerating={state.isGenerating || state.isPersistingMessage || state.history.isManaging}
        onappearancechange={updateAppearance}
        onclose={() => void closeSettings()}
        onsaved={(settings) => state.applyProviderSettings(settings)}
      />
    {/if}
  </div>
{/if}
