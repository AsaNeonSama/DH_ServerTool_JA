import { C, MAX_VALID_PORT, MIN_VALID_PORT } from "../constants";
import { CLI_MESSAGES } from "../messages";
import { println } from "../terminal";
import type { Prompter } from "../terminal";
import type { NumericRangePromptOptions } from "../types";

export function parsePortInput(raw: string, fallback: number): number | null {
  const value: number = raw === "" ? fallback : Number(raw);
  if (
    !Number.isInteger(value) ||
    value < MIN_VALID_PORT ||
    value > MAX_VALID_PORT
  ) {
    return null;
  }
  return value;
}

export function isAbortError(err: unknown): boolean {
  return err instanceof Error && err.name === "AbortError";
}

export async function askNumberInRange(
  prompter: Prompter,
  options: NumericRangePromptOptions,
  signal?: AbortSignal
): Promise<number> {
  const {
    label,
    min,
    max,
    defaultValue,
    integerOnly,
  }: NumericRangePromptOptions = options;
  while (true) {
    const raw: string = await prompter.ask(
      `${C.LIGHT_CYAN} * ${label}(${min}-${max}) ${C.LIGHT_BLACK}[Enter=${defaultValue}]${C.LIGHT_CYAN}: `,
      signal
    );
    if (raw === "") {
      return defaultValue;
    }
    const parsed: number = Number(raw);
    const validType: boolean = integerOnly
      ? Number.isInteger(parsed)
      : Number.isFinite(parsed);
    if (!validType || parsed < min || parsed > max) {
      println(`${C.RED} * ${CLI_MESSAGES.invalidChoiceRetry}`);
      continue;
    }
    return parsed;
  }
}
