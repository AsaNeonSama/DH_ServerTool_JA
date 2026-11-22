import { WEB_CONSOLE_MESSAGES } from "../messages";
import type { WebConsoleApiMessages } from "../messages";
import type { PlayerRoleName } from "../types";

export const PLAYER_NAME_LIST: readonly PlayerRoleName[] = [
  "Captain",
  "Chaplain",
  "Cook",
  "Doctor",
  "Engineer",
  "Hunter",
  "Marine",
  "Navigator",
];

export const PLAYER_NAME_DICT: Readonly<Record<string, string>> =
  WEB_CONSOLE_MESSAGES.playerRoleNames;

export const ACTOR_COMMENT_DICT: Readonly<Record<string, string>> =
  WEB_CONSOLE_MESSAGES.actorDisplayNames;

export const BONE_DAGGER_ITEM: string =
  "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C";

export const STATUS_CODE: Readonly<
  Record<
    "success" | "badRequest" | "serverError" | "invalidValue" | "partialSuccess",
    number
  >
> = {
  success: 200,
  badRequest: 400,
  serverError: 504,
  invalidValue: 416,
  partialSuccess: 207,
};

export const CONSOLE_MESSAGES: WebConsoleApiMessages = WEB_CONSOLE_MESSAGES.api;

export const COOKIE_NAME: string = "dh_console_auth";
export const COOKIE_MAX_AGE_SECONDS: number = 315_360_000;
export const DEFAULT_TCP_TIMEOUT_MS: number = 1500;
export const HIGH_STAT_WARNING_THRESHOLD: number = 500;
