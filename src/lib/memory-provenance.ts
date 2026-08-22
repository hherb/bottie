/** Pure path-free presentation of successful native memory-tool provenance. */

import type { Message } from "./presentation";
import type { StoredToolInvocation } from "./storage";

/** One removable citation shown in the active conversation's Context panel. */
export type MemoryCitation = {
  id: string;
  kind: "conversation" | "attachment";
  label: "Conversation memory" | "Attached file";
  title: string;
  excerpt: string;
  createdAtMs: number;
};

type UnknownRecord = Record<string, unknown>;

/** Derives unique visible citations from successful selected-lineage native tool results. */
export function memoryCitationsForMessages(
  messages: Message[],
  dismissedIds: ReadonlySet<string> = new Set(),
): MemoryCitation[] {
  const citations = new Map<string, MemoryCitation>();
  for (const message of messages.toReversed()) {
    for (const tool of message.toolInvocations ?? []) {
      for (const citation of citationsForTool(tool)) {
        if (!dismissedIds.has(citation.id) && !citations.has(citation.id)) {
          citations.set(citation.id, citation);
        }
      }
    }
  }
  return [...citations.values()];
}

/** Extracts citations only from a completed, successful native execution envelope. */
function citationsForTool(tool: StoredToolInvocation): MemoryCitation[] {
  const result = successfulResult(tool);
  if (!result) return [];
  if (tool.toolName === "search_memory") return conversationSearchCitations(result);
  if (tool.toolName === "open_memory") {
    const citation = openMemoryCitation(result);
    return citation ? [citation] : [];
  }
  if (tool.toolName === "search_attached_files") return attachmentSearchCitations(result);
  return [];
}

/** Returns the structured result inside an exact successful dispatcher envelope. */
function successfulResult(tool: StoredToolInvocation): UnknownRecord | null {
  if (!tool.result || tool.result.isError || !isRecord(tool.result.output)) return null;
  if (tool.result.output.ok !== true || !isRecord(tool.result.output.result)) return null;
  return tool.result.output.result;
}

/** Parses ranked conversation-message matches without reflecting unsupported fields. */
function conversationSearchCitations(result: UnknownRecord): MemoryCitation[] {
  if (!Array.isArray(result.matches)) return [];
  return result.matches.flatMap((match) => {
    if (!isRecord(match) || !isRecord(match.provenance)) return [];
    const excerpt = requiredString(match.excerpt);
    const provenance = match.provenance;
    const conversationId = requiredString(provenance.conversationId);
    const messageId = requiredString(provenance.messageId);
    const title = requiredString(provenance.conversationTitle);
    const createdAtMs = timestamp(provenance.createdAtMs);
    if (
      provenance.sourceKind !== "message" ||
      !conversationId ||
      !messageId ||
      !title ||
      !excerpt ||
      createdAtMs === null
    ) {
      return [];
    }
    return [conversationCitation(conversationId, messageId, title, excerpt, createdAtMs)];
  });
}

/** Parses exact open-memory provenance and its single matched retained turn. */
function openMemoryCitation(result: UnknownRecord): MemoryCitation | null {
  if (!isRecord(result.provenance) || !Array.isArray(result.turns)) return null;
  const provenance = result.provenance;
  const conversationId = requiredString(provenance.conversationId);
  const messageId = requiredString(provenance.messageId);
  const title = requiredString(provenance.conversationTitle);
  if (provenance.sourceKind !== "message" || !conversationId || !messageId || !title) return null;
  const matchedTurn = result.turns.find(
    (turn) => isRecord(turn) && turn.isMatch === true && turn.messageId === messageId,
  );
  if (!isRecord(matchedTurn)) return null;
  const excerpt = requiredString(matchedTurn.text);
  const createdAtMs = timestamp(matchedTurn.createdAtMs);
  if (!excerpt || createdAtMs === null) return null;
  return conversationCitation(conversationId, messageId, title, excerpt, createdAtMs);
}

/** Builds one conversation-memory card without retaining raw tool payload fields. */
function conversationCitation(
  conversationId: string,
  messageId: string,
  title: string,
  excerpt: string,
  createdAtMs: number,
): MemoryCitation {
  return {
    id: `message:${conversationId}:${messageId}`,
    kind: "conversation",
    label: "Conversation memory",
    title,
    excerpt,
    createdAtMs,
  };
}

/** Parses retained-file matches into inert cards containing only native safe-leaf metadata. */
function attachmentSearchCitations(result: UnknownRecord): MemoryCitation[] {
  if (!Array.isArray(result.matches)) return [];
  return result.matches.flatMap((match) => {
    if (!isRecord(match) || !isRecord(match.provenance)) return [];
    const excerpt = requiredString(match.excerpt);
    const provenance = match.provenance;
    const attachmentId = requiredString(provenance.attachmentId);
    const title = requiredString(provenance.displayName);
    const createdAtMs = timestamp(provenance.createdAtMs);
    if (provenance.sourceKind !== "attachment" || !attachmentId || !title || !excerpt || createdAtMs === null) {
      return [];
    }
    return [
      {
        id: `attachment:${attachmentId}`,
        kind: "attachment" as const,
        label: "Attached file" as const,
        title,
        excerpt,
        createdAtMs,
      },
    ];
  });
}

/** Narrows unknown JSON into a non-array object. */
function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Accepts only non-empty native strings for visible presentation. */
function requiredString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

/** Accepts only finite non-negative Unix-millisecond values. */
function timestamp(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}
