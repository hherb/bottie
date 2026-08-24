/** Development-only browser fixture wiring for reproducible long-history scrolling measurements. */

import { performanceConversations, performanceMessages } from "$lib/performance-fixtures";

import type { PageState } from "./page-state.svelte";

const PERFORMANCE_PREVIEW_VALUE = "long-history";

/** Reports whether a query explicitly requests Bottie's long-history performance fixture. */
export function performancePreviewRequested(search: string): boolean {
  return new URLSearchParams(search).get("performance") === PERFORMANCE_PREVIEW_VALUE;
}

/** Applies deterministic path-free fixtures to the disconnected browser preview only. */
export function applyPerformancePreview(state: PageState, search: string): boolean {
  if (!performancePreviewRequested(search)) return false;
  state.messages = performanceMessages();
  state.history.conversations = performanceConversations();
  return true;
}
