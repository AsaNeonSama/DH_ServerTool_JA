import {
  CONSOLE_PATCH_ID,
  CONSOLE_PATCH_PORT_TOKEN,
  DEFAULT_PATCH_SCRIPTS,
  SELECTABLE_MOD_OPTIONS,
} from "../constants";
import type { ModMenuOption, PatchScript } from "../types";

export function areModOptionsConflicting(
  a: ModMenuOption,
  b: ModMenuOption
): boolean {
  if (a.id === b.id) return false;
  return (
    (a.conflictsWith?.includes(b.id) ?? false) ||
    (b.conflictsWith?.includes(a.id) ?? false)
  );
}

export function findConflictingSelectedOptions(
  candidate: ModMenuOption,
  selectedNumbers: readonly number[]
): ModMenuOption[] {
  const conflicts: ModMenuOption[] = [];
  for (const num of selectedNumbers) {
    const selected: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS.find(
      (item: ModMenuOption): boolean => item.number === num
    );
    if (selected && areModOptionsConflicting(candidate, selected)) {
      conflicts.push(selected);
    }
  }
  return conflicts;
}

export function getModConflictPairs(): readonly (readonly [ModMenuOption, ModMenuOption])[] {
  const seenPairs: Set<string> = new Set();
  const pairs: (readonly [ModMenuOption, ModMenuOption])[] = [];

  for (let i: number = 0; i < SELECTABLE_MOD_OPTIONS.length; i++) {
    const a: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS[i];
    if (!a) continue;
    for (let j: number = i + 1; j < SELECTABLE_MOD_OPTIONS.length; j++) {
      const b: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS[j];
      if (!b || !areModOptionsConflicting(a, b)) continue;
      const key: string = `${a.id}::${b.id}`;
      if (seenPairs.has(key)) continue;
      seenPairs.add(key);
      pairs.push([a, b]);
    }
  }

  return pairs;
}

export function buildInjectionPlan(
  selectedOptionNumbers: readonly number[],
  consolePatchPort: number
): PatchScript[] {
  const seen: Set<string> = new Set();
  const acceptedOptions: ModMenuOption[] = [];
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
    const hasConflict: boolean = acceptedOptions.some(
      (existing: ModMenuOption): boolean => areModOptionsConflicting(existing, opt)
    );
    if (hasConflict) continue;
    acceptedOptions.push(opt);
    for (const patch of opt.scripts) {
      if (seen.has(patch.id)) continue;
      seen.add(patch.id);
      result.push(patch);
    }
  }

  return result;
}
