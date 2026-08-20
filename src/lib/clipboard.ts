/** Minimal clipboard surface needed by response-copying presentation code. */
export type ClipboardWriter = {
  writeText: (text: string) => Promise<void>;
};

/** Assistant material intentionally included in one clipboard document. */
export type CopyableAssistantResponse = {
  content: string;
  reasoning?: string;
};

/** Markdown headings that distinguish reasoning from the final response when both are copied. */
const REASONING_HEADING = "## Reasoning";
const RESPONSE_HEADING = "## Response";

/** Builds clipboard Markdown while leaving answers without reasoning byte-for-byte unchanged. */
export function assistantResponseMarkdown(response: CopyableAssistantResponse): string {
  if (!response.reasoning) return response.content;
  return `${REASONING_HEADING}\n\n${response.reasoning}\n\n${RESPONSE_HEADING}\n\n${response.content}`;
}

/** Copies one assistant response document, returning whether the clipboard write succeeded. */
export async function copyAssistantResponse(
  response: CopyableAssistantResponse,
  clipboard: ClipboardWriter | undefined = globalThis.navigator?.clipboard,
): Promise<boolean> {
  if (!clipboard) return false;

  try {
    await clipboard.writeText(assistantResponseMarkdown(response));
    return true;
  } catch {
    return false;
  }
}
