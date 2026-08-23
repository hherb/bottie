<script lang="ts">
  import type { AppearancePreferences, DensityPreference, ThemePreference } from "./appearance";

  type Props = {
    appearance: AppearancePreferences;
    onchange: (preferences: AppearancePreferences) => void;
  };

  /** Closed visible choices for the local presentation surface. */
  const THEMES: Array<{ value: ThemePreference; label: string; description: string }> = [
    { value: "system", label: "System", description: "Follow this device" },
    { value: "light", label: "Light", description: "Bright surfaces" },
    { value: "dark", label: "Dark", description: "Low-light surfaces" },
  ];
  const DENSITIES: Array<{ value: DensityPreference; label: string; description: string }> = [
    { value: "comfortable", label: "Comfortable", description: "More breathing room" },
    { value: "compact", label: "Compact", description: "More content on screen" },
  ];

  let { appearance, onchange }: Props = $props();

  /** Applies one theme choice while retaining the current density. */
  function chooseTheme(theme: ThemePreference): void {
    onchange({ ...appearance, theme });
  }

  /** Applies one density choice while retaining the current theme. */
  function chooseDensity(density: DensityPreference): void {
    onchange({ ...appearance, density });
  }
</script>

<section class="appearance-setting" aria-labelledby="appearance-title">
  <div class="appearance-heading">
    <span>
      <strong id="appearance-title">Appearance</strong>
      <small>Applied immediately and stored only on this device.</small>
    </span>
  </div>

  <fieldset aria-label="Theme">
    <legend>Theme</legend>
    <div class="appearance-options theme-options">
      {#each THEMES as option}
        <label class:active={appearance.theme === option.value}>
          <input
            type="radio"
            name="appearance-theme"
            value={option.value}
            checked={appearance.theme === option.value}
            onchange={() => chooseTheme(option.value)}
          />
          <span><strong>{option.label}</strong><small>{option.description}</small></span>
        </label>
      {/each}
    </div>
  </fieldset>

  <fieldset aria-label="Density">
    <legend>Density</legend>
    <div class="appearance-options density-options">
      {#each DENSITIES as option}
        <label class:active={appearance.density === option.value}>
          <input
            type="radio"
            name="appearance-density"
            value={option.value}
            checked={appearance.density === option.value}
            onchange={() => chooseDensity(option.value)}
          />
          <span><strong>{option.label}</strong><small>{option.description}</small></span>
        </label>
      {/each}
    </div>
  </fieldset>
</section>
