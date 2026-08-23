<script lang="ts">
  import { onMount } from "svelte";

  import Composer from "$lib/Composer.svelte";
  import ContextPanel from "$lib/ContextPanel.svelte";
  import ConversationView from "$lib/ConversationView.svelte";
  import FirstRunSetup from "$lib/FirstRunSetup.svelte";
  import ProviderSettingsDialog from "$lib/ProviderSettingsDialog.svelte";
  import ProviderToolbar from "$lib/ProviderToolbar.svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import StorageRecovery from "$lib/StorageRecovery.svelte";
  import { composerAttachmentNote } from "$lib/chat";
  import { webSearchProviderName } from "$lib/presentation";
  import { canBatchExportConversations } from "$lib/storage";
  import "$lib/styles/shell.css";
  import "$lib/styles/conversation-nav.css";
  import "$lib/styles/conversation.css";
  import "$lib/styles/message-attachments.css";
  import "$lib/styles/tool-activity.css";
  import "$lib/styles/markdown.css";
  import "$lib/styles/composer.css";
  import "$lib/styles/context.css";
  import "$lib/styles/first-run.css";
  import "$lib/styles/settings.css";
  import "$lib/styles/diagnostics.css";
  import "$lib/styles/localmail-settings.css";
  import "$lib/styles/recovery.css";

  import { PageState } from "./page-state.svelte";
  import {
    inferenceStages,
    messageAttachmentAssociations,
    nextRequestAttachments,
    selectedProviderEndpoint,
  } from "./page-presentation";

  const state = new PageState();

  onMount(() => {
    void state.initialize();
    return () => state.dispose();
  });

  /** Completes guided recovery before returning to normal conversation and provider initialization. */
  async function recoverStorage(source: "manual" | "automatic"): Promise<void> {
    const messages = await state.recovery.restore(state.history, source);
    if (messages === null) return;
    state.messages = messages;
    await state.refreshModels();
  }
</script>

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
      onclose={() => (state.showSidebar = false)}
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
      onopensettings={() => {
        state.showSettings = true;
        state.showSidebar = false;
      }}
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
        canSend={state.canSend && state.attachmentsCanSubmit}
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
        emailUnavailableReason={state.emailUnavailableReason}
        onprompt={(prompt) => (state.prompt = prompt)}
        oninput={() => state.interaction.resizeComposer()}
        onkeydown={(event) => state.interaction.handleKeydown(event, () => void state.sendMessage())}
        onsend={() => state.handleSendButton()}
        onadd={() => void state.attachment.openPicker()}
        onfiles={(event) => state.attachment.addBrowserFiles(event)}
        onremove={(id) => state.attachment.remove(id)}
        ontogglememory={() => state.memory.toggle(state.memoryAvailable, state.isGenerating)}
        ontoggleweb={() => state.web.toggle(state.webAvailable, state.isGenerating)}
        ontoggleemail={() => state.email.toggle(state.emailAvailable, state.isGenerating)}
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
      onclose={() => (state.showContext = false)}
      onadd={() => void state.attachment.openPicker()}
      onremove={(id) => state.attachment.remove(id)}
      onkeep={(attachmentId) => void state.keepDraftAttachmentInConversation(attachmentId)}
      onremoveconversation={(attachmentId) => void state.removeConversationAttachment(attachmentId)}
      onremovemessage={(messageId, attachmentId) => void state.removeMessageAttachment(messageId, attachmentId)}
      onremovememory={(citationId) => state.memory.dismiss(citationId)}
      onremovewebsource={(sourceId) => state.web.dismiss(sourceId)}
    />

    {#if state.firstRun.requiresSetup(state.providerSettings) && !state.showSettings}
      <FirstRunSetup
        providerName={state.selectedModel?.providerName ?? null}
        modelName={state.selectedModel?.displayName ?? null}
        providerEndpoint={selectedProviderEndpoint(state.selectedProviderId, state.providerSettings)}
        isLocalRoute={state.isLocalRoute}
        canComplete={state.canSend}
        isSaving={state.firstRun.isSaving}
        error={state.firstRun.error}
        onopensettings={() => (state.showSettings = true)}
        oncomplete={() =>
          void state.firstRun.complete(state.canSend, (settings) => (state.providerSettings = settings))}
      />
    {/if}

    {#if state.showSettings}
      <ProviderSettingsDialog
        settings={state.providerSettings}
        isGenerating={state.isGenerating || state.isPersistingMessage || state.history.isManaging}
        onclose={() => {
          state.showSettings = false;
          void state.email.refresh();
        }}
        onsaved={(settings) => state.applyProviderSettings(settings)}
      />
    {/if}
  </div>
{/if}
