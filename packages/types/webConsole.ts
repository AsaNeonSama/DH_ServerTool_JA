export interface WebConsoleOptions {
  readonly webPort: number;
  readonly consolePatchPort: number;
  readonly autoBroadcast: boolean;
  readonly publicIp: string | null;
}

export interface WebConsoleInstance {
  readonly authId: string;
  readonly authToken: string;
  readonly port: number;
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

export type RouteHandler = (req: Request) => Response | Promise<Response>;

export type AuthRouteHandler = (
  req: Bun.BunRequest<"/dh-console/auth/:token">
) => Response;
