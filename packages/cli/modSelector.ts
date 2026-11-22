import {
  C,
  DEFAULT_PATCH_SCRIPTS,
  MOD_MENU_COL_WIDTH,
  MOD_MENU_COLS,
  SELECTABLE_MOD_OPTIONS,
} from "../constants";
import { CLI_MESSAGES } from "../messages";
import {
  findConflictingSelectedOptions,
  getModConflictPairs,
} from "../server/mods";
import { padDisplayWidth, println } from "../terminal";
import type { Prompter } from "../terminal";
import type { ModMenuOption, PatchScript } from "../types";

export function renderModMenuTable(): void {
  const rows: number = Math.ceil(SELECTABLE_MOD_OPTIONS.length / MOD_MENU_COLS);

  for (let r: number = 0; r < rows; r++) {
    let line: string = " ";
    for (let c: number = 0; c < MOD_MENU_COLS; c++) {
      const idx: number = c * rows + r;
      const opt: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS[idx];
      if (!opt) continue;
      const suffix: string =
        opt.kind === "mode" ? CLI_MESSAGES.modModeSuffix : "";
      const cell: string = `${opt.number}:${opt.label}${suffix}`;
      const hasNextCol: boolean =
        (c + 1) * rows + r < SELECTABLE_MOD_OPTIONS.length;
      line += hasNextCol ? padDisplayWidth(cell, MOD_MENU_COL_WIDTH) : cell;
    }
    println(`${C.CYAN}${line}`);
  }

  const conflictPairs: readonly (readonly [ModMenuOption, ModMenuOption])[] =
    getModConflictPairs();
  if (conflictPairs.length > 0) {
    println();
    const pairText: string = conflictPairs
      .map(
        ([a, b]: readonly [ModMenuOption, ModMenuOption]): string =>
          `${a.number}:${a.label} <-> ${b.number}:${b.label}`
      )
      .join(" / ");
    println(`${C.LIGHT_BLACK} * ${CLI_MESSAGES.modConflictLegendPrefix} ${pairText}`);
  } else {
    println();
  }
}

export function formatSelectedLabels(selectedNumbers: readonly number[]): string {
  return selectedNumbers
    .map((n: number): string => {
      const found: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS.find(
        (item: ModMenuOption): boolean => item.number === n
      );
      return found?.label ?? String(n);
    })
    .join(", ");
}

export async function promptModSelection(
  prompter: Prompter,
  signal?: AbortSignal
): Promise<{
  readonly selectedOptionNumbers: number[];
  readonly selectedOptionLabels: string[];
}> {
  const defaultLabels: string = DEFAULT_PATCH_SCRIPTS.map(
    (patch: PatchScript): string => patch.label
  ).join(", ");
  println(
    `${C.LIGHT_CYAN} * ${CLI_MESSAGES.defaultModsLabel} ${C.LIGHT_GREEN}${defaultLabels}`
  );
  println(
    `${C.LIGHT_YELLOW} ${CLI_MESSAGES.modSelectionGuideBeforeY} ${C.LIGHT_GREEN}Y/y ${C.LIGHT_YELLOW}${CLI_MESSAGES.modSelectionGuideBetweenYAndC} ${C.LIGHT_GREEN}C/c ${C.LIGHT_YELLOW}${CLI_MESSAGES.modSelectionGuideAfterC}`
  );
  println();
  renderModMenuTable();

  const selectedNumbers: number[] = [];
  while (true) {
    const rawChoice: string = await prompter.ask(
      `${C.LIGHT_CYAN} * ${CLI_MESSAGES.modChoicePrompt} `,
      signal
    );
    const trimmedChoice: string = rawChoice.trim();
    const lower: string = trimmedChoice.toLowerCase();

    if (lower === "y") {
      break;
    }

    if (lower === "c") {
      selectedNumbers.length = 0;
      println(`${C.LIGHT_YELLOW} * ${CLI_MESSAGES.modSelectionCleared}`);
      continue;
    }

    const tokens: string[] = trimmedChoice
      .split(/[,，、]+/)
      .map((part: string): string => part.trim())
      .filter((part: string): boolean => part.length > 0);

    if (tokens.length === 0) {
      println(`${C.RED} * ${CLI_MESSAGES.invalidChoiceShort}`);
      continue;
    }

    const addedLabels: string[] = [];
    const duplicateLabels: string[] = [];
    const conflictNotices: string[] = [];
    const invalidTokens: string[] = [];

    for (const token of tokens) {
      const num: number = Number(token);
      const matched: ModMenuOption | undefined = Number.isInteger(num)
        ? SELECTABLE_MOD_OPTIONS.find(
          (item: ModMenuOption): boolean => item.number === num
        )
        : undefined;

      if (!matched) {
        invalidTokens.push(token);
        continue;
      }

      if (selectedNumbers.includes(matched.number)) {
        if (!duplicateLabels.includes(matched.label)) {
          duplicateLabels.push(matched.label);
        }
        continue;
      }

      const conflicts: ModMenuOption[] = findConflictingSelectedOptions(
        matched,
        selectedNumbers
      );
      if (conflicts.length > 0) {
        const conflictTargets: string = conflicts
          .map((item: ModMenuOption): string => `${item.number}:${item.label}`)
          .join(", ");
        const notice: string = CLI_MESSAGES.modConflictDetail(
          `${matched.number}:${matched.label}`,
          conflictTargets
        );
        if (!conflictNotices.includes(notice)) {
          conflictNotices.push(notice);
        }
        continue;
      }

      selectedNumbers.push(matched.number);
      addedLabels.push(matched.label);
    }

    if (addedLabels.length > 0) {
      const currentNames: string = formatSelectedLabels(selectedNumbers);
      println(
        `${C.LIGHT_GREEN} * ${CLI_MESSAGES.modAddedPrefix} ${addedLabels.join(", ")} ${C.LIGHT_BLACK}${CLI_MESSAGES.modCurrentSelection(currentNames)}`
      );
    }

    if (duplicateLabels.length > 0) {
      const currentNames: string = formatSelectedLabels(selectedNumbers);
      println(
        `${C.LIGHT_YELLOW} * ${CLI_MESSAGES.modDuplicatePrefix} ${duplicateLabels.join(", ")} ${C.LIGHT_BLACK}${CLI_MESSAGES.modCurrentSelection(currentNames)}`
      );
    }

    if (conflictNotices.length > 0) {
      println(
        `${C.RED} * ${CLI_MESSAGES.modConflictSkippedPrefix} ${conflictNotices.join(" / ")}`
      );
    }

    if (invalidTokens.length > 0) {
      println(
        `${C.RED} * ${CLI_MESSAGES.invalidChoiceList(invalidTokens.join(", "))}`
      );
    }
  }

  const selectedOptionLabels: string[] = selectedNumbers.map((n: number): string => {
    const found: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS.find(
      (item: ModMenuOption): boolean => item.number === n
    );
    return found?.label ?? String(n);
  });

  return {
    selectedOptionNumbers: selectedNumbers,
    selectedOptionLabels,
  };
}
