import {
  CONSOLE_PATCH_ID,
  CONSOLE_PATCH_PORT_TOKEN,
  DEFAULT_PATCH_SCRIPTS,
  SELECTABLE_MOD_OPTIONS,
} from "./constants";
import type { ModMenuOption, PatchScript } from "./types";

export function buildInjectionPlan(
  selectedOptionNumbers: readonly number[],
  consolePatchPort: number
): PatchScript[] {
  const seen: Set<string> = new Set();
  const result: PatchScript[] = [];

  for (const patch of DEFAULT_PATCH_SCRIPTS) {
    const resolvedSource: string =
      patch.id === CONSOLE_PATCH_ID
        ? patch.source.replaceAll(CONSOLE_PATCH_PORT_TOKEN, String(consolePatchPort))
        : patch.source;
    seen.add(patch.id);
    result.push({
      ...patch,
      source: resolvedSource,
    });
  }

  for (const num of selectedOptionNumbers) {
    const opt: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS.find(
      (item: ModMenuOption): boolean => item.number === num
    );
    if (!opt) continue;
    for (const patch of opt.scripts) {
      if (seen.has(patch.id)) continue;
      seen.add(patch.id);
      result.push(patch);
    }
  }

  return result;
}
