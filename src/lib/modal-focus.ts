/** Shared keyboard-focus helpers for WebView-owned modal surfaces. */

const MODAL_CONTROL_SELECTOR = [
  "button:not([disabled]):not([tabindex='-1'])",
  "input:not([disabled]):not([tabindex='-1'])",
  "textarea:not([disabled]):not([tabindex='-1'])",
  "select:not([disabled]):not([tabindex='-1'])",
  "a[href]:not([tabindex='-1'])",
].join(", ");

/** Returns enabled controls that participate in one modal's Tab order. */
export function modalControls(container: HTMLElement | undefined): HTMLElement[] {
  return Array.from(container?.querySelectorAll<HTMLElement>(MODAL_CONTROL_SELECTOR) ?? []);
}

/** Focuses the first enabled control in one newly mounted modal. */
export function focusFirstModalControl(container: HTMLElement | undefined): void {
  modalControls(container)[0]?.focus();
}

/** Wraps Tab or Shift+Tab at one modal boundary and reports whether it handled the event. */
export function trapModalFocus(event: KeyboardEvent, container: HTMLElement | undefined): boolean {
  if (event.key !== "Tab") return false;
  const controls = modalControls(container);
  if (controls.length === 0) return false;
  const first = controls[0];
  const last = controls[controls.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
    return true;
  }
  if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
    return true;
  }
  return false;
}
