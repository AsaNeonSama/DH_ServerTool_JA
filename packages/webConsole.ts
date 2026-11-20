import { readFileSync } from "fs";
import { brotliCompressSync, gzipSync } from "zlib";
import controlPageHtml from "../assets/console/dh_console.html" with { type: "text" };
import unAuthPageHtml from "../assets/console/un_auth.html" with { type: "text" };
import appIconIcoPath from "../assets/icon.ico" with { type: "file" };
import {
  ACTOR_COMMENT_DICT,
  AUTH_ID_PREFIX,
  BONE_DAGGER_ITEM,
  C,
  CONSOLE_MESSAGES,
  COOKIE_MAX_AGE_SECONDS,
  COOKIE_NAME,
  DEFAULT_TCP_TIMEOUT_MS,
  HIGH_STAT_WARNING_THRESHOLD,
  PLAYER_NAME_DICT,
  PLAYER_NAME_LIST,
  STATUS_CODE,
} from "./constants";
import {
  createAesGcmCipher,
  createEncryptedAuthToken,
  verifyEncryptedAuthToken,
} from "./crypto";
import { println } from "./terminal";
import type {
  AesGcmSessionCipher,
  AuthRouteHandler,
  CachedPage,
  RouteHandler,
  TcpPacketOptions,
  TcpSocketWriter,
  WebConsoleInstance,
  WebConsoleOptions,
} from "./types";

function toArrayBufferUint8(data: Uint8Array): Uint8Array<ArrayBuffer> {
  const copy: Uint8Array<ArrayBuffer> = new Uint8Array(data.byteLength);
  copy.set(data);
  return copy;
}

function unwrapTextImport(asset: string | Bun.HTMLBundle): string {
  return typeof asset === "string" ? asset : "";
}

function precompressHtml(html: string): CachedPage {
  const rawBuf: Buffer = Buffer.from(html, "utf-8");
  return {
    raw: toArrayBufferUint8(rawBuf),
    gzip: toArrayBufferUint8(gzipSync(rawBuf, { level: 6 })),
    br: toArrayBufferUint8(brotliCompressSync(rawBuf)),
  };
}

const controlPageCache: CachedPage = precompressHtml(unwrapTextImport(controlPageHtml));
const unAuthPageCache: CachedPage = precompressHtml(unwrapTextImport(unAuthPageHtml));
const faviconBytes: Uint8Array<ArrayBuffer> = toArrayBufferUint8(
  readFileSync(appIconIcoPath)
);

function htmlResponse(req: Request, page: CachedPage): Response {
  const accept: string = req.headers.get("accept-encoding") ?? "";
  if (accept.includes("br")) {
    return new Response(page.br, {
      headers: {
        "Content-Type": "text/html; charset=utf-8",
        "Content-Encoding": "br",
        Vary: "Accept-Encoding",
      },
    });
  }
  if (accept.includes("gzip")) {
    return new Response(page.gzip, {
      headers: {
        "Content-Type": "text/html; charset=utf-8",
        "Content-Encoding": "gzip",
        Vary: "Accept-Encoding",
      },
    });
  }
  return new Response(page.raw, {
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      Vary: "Accept-Encoding",
    },
  });
}

function parseCookies(header: string | null): Record<string, string> {
  const out: Record<string, string> = {};
  if (!header) return out;
  const parts: string[] = header.split(";");
  for (const part of parts) {
    const eq: number = part.indexOf("=");
    if (eq === -1) continue;
    const key: string = part.slice(0, eq).trim();
    const val: string = part.slice(eq + 1).trim();
    if (key) out[key] = decodeURIComponent(val);
  }
  return out;
}

function toStringValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return "";
}

function redirectTo(location: string): Response {
  return new Response(null, {
    status: 302,
    headers: { Location: location },
  });
}

function redirectToUnAuth(): Response {
  return redirectTo("/dh-console/un-auth");
}

function jsonBody(status: number, msg: string): Response {
  return Response.json({ status, msg });
}

