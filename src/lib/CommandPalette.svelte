<script lang="ts">
  import { onMount } from "svelte";

  import {
    filterCommandPaletteItems,
    nextEnabledCommandIndex,
    type CommandId,
    type CommandPaletteItem,
  } from "./command-palette";

  type Props = {
    items: CommandPaletteItem[];
    onclose: () => void;
    onrun: (id: CommandId) => void;
  };

  let { items, onclose, onrun }: Props = $props();
  let query = $state("");
  let activeIndex = $state(-1);
  let layer = $state<HTMLDivElement>();
  let searchInput = $state<HTMLInputElement>();
  let filteredItems = $derived(filterCommandPaletteItems(items, query));

  onMount(() => {
    activeIndex = nextEnabledCommandIndex(filteredItems, -1, 1);
    searchInput?.focus();
  });

  /** Resets keyboard selection after the local filter changes. */
  function handleQuery(value: string): void {
    query = value;
    activeIndex = nextEnabledCommandIndex(filterCommandPaletteItems(items, value), -1, 1);
  }

  /** Runs one enabled command selected by pointer or keyboard. */
  function run(item: CommandPaletteItem): void {
    if (!item.disabledReason) onrun(item.id);
  }

  /** Keeps palette navigation and dismissal predictable from every child control. */
  function handleWindowKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      activeIndex = nextEnabledCommandIndex(filteredItems, activeIndex, event.key === "ArrowDown" ? 1 : -1);
      focusActiveOption();
      return;
    }
    if (event.key === "Enter" && event.target === searchInput && activeIndex >= 0) {
      event.preventDefault();
      run(filteredItems[activeIndex]);
      return;
    }
    if (event.key === "Tab") trapFocus(event);
  }

  /** Scrolls the active option into view without moving focus away from the search field. */
  function focusActiveOption(): void {
    searchInput?.focus();
    layer?.querySelector<HTMLElement>(`[data-command-index="${activeIndex}"]`)?.scrollIntoView({ block: "nearest" });
  }

  /** Retains Tab focus within the modal palette. */
  function trapFocus(event: KeyboardEvent): void {
    const controls = Array.from(layer?.querySelectorAll<HTMLElement>("input, button:not([disabled])") ?? []);
    if (controls.length === 0) return;
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<div class="command-layer" bind:this={layer}>
  <button class="command-scrim" aria-label="Close command palette" tabindex="-1" onclick={onclose}></button>
  <div class="command-dialog" role="dialog" aria-modal="true" aria-labelledby="command-palette-title">
    <h2 id="command-palette-title" class="visually-hidden">Command palette</h2>
    <label class="command-search">
      <span class="visually-hidden">Search commands</span>
      <input
        bind:this={searchInput}
        aria-label="Search commands"
        aria-activedescendant={activeIndex >= 0 ? `command-option-${filteredItems[activeIndex]?.id}` : undefined}
        aria-controls="command-list"
        aria-expanded="true"
        autocomplete="off"
        placeholder="Type a command…"
        role="combobox"
        value={query}
        oninput={(event) => handleQuery(event.currentTarget.value)}
      />
      <kbd>Esc</kbd>
    </label>
    <div id="command-list" class="command-list" role="listbox" aria-label="Available commands">
      {#each filteredItems as item, index (item.id)}
        <button
          class:active={index === activeIndex}
          class="command-option"
          id={`command-option-${item.id}`}
          role="option"
          aria-selected={index === activeIndex}
          aria-disabled={Boolean(item.disabledReason)}
          disabled={Boolean(item.disabledReason)}
          data-command-index={index}
          onclick={() => run(item)}
          onmousemove={() => {
            if (!item.disabledReason) activeIndex = index;
          }}
        >
          <span class="command-copy">
            <strong>{item.label}</strong>
            <small>{item.disabledReason ?? item.description}</small>
          </span>
          <kbd>{item.shortcut}</kbd>
        </button>
      {:else}
        <p class="command-empty" role="status">No commands match “{query.trim()}”.</p>
      {/each}
    </div>
    <footer class="command-footer">
      <span><kbd>↑</kbd><kbd>↓</kbd> Navigate</span>
      <span><kbd>Enter</kbd> Run</span>
      <span>Local UI actions only</span>
    </footer>
  </div>
</div>
