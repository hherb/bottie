/** Formats one bounded native interval without presenting sub-millisecond work as zero. */
export function formatNativeLatency(milliseconds: number): string {
  if (milliseconds < 1) return "<1 ms";
  if (milliseconds < 1_000) return `${milliseconds} ms`;
  return `${(milliseconds / 1_000)
    .toFixed(2)
    .replace(/\.00$/, "")
    .replace(/(\.\d)0$/, "$1")} s`;
}
