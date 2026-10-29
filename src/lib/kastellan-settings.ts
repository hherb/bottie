/** Pure presentation rules for the Kastellan section of Settings. */

import { KASTELLAN_DEFAULT_HOST, type KastellanSettings } from "./kastellan";

/** Whether the drafts differ from the saved settings, treating blank and the defaults as equal. */
export function kastellanDraftChanged(saved: KastellanSettings | null, host: string, cliPath: string): boolean {
  const draftHost = host.trim() || KASTELLAN_DEFAULT_HOST;
  const draftCliPath = cliPath.trim();
  const savedHost = saved?.host ?? KASTELLAN_DEFAULT_HOST;
  const savedCliPath = saved?.cliPath ?? "";
  return draftHost !== savedHost || draftCliPath !== savedCliPath;
}

/** Describes where requests go, for confirmation messages. */
export function kastellanRouteLabel(settings: KastellanSettings): string {
  return settings.remote ? `${settings.host} over ssh` : "Kastellan on this computer";
}
