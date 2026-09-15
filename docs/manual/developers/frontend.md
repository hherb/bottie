# Frontend development

[Back to the manual index](index.md)

## TypeScript from C#

TypeScript types disappear at runtime. An `interface` checks compilation but does not validate JSON. Rust remains the
authoritative validator. `type A = B | C` is a discriminated union; `Promise<T>` resembles `Task<T>`; `T | null`
resembles nullable references. Always `await` promises or deliberately mark fire-and-forget work with `void`.

Prefer string unions to loosely related booleans, `import type` for type-only dependencies, and pure functions without
hidden object/array mutation.

## Svelte 5 conventions

`.svelte` files combine typed script and markup. `.svelte.ts` files can contain reactive controllers. Follow a nearby
Bottie feature rather than older Svelte tutorials.

- Keep calculations in plain TypeScript and props/callbacks small and typed.
- Derive display state instead of maintaining duplicate flags.
- Use `onMount` only for lifecycle setup and return cleanup for listeners/controllers.
- Use `tick()` before focusing DOM changed by reactive state.
- Preserve focus around dialogs, panels, and responsive navigation.
- Never render provider/user content as raw HTML.
- Put cohesive new state in a feature controller, not another unrelated block in the large page shell.

## Calling Rust

```ts
import { invoke } from "@tauri-apps/api/core";

export interface WidgetStatus {
  state: "idle" | "busy" | "failed";
  message: string | null;
}

/** Returns path-free native widget status. */
export function getWidgetStatus(): Promise<WidgetStatus> {
  return invoke<WidgetStatus>("get_widget_status");
}
```

Rust command names are snake_case. JavaScript argument keys and serialized result casing must match neighboring
commands and Rust Serde attributes. The generic type does not validate runtime data.

Command adapter checklist:

1. Keep the DTO/wrapper beside the feature and use fixed unions.
2. Exclude paths, bytes, credentials, raw native IDs/hashes, and provider internals.
3. Handle rejection where retry/stale-result policy is known.
4. Compare conversation/run/session identity before applying late results.
5. Test the adapter/controller and native serialization/validation.
6. For streams, use the established channel pattern and explicit run/terminal state; silence is not completion.

Local storage is for presentation-only preferences such as appearance, not durable product state or secrets.

## Content, CSS, and accessibility

Provider/user Markdown goes through `src/lib/markdown.ts`; preserve its allowlist and URL policy. Attachments/generated
assets use validated custom protocols rather than filesystem URLs. Spoken text derives from safe visible tokens.

Feature CSS lives in `src/lib/styles/`. Reuse existing custom properties, density rules, focus treatment, and responsive
patterns. For every interaction:

- use semantic controls/headings/lists/dialog/status regions;
- name icon-only controls and associate input labels/errors;
- keep keyboard access and visible focus;
- restore focus when closing overlays;
- do not encode state by color alone;
- inspect light/dark/system, compact/comfortable, desktop, and a relevant narrow viewport.

Meaningful visual changes require screenshots/inspection. Native-only states may need deterministic development preview
fixtures, but those fixtures must remain development-only.

## Testing

Colocated `*.test.ts` files test helpers/controllers; component tests exercise rendered semantics and events.
`scripts/accessibility-presentation.test.mjs` provides broader presentation contracts. Mock the thin native adapter, not
every helper. Cover rejection, retry, cancellation, stale results, missing capabilities, empty input, and exact/over
limits.

```sh
npx vitest run src/lib/Composer.test.ts
```

Then run the complete frontend checks in [Testing and change workflow](testing-and-workflow.md).
