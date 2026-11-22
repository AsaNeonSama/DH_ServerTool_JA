export const APP_CONSOLE_TITLE: string = "DH_Server_Tool_JA";

export const C: Readonly<Record<
  | "RESET"
  | "RED"
  | "GREEN"
  | "YELLOW"
  | "CYAN"
  | "LIGHT_BLACK"
  | "LIGHT_RED"
  | "LIGHT_GREEN"
  | "LIGHT_YELLOW"
  | "LIGHT_BLUE"
  | "LIGHT_MAGENTA"
  | "LIGHT_CYAN"
  | "LIGHT_WHITE",
  string
>> = {
  RESET: "\x1b[0m",
  RED: "\x1b[31m",
  GREEN: "\x1b[32m",
  YELLOW: "\x1b[33m",
  CYAN: "\x1b[36m",
  LIGHT_BLACK: "\x1b[90m",
  LIGHT_RED: "\x1b[91m",
  LIGHT_GREEN: "\x1b[92m",
  LIGHT_YELLOW: "\x1b[93m",
  LIGHT_BLUE: "\x1b[94m",
  LIGHT_MAGENTA: "\x1b[95m",
  LIGHT_CYAN: "\x1b[96m",
  LIGHT_WHITE: "\x1b[97m",
};

export const MOD_MENU_COLS: number = 4;
export const MOD_MENU_COL_WIDTH: number = 27;
export const MAX_SERVER_TAB_LINES: number = 400;
export const MAX_WEB_LOG_LINES: number = 512;
export const TAB_ACTIVE_STYLE: string = "\x1b[1;30;106m";
export const TAB_INACTIVE_STYLE: string = "\x1b[37;100m";
export const TAB_BUTTON_STYLE: string = "\x1b[1;30;47m";