function roleDisplay(name: string): string {
  return PLAYER_NAME_DICT[name] ?? name;
}

async function parseJsonObject(req: Request): Promise<Record<string, unknown> | null> {
  try {
    const parsed: unknown = await req.json();
    if (typeof parsed !== "object" || parsed === null) {
      return null;
    }
    return parsed as Record<string, unknown>;
  } catch {
    return null;
  }
}

/**
 * Sends a UTF-16LE null-terminated JSON string to console_win64.js over TCP.
 */
async function sendUtf16TcpPacket(options: TcpPacketOptions): Promise<Error | null> {
  const {
    port,
    text,
    timeoutMs = DEFAULT_TCP_TIMEOUT_MS,
  }: TcpPacketOptions = options;
  const bytes: Buffer = Buffer.from(`${text}\0`, "utf16le");
  const result: Promise<Error | null> = new Promise(
    (resolve: (value: Error | null) => void): void => {
      let settled: boolean = false;
      let offset: number = 0;
      let activeSocket: TcpSocketWriter | null = null;

      const finish = (err: Error | null): void => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve(err);
      };

      const writeRemaining = (socket: TcpSocketWriter): void => {
        try {
          while (offset < bytes.byteLength) {
            const written: number = socket.write(bytes.subarray(offset));
            if (written <= 0) {
              return;
            }
            offset += written;
          }
          socket.flush();
          socket.end();
          finish(null);
        } catch (err: unknown) {
          try {
            socket.terminate();
          } catch {
            // Ignore socket terminate error
          }
          finish(err instanceof Error ? err : new Error(String(err)));
        }
      };

      const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
        try {
          activeSocket?.terminate();
        } catch {
          // Ignore timeout terminate error
        }
        finish(new Error("TCP connection timeout"));
      }, timeoutMs);

      Bun.connect({
        hostname: "127.0.0.1",
        port,
        socket: {
          open(socket: TcpSocketWriter): void {
            activeSocket = socket;
            writeRemaining(socket);
          },
          drain(socket: TcpSocketWriter): void {
            writeRemaining(socket);
          },
          data(): void {
            // console_win64.js does not send response data
          },
          close(): void {
            finish(
              offset >= bytes.byteLength
                ? null
                : new Error("Connection closed early")
            );
          },
          error(_socket: unknown, err: Error): void {
            finish(err);
          },
          connectError(_socket: unknown, err: Error): void {
            finish(err);
          },
        },
      }).catch((err: unknown): void => {
        finish(err instanceof Error ? err : new Error(String(err)));
      });
    }
  );
  return result;
}

function buildConsoleBroadcastMessage(
  playerName: string,
  cmdName: string,
  cmdArg: string
): string | null {
  if (cmdName === "summon") {
    const target: string = ACTOR_COMMENT_DICT[cmdArg] ?? cmdArg;
    return `${roleDisplay(playerName)} が${target}を召喚しました`;
  }
  if (cmdName === "setisthrall") {
    return `${roleDisplay(playerName)} が闇の傀儡に悪墜ちしました`;
  }
  if (cmdName === "setspellcharge") {
    return `${roleDisplay(playerName)} がスペールチャージしました`;
  }
  if (cmdName === "SetBlizzardIntensity") {
    return cmdArg === "0" || cmdArg === "0.00"
      ? `${roleDisplay(playerName)} が暴風雪を停止しました`
      : `${roleDisplay(playerName)} が暴風雪を呼びました`;
  }
  if (cmdName === "slomo") {
    return cmdArg === "1"
      ? `${roleDisplay(playerName)} が世界を元に戻しました`
      : `${roleDisplay(playerName)} が世界を加速しました`;
  }
  if (cmdName === "god") {
    return cmdArg === "0"
      ? `${roleDisplay(playerName)} が無敵状態を解除しました`
      : `${roleDisplay(playerName)} が無敵状態になりました`;
  }
  return null;
}

