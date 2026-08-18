<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import Icon from "$lib/Icon.svelte";

  type Message = {
    id: number;
    role: "user" | "assistant";
    content: string;
    featured?: boolean;
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

  const toolStages = [
    { icon: "brain" as const, label: "Searched memory", detail: "3 relevant conversations" },
    { icon: "file" as const, label: "Read attachments", detail: "2 context files" },
    { icon: "sparkles" as const, label: "Composed response", detail: "Local model" },
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
      content:
        "Absolutely. I’d build bottie as a sequence of small, complete slices—starting with the conversation experience, then connecting inference, persistence, and tools behind it.\n\nThe important boundary is simple: the WebView presents state; the Rust core owns secrets, files, storage, provider calls, and tool execution.",
    },
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
  let showContext = $state(true);
  let showSidebar = $state(false);
  let messageScroll: HTMLDivElement;
  let composer: HTMLTextAreaElement;
  let attachmentInput: HTMLInputElement;
  let runtime = $state<RuntimeInfo>({ name: "bottie", version: "preview", storage: "local" });

  const delay = (duration: number) => new Promise((resolve) => setTimeout(resolve, duration));

  onMount(async () => {
    if (isTauri()) {
      try {
        runtime = await invoke<RuntimeInfo>("app_info");
      } catch (error) {
        console.warn("Could not read the native runtime information", error);
      }
    }
  });

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
    if (!submittedPrompt || isGenerating) return;

    messages.push({ id: Date.now(), role: "user", content: submittedPrompt });
    prompt = "";
    resizeComposer();
    isGenerating = true;
    const run = ++generationRun;
    activeStage = 0;
    await scrollToBottom();

    for (let index = 0; index < toolStages.length; index += 1) {
      activeStage = index;
      await delay(index === 0 ? 520 : 440);
      if (run !== generationRun) return;
    }
    activeStage = toolStages.length;

    const response =
      "That fits the foundation we’re building. I’ve kept this first slice deliberately local and inspectable: the interface owns presentation state, while a typed Tauri command confirms the native Rust boundary.\n\nNext, I’d replace this simulated stream with a provider-neutral event stream, then connect oMLX as the first real adapter without changing the UI contract.";
    messages.push({ id: Date.now() + 1, role: "assistant", content: "" });
    const replyIndex = messages.length - 1;
    await scrollToBottom();

    const pieces = response.match(/\S+\s*/g) ?? [response];
    for (const piece of pieces) {
      if (run !== generationRun) return;
      messages[replyIndex].content += piece;
      await tick();
      messageScroll?.scrollTo({ top: messageScroll.scrollHeight, behavior: "auto" });
      await delay(22);
    }

    isGenerating = false;
    activeStage = -1;
  }

  function stopGenerating() {
    generationRun += 1;
    isGenerating = false;
    activeStage = -1;
  }

  function handleSendButton() {
    if (isGenerating) {
      stopGenerating();
    } else {
      void sendMessage();
    }
  }

  function startNewChat() {
    messages = [
      {
        id: Date.now(),
        role: "assistant",
        content: "Fresh thread, same memory. What would you like to explore?",
      },
    ];
    activeStage = -1;
    generationRun += 1;
    isGenerating = false;
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
      <button class="settings-button">
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

      <button class="model-picker" aria-label="Choose model">
        <span class="provider-pip"></span>
        <span class="model-copy">
          <strong>Qwen 3.5 35B</strong>
          <small>oMLX · local</small>
        </span>
        <Icon name="chevron-down" size={15} />
      </button>

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
      <div class="conversation-canvas">
        <div class="date-divider"><span>Today · 19:42</span></div>

        {#each messages as message (message.id)}
          <article class:assistant={message.role === "assistant"} class="message">
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
                  <span>Qwen 3.5 35B</span>
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
                  <span class="response-meta">1.4s · 284 tokens</span>
                </div>
              {/if}
            </div>
          </article>
        {/each}

        {#if activeStage >= 0}
          <div class="activity-card" aria-live="polite">
            <div class="activity-heading">
              <span class="activity-orbit"><span></span></span>
              <strong>{activeStage >= toolStages.length ? "Context ready" : "Working with context"}</strong>
            </div>
            <div class="activity-stages">
              {#each toolStages as stage, index}
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
          placeholder="Ask anything, or drop a file…"
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
            <button class="tool-toggle active" aria-label="Memory search enabled" aria-pressed="true">
              <Icon name="brain" size={17} />
              <span>Memory</span>
            </button>
            <button class="tool-toggle" aria-label="Web search disabled" aria-pressed="false">
              <Icon name="globe" size={17} />
              <span>Web</span>
            </button>
          </div>

          <button
            class="send-button"
            class:enabled={prompt.trim().length > 0 || isGenerating}
            disabled={!prompt.trim() && !isGenerating}
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
              <small>{attachment.size} · Indexed</small>
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
        <h3>Active memories <span>3</span></h3>
        <button>Manage</button>
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
          <span><strong>oMLX</strong><small>localhost:8000</small></span>
        </div>
        <div class="route-status"><span></span> Nothing leaves this device</div>
      </div>
    </section>

    <div class="context-footer">
      <span>Estimated context</span>
      <strong>8.4k <small>/ 64k tokens</small></strong>
      <div class="context-meter"><span></span></div>
    </div>
  </aside>
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
  .model-picker { display: flex; align-items: center; gap: 9px; padding: 6px 9px 6px 8px; border: 0; border-radius: 10px; background: transparent; cursor: pointer; }
  .model-picker:hover { background: rgba(255, 255, 255, 0.04); }
  .provider-pip { width: 8px; height: 8px; border-radius: 50%; background: var(--cyan); box-shadow: 0 0 10px rgba(91, 216, 200, 0.7); }
  .model-copy { display: flex; flex-direction: column; align-items: flex-start; line-height: 1.15; }
  .model-copy strong { font-size: 12px; font-weight: 610; }
  .model-copy small { margin-top: 3px; color: var(--muted); font-size: 9px; }
  .topbar-actions, .composer-tools, .source-row, .message-actions { display: flex; align-items: center; }
  .topbar-actions { gap: 6px; }
  .privacy-pill { display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 10px; margin-right: 5px; border: 1px solid rgba(91, 216, 200, 0.15); border-radius: 20px; background: rgba(91, 216, 200, 0.055); color: #8fded3; font-size: 9.5px; font-weight: 600; }
  .icon-button { display: grid; place-items: center; width: 32px; height: 32px; padding: 0; border: 0; border-radius: 8px; background: transparent; color: #888692; cursor: pointer; }
  .icon-button:hover, .icon-button.active { background: rgba(255, 255, 255, 0.05); color: #dbd8e1; }
  .mobile-menu { display: none; }

  .message-scroll { min-height: 0; overflow-x: hidden; overflow-y: auto; scrollbar-color: #2b2a34 transparent; scrollbar-width: thin; }
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
  .activity-stages { display: grid; grid-template-columns: repeat(3, 1fr); gap: 7px; }
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
  .composer-toolbar { display: flex; align-items: center; justify-content: space-between; padding: 4px 8px 8px; }
  .composer-tools { gap: 3px; }
  .composer-tools button { height: 29px; border-radius: 8px; }
  .composer-tools button:not(.tool-toggle) { width: 30px; }
  .tool-toggle { display: flex !important; grid: none !important; gap: 5px; padding: 0 8px !important; font-size: 9px; }
  .tool-toggle.active { background: rgba(143, 125, 247, 0.08); color: #9f92e7; }
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
  .context-footer { margin-top: auto; padding: 13px 18px 16px; background: rgba(0, 0, 0, 0.08); color: #6e6c76; font-size: 7.5px; }
  .context-footer strong { float: right; color: #a9a6ae; font-size: 8px; font-weight: 570; }
  .context-footer strong small { color: #5d5b65; font-size: 7px; font-weight: 450; }
  .context-meter { height: 3px; margin-top: 9px; overflow: hidden; border-radius: 4px; background: #25242d; }
  .context-meter span { display: block; width: 13%; height: 100%; border-radius: inherit; background: linear-gradient(90deg, var(--violet), var(--cyan)); }

  .mobile-scrim { display: none; }
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
    .model-picker { margin-right: auto; }
  }
  @media (max-width: 600px) {
    .privacy-pill, .tool-toggle span { display: none; }
    .conversation-canvas { width: calc(100% - 30px); padding-top: 23px; }
    .composer-zone { padding: 0 12px 9px; }
    .activity-stages { grid-template-columns: 1fr; }
    .context-panel { width: min(320px, 92vw); }
  }
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { scroll-behavior: auto !important; animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; transition-duration: 0.01ms !important; }
  }
</style>
