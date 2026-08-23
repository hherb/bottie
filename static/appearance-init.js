/** Applies validated local appearance state before app styles and Svelte can paint. */
(function initializeAppearance() {
  const fallback = { theme: "dark", density: "comfortable" };
  let saved = fallback;
  try {
    const parsed = JSON.parse(localStorage.getItem("bottie.appearance.v1") || "null");
    if (parsed && typeof parsed === "object") {
      saved = {
        theme: ["system", "light", "dark"].includes(parsed.theme) ? parsed.theme : fallback.theme,
        density: ["comfortable", "compact"].includes(parsed.density) ? parsed.density : fallback.density,
      };
    }
  } catch {
    saved = fallback;
  }
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const resolvedTheme = saved.theme === "system" ? (systemDark ? "dark" : "light") : saved.theme;
  const root = document.documentElement;
  root.dataset.theme = resolvedTheme;
  root.dataset.themePreference = saved.theme;
  root.dataset.density = saved.density;
  root.style.colorScheme = resolvedTheme;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", resolvedTheme === "dark" ? "#0a0a0e" : "#f1efe9");
})();