export function startWebConsole(options: WebConsoleOptions): WebConsoleInstance {
  const { webPort, consolePatchPort, autoBroadcast, publicIp }: WebConsoleOptions = options;
  const authId: string = `${AUTH_ID_PREFIX}${crypto.randomUUID().toUpperCase()}`;
  const cipher: AesGcmSessionCipher = createAesGcmCipher();
  const authToken: string = createEncryptedAuthToken(authId, cipher);

  const sendCommandObj = (payload: Record<string, unknown>): Promise<Error | null> =>
    sendUtf16TcpPacket({
      port: consolePatchPort,
      text: JSON.stringify(payload),
    });

  const consoleCommand = (playerName: string, command: string): Promise<Error | null> =>
    sendCommandObj({ op: "consoleexec", role: playerName, command });

  const setHuman = (
    playerName: string,
    propName: string,
    value: number
  ): Promise<Error | null> =>
    sendCommandObj({
      op: "sethuman",
      role: playerName,
      propname: propName,
      value,
    });

  const addItem = (
    playerName: string,
    itemName: string,
    count: number
  ): Promise<Error | null> =>
    sendCommandObj({
      op: "additem",
      role: playerName,
      itemname: itemName,
      count,
    });

  const setStorage = (playerName: string, limit: number): Promise<Error | null> =>
    sendCommandObj({ op: "setstorage", role: playerName, limit });

  const sendThrallMsg = (playerName: string, msg: string): Promise<Error | null> =>
    sendCommandObj({ op: "thrallmsg", role: playerName, msg });

  const kickPlayer = (playerName: string): Promise<Error | null> =>
    sendCommandObj({ op: "kick", role: playerName });

  const broadcast = async (msg: string): Promise<Error | null> => {
    for (const role of PLAYER_NAME_LIST) {
      const err: Error | null = await sendThrallMsg(role, msg);
      if (err) return err;
    }
    return null;
  };

  const autoBroadcastExcept = async (
    excludePlayer: string,
    msg: string
  ): Promise<Error | null> => {
    for (const role of PLAYER_NAME_LIST) {
      if (role === excludePlayer) continue;
      const err: Error | null = await sendThrallMsg(role, msg);
      if (err) return err;
    }
    return null;
  };

  const isAuthenticated = (req: Request): boolean => {
    const cookies: Record<string, string> = parseCookies(req.headers.get("cookie"));
    const sessionCookie: string | undefined = cookies[COOKIE_NAME];
    if (!sessionCookie) return false;
    return cipher.verify(sessionCookie, authId);
  };

  const withAuth = (handler: RouteHandler): RouteHandler =>
    (req: Request): Response | Promise<Response> => {
      if (!isAuthenticated(req)) {
        return redirectToUnAuth();
      }
      return handler(req);
    };

  const handleFavicon: RouteHandler = (): Response =>
    new Response(faviconBytes, {
      status: 200,
      headers: {
        "Content-Type": "image/x-icon",
        "Cache-Control": "public, max-age=86400",
      },
    });

  const handleRoot: RouteHandler = (req: Request): Response =>
    isAuthenticated(req)
      ? redirectTo("/dh-console/home_page")
      : redirectToUnAuth();

  const handleUnAuth: RouteHandler = (req: Request): Response =>
    htmlResponse(req, unAuthPageCache);

  const handleAuth: AuthRouteHandler = (
    req: Bun.BunRequest<"/dh-console/auth/:token">
  ): Response => {
    const candidateToken: string = decodeURIComponent(req.params.token);
    if (!verifyEncryptedAuthToken(candidateToken, authId, cipher)) {
      return redirectToUnAuth();
    }
    const encryptedCookieValue: string = cipher.encrypt(authId);
    return new Response(null, {
      status: 302,
      headers: {
        Location: "/dh-console/home_page",
        "Set-Cookie": `${COOKIE_NAME}=${encodeURIComponent(
          encryptedCookieValue
        )}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${COOKIE_MAX_AGE_SECONDS}`,
      },
    });
  };

  const handleHomePage: RouteHandler = withAuth((req: Request): Response =>
    htmlResponse(req, controlPageCache));

  const handleConsoleCommand: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const playerName: unknown = data.player_name;
      const command: unknown = data.command;
      if (typeof playerName !== "string" || typeof command !== "string") {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const err: Error | null = await consoleCommand(playerName, command);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);

      const spaceIdx: number = command.indexOf(" ");
      const cmdName: string = spaceIdx === -1 ? command : command.slice(0, spaceIdx);
      const cmdArg: string = spaceIdx === -1 ? "" : command.slice(spaceIdx + 1);

      if (cmdName === "setisthrall") {
        const daggerErr: Error | null = await addItem(playerName, BONE_DAGGER_ITEM, 1);
        if (daggerErr) return jsonBody(STATUS_CODE.par, CONSOLE_MESSAGES.ine);
      }

      if (autoBroadcast) {
        const msg: string | null = buildConsoleBroadcastMessage(
          playerName,
          cmdName,
          cmdArg
        );
        if (msg !== null) {
          const bcErr: Error | null = await autoBroadcastExcept(playerName, msg);
          if (bcErr) return jsonBody(STATUS_CODE.par, CONSOLE_MESSAGES.ine);
        }
      }
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const handleSetHuman: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const playerName: unknown = data.player_name;
      const propName: unknown = data.prop_name;
      const webValue: unknown = data.value;
      if (
        typeof playerName !== "string" ||
        typeof propName !== "string" ||
        webValue === undefined ||
        webValue === null ||
        webValue === ""
      ) {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const value: number = Number(webValue);
      if (!Number.isFinite(value) || value < 0) {
        return jsonBody(STATUS_CODE.val, CONSOLE_MESSAGES.val);
      }

      const err: Error | null = await setHuman(playerName, propName, value);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);

      if (autoBroadcast) {
        const bcErr: Error | null = await autoBroadcastExcept(
          playerName,
          `${roleDisplay(playerName)} が自分のステータスを操作しました`
        );
        if (bcErr) return jsonBody(STATUS_CODE.par, CONSOLE_MESSAGES.ine);
      }
      if (value > HIGH_STAT_WARNING_THRESHOLD) {
        return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.ove);
      }
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const handleAddItem: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const playerName: unknown = data.player_name;
      const itemName: unknown = data.item_name;
      const rawCount: unknown = data.count;
      if (
        typeof playerName !== "string" ||
        typeof itemName !== "string" ||
        rawCount === undefined ||
        rawCount === null
      ) {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const count: number = Number.parseInt(toStringValue(rawCount), 10);
      if (!Number.isInteger(count) || count <= 0) {
        return jsonBody(STATUS_CODE.val, CONSOLE_MESSAGES.val);
      }

      const err: Error | null = await addItem(playerName, itemName, count);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);

      if (autoBroadcast) {
        const label: string = ACTOR_COMMENT_DICT[itemName] ?? itemName;
        const bcErr: Error | null = await autoBroadcastExcept(
          playerName,
          `${roleDisplay(playerName)} が ${label} ${count} を獲得しました`
        );
        if (bcErr) return jsonBody(STATUS_CODE.par, CONSOLE_MESSAGES.ine);
      }
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const handleSetStorage: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const playerName: unknown = data.player_name;
      const rawLimit: unknown = data.limit;
      if (
        typeof playerName !== "string" ||
        rawLimit === undefined ||
        rawLimit === null
      ) {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const limit: number = Number.parseInt(toStringValue(rawLimit), 10);
      if (!Number.isInteger(limit) || limit <= 0) {
        return jsonBody(STATUS_CODE.val, CONSOLE_MESSAGES.val);
      }

      const err: Error | null = await setStorage(playerName, limit);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);

      if (autoBroadcast) {
        const bcErr: Error | null = await autoBroadcastExcept(
          playerName,
          `${roleDisplay(playerName)} のストレージ容量が ${limit} になりました`
        );
        if (bcErr) return jsonBody(STATUS_CODE.par, CONSOLE_MESSAGES.ine);
      }
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const handleSendThrallMsg: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const playerName: unknown = data.player_name;
      const msg: unknown = data.msg;
      if (typeof playerName !== "string" || typeof msg !== "string") {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const err: Error | null = await sendThrallMsg(playerName, msg);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const handleBroadcast: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const msg: unknown = data.msg;
      if (typeof msg !== "string") {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const err: Error | null = await broadcast(msg);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const handleKickPlayer: RouteHandler = withAuth(
    async (req: Request): Promise<Response> => {
      const data: Record<string, unknown> | null = await parseJsonObject(req);
      if (!data) return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);

      const playerName: unknown = data.player_name;
      if (typeof playerName !== "string") {
        return jsonBody(STATUS_CODE.net, CONSOLE_MESSAGES.net);
      }

      const err: Error | null = await kickPlayer(playerName);
      if (err) return jsonBody(STATUS_CODE.ser, CONSOLE_MESSAGES.ser);

      if (autoBroadcast) {
        const bcErr: Error | null = await autoBroadcastExcept(
          playerName,
          `${roleDisplay(playerName)} がキックされました`
        );
        if (bcErr) return jsonBody(STATUS_CODE.par, CONSOLE_MESSAGES.ine);
      }
      return jsonBody(STATUS_CODE.suc, CONSOLE_MESSAGES.suc);
    }
  );

  const server: Bun.Server<undefined> = Bun.serve({
    hostname: "0.0.0.0",
    port: webPort,
    routes: {
      "/favicon.ico": { GET: handleFavicon },
      "/": { GET: handleRoot },
      "/dh-console/un-auth": { GET: handleUnAuth },
      "/dh-console/auth/:token": { GET: handleAuth },
      "/dh-console/home_page": { GET: handleHomePage },
      "/dh-console/console_command": { POST: handleConsoleCommand },
      "/dh-console/set_human": { POST: handleSetHuman },
      "/dh-console/add_item": { POST: handleAddItem },
      "/dh-console/set_storage": { POST: handleSetStorage },
      "/dh-console/send_thrall_msg": { POST: handleSendThrallMsg },
      "/dh-console/send_thrall_msg/broadcast": { POST: handleBroadcast },
      "/dh-console/kick_player": { POST: handleKickPlayer },
    },
    fetch(): Response {
      return redirectToUnAuth();
    },
  });

  const displayPublicIp: string = publicIp ?? "127.0.0.1";
  const broadcastStatusText: string = autoBroadcast ? "オン" : "オフ";
  println();
  println(
    `${C.LIGHT_CYAN} ==================== Webコンソール起動 ====================`
  );
  println(
    `${C.LIGHT_WHITE} * パブリックアドレス: ${C.LIGHT_GREEN}http://${displayPublicIp}:${webPort}/dh-console/auth/${authToken}`
  );
  println(
    `${C.LIGHT_WHITE} * ローカルアドレス:   ${C.LIGHT_GREEN}http://127.0.0.1:${webPort}/dh-console/auth/${authToken}`
  );
  println(
    `${C.LIGHT_YELLOW} * ${authToken}${C.LIGHT_WHITE} はあなたのアクセス認証です(AES-256-GCM)。${C.LIGHT_RED}信用していない人に渡さないで下さい`
  );
  println(
    `${C.LIGHT_BLACK} * 内部連携ポート(console_win64.js): 127.0.0.1:${consolePatchPort} / 自動ブロードキャスト: ${broadcastStatusText}`
  );
  println(
    `${C.LIGHT_CYAN} ===========================================================`
  );

  return {
    authId,
    authToken,
    port: server.port ?? webPort,
    fetch(req: Request): Response | Promise<Response> {
      return server.fetch(req);
    },
    stop(): void {
      try {
        void server.stop(true);
      } catch {
        // Ignore stop failure
      }
    },
  };
}
