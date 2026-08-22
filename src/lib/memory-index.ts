/** Path-free frontend contract for Bottie's native semantic memory index. */

import { invoke, isTauri } from "@tauri-apps/api/core";

/** Durable native semantic-index phases exposed without model or filesystem details. */
export type SemanticIndexState = "pending" | "loading_model" | "indexing" | "ready" | "failed";

/** Durable path-free progress for the built-in local semantic index. */
export type SemanticIndexProgress = {
  state: SemanticIndexState;
  completedChunks: number;
  totalChunks: number;
  errorCode: string | null;
};

const FAILURE_LABELS: Record<string, string> = {
  embedding_count: "embedding output invalid",
  embedding_dimensions: "embedding output invalid",
  embedding_runtime: "embedding runtime unavailable",
  embedding_values: "embedding output invalid",
  model_cache: "local model unavailable",
  model_runtime: "local model unavailable",
};

/** Reads durable semantic-index progress from the Rust-owned store. */
export async function getSemanticIndexProgress(): Promise<SemanticIndexProgress> {
  if (!isTauri()) throw new Error("Semantic indexing is available only in the native Bottie app.");
  return invoke<SemanticIndexProgress>("get_semantic_index_progress");
}

/** Clears only derived vectors and starts one restore-safe native rebuild. */
export async function reindexSemanticMemory(): Promise<SemanticIndexProgress> {
  if (!isTauri()) throw new Error("Semantic indexing is available only in the native Bottie app.");
  return invoke<SemanticIndexProgress>("reindex_semantic_memory");
}

/** Reports whether automatic status refresh should continue. */
export function memoryIndexIsActive(progress: SemanticIndexProgress): boolean {
  return progress.state === "pending" || progress.state === "loading_model" || progress.state === "indexing";
}

/** Converts durable counts into a defensive bounded percentage. */
export function memoryIndexPercent(progress: SemanticIndexProgress): number {
  if (progress.totalChunks === 0) return 100;
  return Math.max(0, Math.min(100, Math.round((progress.completedChunks / progress.totalChunks) * 100)));
}

/** Formats path-free durable progress for the settings surface. */
export function memoryIndexCopy(progress: SemanticIndexProgress): string {
  if (progress.state === "ready" && progress.totalChunks === 0) return "Ready · no eligible chunks";
  const counts = `${progress.completedChunks} of ${progress.totalChunks} chunks`;
  if (progress.state === "pending") return `Waiting to index · ${counts}`;
  if (progress.state === "loading_model") return `Preparing local model · ${counts}`;
  if (progress.state === "indexing") return `Indexing · ${counts}`;
  if (progress.state === "failed") {
    const failure = FAILURE_LABELS[progress.errorCode ?? ""] ?? "local indexing failed";
    return `Paused · ${counts} · ${failure}`;
  }
  return `Ready · ${counts}`;
}
