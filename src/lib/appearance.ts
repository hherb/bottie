/** Local presentation preference normalization, persistence, and system-theme coordination. */

/** Versioned local-storage key shared with the pre-paint initializer. */
export const APPEARANCE_STORAGE_KEY = "bottie.appearance.v1";

/** User-selectable color routing. */
export type ThemePreference = "system" | "light" | "dark";

/** User-selectable interface spacing. */
export type DensityPreference = "comfortable" | "compact";

/** Closed local presentation preference shape. */
export type AppearancePreferences = {
  theme: ThemePreference;
  density: DensityPreference;
};

/** Migration-safe presentation for installs without a valid saved preference. */
export const DEFAULT_APPEARANCE: AppearancePreferences = {
  theme: "dark",
  density: "comfortable",
};

/** Minimal storage contract used by browser local storage and focused tests. */
type AppearanceStorage = Pick<Storage, "getItem" | "setItem">;

/** Minimal document-root contract required to apply presentation state. */
type AppearanceRoot = {
  setAttribute(name: string, value: string): void;
  style: { colorScheme: string };
  ownerDocument?: { querySelector(selector: string): { setAttribute(name: string, value: string): void } | null };
};

/** Minimal system-color query contract required by the runtime controller. */
type SystemThemeQuery = {
  matches: boolean;
  addEventListener(name: "change", listener: () => void): void;
  removeEventListener(name: "change", listener: () => void): void;
};

/** Dependencies kept explicit so presentation behavior stays deterministic and testable. */
type AppearanceControllerOptions = {
  root: AppearanceRoot;
  storage: AppearanceStorage;
  mediaQuery: SystemThemeQuery;
  onChange: (preferences: AppearancePreferences) => void;
};

/** Runtime handle for updating preferences and releasing the conditional system listener. */
export type AppearanceController = {
  update(preferences: AppearancePreferences): void;
  dispose(): void;
};

/** Normalizes unknown persisted input into the closed preference shape. */
export function normalizeAppearance(value: unknown): AppearancePreferences {
  if (typeof value !== "object" || value === null) return { ...DEFAULT_APPEARANCE };
  const candidate = value as Record<string, unknown>;
  const validTheme = candidate.theme === "system" || candidate.theme === "light" || candidate.theme === "dark";
  const validDensity = candidate.density === "comfortable" || candidate.density === "compact";
  return {
    theme: validTheme ? (candidate.theme as ThemePreference) : DEFAULT_APPEARANCE.theme,
    density: validDensity ? (candidate.density as DensityPreference) : DEFAULT_APPEARANCE.density,
  };
}

/** Resolves an explicit document color scheme from a preference and current system state. */
export function resolveTheme(theme: ThemePreference, systemPrefersDark: boolean): "light" | "dark" {
  if (theme === "system") return systemPrefersDark ? "dark" : "light";
  return theme;
}

/** Reads a saved preference without allowing storage or parse failures to affect startup. */
export function readAppearance(storage: AppearanceStorage): AppearancePreferences {
  try {
    const value = storage.getItem(APPEARANCE_STORAGE_KEY);
    return value === null ? { ...DEFAULT_APPEARANCE } : normalizeAppearance(JSON.parse(value));
  } catch {
    return { ...DEFAULT_APPEARANCE };
  }
}

/** Applies the normalized preference as explicit document-level presentation state. */
export function applyAppearance(
  root: AppearanceRoot,
  preferences: AppearancePreferences,
  systemPrefersDark: boolean,
): void {
  const resolved = resolveTheme(preferences.theme, systemPrefersDark);
  root.setAttribute("data-theme", resolved);
  root.setAttribute("data-theme-preference", preferences.theme);
  root.setAttribute("data-density", preferences.density);
  root.style.colorScheme = resolved;
  root.ownerDocument
    ?.querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", resolved === "dark" ? "#0a0a0e" : "#f1efe9");
}

/** Creates the local presentation controller and conditionally tracks system-theme changes. */
export function createAppearanceController(options: AppearanceControllerOptions): AppearanceController {
  let preferences = readAppearance(options.storage);
  let listeningToSystem = false;

  /** Reapplies the current preference after a relevant system color change. */
  const handleSystemChange = (): void => {
    applyAppearance(options.root, preferences, options.mediaQuery.matches);
  };

  /** Keeps the media-query subscription scoped strictly to the System selection. */
  const syncSystemListener = (): void => {
    const shouldListen = preferences.theme === "system";
    if (shouldListen && !listeningToSystem) {
      options.mediaQuery.addEventListener("change", handleSystemChange);
      listeningToSystem = true;
    } else if (!shouldListen && listeningToSystem) {
      options.mediaQuery.removeEventListener("change", handleSystemChange);
      listeningToSystem = false;
    }
  };

  /** Applies state, updates reactive presentation controls, and adjusts the system listener. */
  const applyCurrent = (): void => {
    applyAppearance(options.root, preferences, options.mediaQuery.matches);
    options.onChange({ ...preferences });
    syncSystemListener();
  };

  applyCurrent();
  return {
    update(nextPreferences) {
      preferences = normalizeAppearance(nextPreferences);
      try {
        options.storage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(preferences));
      } catch {
        // A denied or full local store must not prevent a session-only appearance change.
      }
      applyCurrent();
    },
    dispose() {
      if (!listeningToSystem) return;
      options.mediaQuery.removeEventListener("change", handleSystemChange);
      listeningToSystem = false;
    },
  };
}
