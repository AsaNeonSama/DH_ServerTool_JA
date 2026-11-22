import { APP_MESSAGES } from "./app";
import type { AppMessages } from "./app";
import { CLI_MESSAGES } from "./cli";
import type { CliMessages } from "./cli";
import { MOD_MESSAGES } from "./mods";
import type { ModMessages } from "./mods";
import { SERVER_MESSAGES } from "./server";
import type { ServerMessages } from "./server";
import { TERMINAL_MESSAGES } from "./terminal";
import type { TerminalMessages } from "./terminal";
import { WEB_CONSOLE_MESSAGES } from "./webConsole";
import type { WebConsoleApiMessages, WebConsoleMessages } from "./webConsole";

export interface MessagesCatalog {
  readonly app: AppMessages;
  readonly cli: CliMessages;
  readonly mods: ModMessages;
  readonly server: ServerMessages;
  readonly terminal: TerminalMessages;
  readonly webConsole: WebConsoleMessages;
}

export const MESSAGES: MessagesCatalog = {
  app: APP_MESSAGES,
  cli: CLI_MESSAGES,
  mods: MOD_MESSAGES,
  server: SERVER_MESSAGES,
  terminal: TERMINAL_MESSAGES,
  webConsole: WEB_CONSOLE_MESSAGES,
};

export {
  APP_MESSAGES,
  CLI_MESSAGES,
  MOD_MESSAGES,
  SERVER_MESSAGES,
  TERMINAL_MESSAGES,
  WEB_CONSOLE_MESSAGES,
};

export type {
  AppMessages,
  CliMessages,
  ModMessages,
  ServerMessages,
  TerminalMessages,
  WebConsoleApiMessages,
  WebConsoleMessages,
};
