export interface WebConsoleOptions {
  readonly webPort: number;
  readonly consolePatchPort: number;
  readonly autoBroadcast: boolean;
  readonly publicIp: string | null;
}

export interface WebConsoleDisplayInfo {
  readonly port: number;
  readonly consolePatchPort: number;
  readonly autoBroadcast: boolean;
  readonly publicUrl: string;
  readonly localUrl: string;
}

export interface WebConsoleInstance {
  readonly authId: string;
  readonly authToken: string;
  readonly port: number;
  readonly displayInfo: WebConsoleDisplayInfo;
  readonly fetch: (req: Request) => Response | Promise<Response>;
  readonly stop: () => void;
}

export interface CachedPage {
  readonly raw: Uint8Array<ArrayBuffer>;
  readonly gzip: Uint8Array<ArrayBuffer>;
  readonly br: Uint8Array<ArrayBuffer>;
}

export interface TcpSocketWriter {
  write: (data: Uint8Array) => number;
  flush: () => void;
  end: () => void;
  terminate: () => void;
}

export interface TcpPacketOptions {
  readonly port: number;
  readonly text: string;
  readonly timeoutMs?: number;
}

export type PlayerRoleName =
  | "Captain"
  | "Chaplain"
  | "Cook"
  | "Doctor"
  | "Engineer"
  | "Hunter"
  | "Marine"
  | "Navigator";

export interface ApiResultPayload {
  readonly status: number;
  readonly msg: string;
}

export interface ExecuteConsoleCommandPayload {
  readonly action: "execute_console_command";
  readonly targetRole: string;
  readonly command: string;
}

export interface SetHumanPropertyPayload {
  readonly action: "set_human_property";
  readonly targetRole: string;
  readonly propertyName: string;
  readonly propertyValue: number;
}

export interface AddInventoryItemPayload {
  readonly action: "add_inventory_item";
  readonly targetRole: string;
  readonly itemBlueprintPath: string;
  readonly itemCount: number;
}

export interface SetStorageLimitPayload {
  readonly action: "set_storage_limit";
  readonly targetRole: string;
  readonly storageLimit: number;
}

export interface SendThrallMessagePayload {
  readonly action: "send_thrall_message";
  readonly targetRole: string;
  readonly message: string;
}

export interface KickPlayerPayload {
  readonly action: "kick_player";
  readonly targetRole: string;
}

export type ConsoleBridgePayload =
  | ExecuteConsoleCommandPayload
  | SetHumanPropertyPayload
  | AddInventoryItemPayload
  | SetStorageLimitPayload
  | SendThrallMessagePayload
  | KickPlayerPayload;

export type RouteHandler = (req: Request) => Response | Promise<Response>;

export type EncryptedRouteHandler = (
  data: Record<string, unknown>
) => ApiResultPayload | Promise<ApiResultPayload>;
