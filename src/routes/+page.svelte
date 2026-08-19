<script lang="ts">
  import { onMount } from "svelte";

  import Composer from "$lib/Composer.svelte";
  import ContextPanel from "$lib/ContextPanel.svelte";
  import ConversationView from "$lib/ConversationView.svelte";
  import ProviderSettingsDialog from "$lib/ProviderSettingsDialog.svelte";
  import ProviderToolbar from "$lib/ProviderToolbar.svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import "$lib/styles/shell.css";
  import "$lib/styles/conversation.css";
  import "$lib/styles/composer.css";
  import "$lib/styles/context.css";
  import "$lib/styles/settings.css";

  import { PageState } from "./page-state.svelte";

  const state = new PageState();

  onMount(() => state.initialize());
</script>

<svelte:head>
  <title>bottie — local-first conversations</title>
  <meta name="description" content="A private, provider-flexible conversation space with persistent memory." />
</svelte:head>

<div class="app-shell">
  <div class="ambient ambient-one"></div>
  <div class="ambient ambient-two"></div>

  <Sidebar
    mobileOpen={state.showSidebar}
    runtimeVersion={state.runtime.version}
    onclose={() => (state.showSidebar = false)}
    onnewchat={() => state.startNewChat()}
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
      isGenerating={state.isGenerating}
      reasoningEffort={state.reasoningEffort}
      showContext={state.showContext}
      onproviderchange={(providerId) => void state.changeProvider(providerId)}
      onmodelchange={(modelKey) => void state.changeModel(modelKey)}
      ontogglereasoning={() => state.toggleReasoning()}
      onopensidebar={() => (state.showSidebar = true)}
      ontogglecontext={() => (state.showContext = !state.showContext)}
    />

    <ConversationView
      messages={state.messages}
      providerStatus={state.providerStatus}
      providerError={state.providerError}
      selectedModel={state.selectedModel}
      activeStage={state.activeStage}
      inferenceStages={state.inferenceStages}
      isGenerating={state.isGenerating}
      onretry={() => void state.refreshModels()}
      onscrollready={(element) => state.setMessageScroll(element)}
    />

    <Composer
      attachments={state.attachments}
      prompt={state.prompt}
      isGenerating={state.isGenerating}
      canSend={state.canSend}
      providerStatus={state.providerStatus}
      onprompt={(prompt) => (state.prompt = prompt)}
      oninput={() => state.resizeComposer()}
      onkeydown={(event) => state.handleComposerKeydown(event)}
      onsend={() => state.handleSendButton()}
      onfiles={(event) => state.addAttachments(event)}
      onremove={(id) => state.removeAttachment(id)}
      oncomposerready={(element) => state.setComposer(element)}
      onattachmentinputready={(element) => state.setAttachmentInput(element)}
    />
  </main>

  <ContextPanel
    open={state.showContext}
    attachments={state.attachments}
    selectedModel={state.selectedModel}
    selectedProviderEndpoint={state.selectedProviderEndpoint}
    providerStatus={state.providerStatus}
    onclose={() => (state.showContext = false)}
    onadd={() => state.openAttachmentPicker()}
    onremove={(id) => state.removeAttachment(id)}
  />

  {#if state.showSettings}
    <ProviderSettingsDialog
      settings={state.providerSettings}
      isGenerating={state.isGenerating}
      onclose={() => (state.showSettings = false)}
      onsaved={(settings) => state.applyProviderSettings(settings)}
    />
  {/if}
</div>
