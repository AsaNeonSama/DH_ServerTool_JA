import { randomBytes } from "crypto";
import { readFileSync } from "fs";
import { brotliCompressSync, gzipSync } from "zlib";
import dhCipherJs from "../assets/console/dh_cipher.js" with { type: "text" };
import controlPageHtml from "../assets/console/dh_console.html" with { type: "text" };
import unAuthPageHtml from "../assets/console/un_auth.html" with { type: "text" };
import appIconIcoPath from "../assets/icon.ico" with { type: "file" };
import {
  ACTOR_COMMENT_DICT,
  AUTH_CHALLENGE_BYTES,
  AUTH_CHALLENGE_TTL_MS,
  AUTH_PATH,
  AUTH_PSK_SEPARATOR,
  AUTH_VERIFY_PATH,
  BONE_DAGGER_ITEM,
  C,
  CONSOLE_MESSAGES,
  COOKIE_AAD,
  COOKIE_MAX_AGE_SECONDS,
  COOKIE_NAME,
  DEFAULT_TCP_TIMEOUT_MS,
  HIGH_STAT_WARNING_THRESHOLD,
  MAX_PENDING_CHALLENGES,
  PLAYER_NAME_DICT,
  PLAYER_NAME_LIST,
  STATUS_CODE,
} from "./constants";
import { createAesGcmCipher } from "./crypto";
import { isInteractiveTty, println } from "./terminal";
import type {
  AesGcmSessionCipher,
  ApiResultPayload,
  CachedPage,
  ConsoleBridgePayload,
  EncryptedRouteHandler,
  RouteHandler,
  TcpPacketOptions,
  TcpSocketWriter,
  WebConsoleDisplayInfo,
  WebConsoleInstance,
  WebConsoleOptions,
} from "./types";

const CIPHER_INLINE_TOKEN: string = "/*__DH_CIPHER_INLINE__*/";

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

const cipherScriptSource: string = unwrapTextImport(dhCipherJs);
const controlPageWithCipher: string = unwrapTextImport(controlPageHtml).replace(
  CIPHER_INLINE_TOKEN,
  (): string => cipherScriptSource
);
const controlPageCache: CachedPage = precompressHtml(controlPageWithCipher);
const unAuthPageCache: CachedPage = precompressHtml(unwrapTextImport(unAuthPageHtml));
const faviconBytes: Uint8Array<ArrayBuffer> = toArrayBufferUint8(
  readFileSync(appIconIcoPath)
);

function buildAuthBootstrapHtml(challenge: string): string {
  return `<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta name="referrer" content="no-referrer">
  <title>DH-Web-Console</title>
  <link rel="icon" type="image/x-icon" href="/favicon.ico">
  <style>
    :root {
      --ink: #0b0b10;
      --paper: #f6f4ea;
      --cyan: #00e5ff;
      --yellow: #ffe600;
      --pink: #ff2a85;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      min-height: 100dvh;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: var(--paper);
      background-image:
        linear-gradient(to right, rgba(11, 11, 16, 0.055) 1px, transparent 1px),
        linear-gradient(to bottom, rgba(11, 11, 16, 0.055) 1px, transparent 1px);
      background-size: 26px 26px;
    }
    .dots {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 16px;
    }
    .dots i {
      display: block;
      width: 14px;
      height: 14px;
      border-radius: 50%;
      border: 2.5px solid var(--ink);
      animation: dotBounce 0.9s ease-in-out infinite;
    }
    .dots i:nth-child(1) {
      background-color: var(--cyan);
      animation-delay: 0s;
    }
    .dots i:nth-child(2) {
      background-color: var(--yellow);
      animation-delay: 0.16s;
    }
    .dots i:nth-child(3) {
      background-color: var(--pink);
      animation-delay: 0.32s;
    }
    @keyframes dotBounce {
      0%, 80%, 100% {
        transform: translateY(0) scale(0.8);
        opacity: 0.55;
      }
      40% {
        transform: translateY(-8px) scale(1.22);
        opacity: 1;
      }
    }
  </style>
</head>
<body>
  <div class="dots" role="status" aria-label="Loading"><i></i><i></i><i></i></div>
  <script>
${cipherScriptSource}
(async function () {
  "use strict";
  var STORAGE_KEY = "dh_console_psk";
  var CHALLENGE = ${JSON.stringify(challenge)};
  var VERIFY_PATH = ${JSON.stringify(AUTH_VERIFY_PATH)};

  function fail() {
    try { sessionStorage.removeItem(STORAGE_KEY); } catch (_) {}
    try { localStorage.removeItem(STORAGE_KEY); } catch (_) {}
    window.location.replace("/dh-console/un-auth");
  }

  var psk = window.location.hash && window.location.hash.length > 1
    ? window.location.hash.slice(1).trim()
    : "";
  try {
    window.history.replaceState(null, "", window.location.pathname);
  } catch (_) {}

  if (!window.__dhCipher || !window.__dhCipher.isValidPsk(psk)) {
    fail();
    return;
  }

  try {
    var cipherReq = window.__dhCipher.encrypt(
      JSON.stringify({ challenge: CHALLENGE }),
      psk,
      "REQ:" + VERIFY_PATH
    );
    var res = await window.fetch(VERIFY_PATH, {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: cipherReq
    });
    if (!res.ok) {
      fail();
      return;
    }
    var cipherRes = (await res.text()).trim();
    var plainRes = window.__dhCipher.decrypt(cipherRes, psk, "RES:" + VERIFY_PATH);
    if (plainRes === null) {
      fail();
      return;
    }
    var parsed = JSON.parse(plainRes);
    if (!parsed || parsed.ok !== true || parsed.challenge !== CHALLENGE) {
      fail();
      return;
    }
    try { sessionStorage.setItem(STORAGE_KEY, psk); } catch (_) {}
    try { localStorage.setItem(STORAGE_KEY, psk); } catch (_) {}
    window.location.replace("/dh-console/home_page");
  } catch (_) {
    fail();
  }
})();
  </script>
</body>
</html>`;
}

function htmlResponse(req: Request, page: CachedPage, noStore: boolean = false): Response {
  const accept: string = req.headers.get("accept-encoding") ?? "";
  const baseHeaders: Record<string, string> = {
    "Content-Type": "text/html; charset=utf-8",
    Vary: "Accept-Encoding",
    ...(noStore ? { "Cache-Control": "no-store" } : {}),
  };
  if (accept.includes("br")) {
    return new Response(page.br, {
      headers: {
        ...baseHeaders,
        "Content-Encoding": "br",
      },
    });
  }
  if (accept.includes("gzip")) {
    return new Response(page.gzip, {
      headers: {
        ...baseHeaders,
        "Content-Encoding": "gzip",
      },
    });
  }
  return new Response(page.raw, {
    headers: baseHeaders,
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

function apiResult(status: number, msg: string): ApiResultPayload {
  return { status, msg };
}

function roleDisplay(name: string): string {
  return PLAYER_NAME_DICT[name] ?? name;
}

function parseJsonObjectString(text: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
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
  const authId: string = crypto.randomUUID().toUpperCase();
  const cipher: AesGcmSessionCipher = createAesGcmCipher();
  const authToken: string = cipher.psk;
  const pendingChallenges: Map<string, number> = new Map();

  const pruneExpiredChallenges = (now: number): void => {
    for (const [key, expiresAt] of pendingChallenges) {
      if (expiresAt <= now) {
        pendingChallenges.delete(key);
      }
    }
  };

  const issueAuthChallenge = (): string => {
    const now: number = Date.now();
    pruneExpiredChallenges(now);
    while (pendingChallenges.size >= MAX_PENDING_CHALLENGES) {
      const oldestKey: string | undefined = pendingChallenges.keys().next().value;
      if (oldestKey === undefined) break;
      pendingChallenges.delete(oldestKey);
    }
    const challenge: string = randomBytes(AUTH_CHALLENGE_BYTES).toString("base64url");
    pendingChallenges.set(challenge, now + AUTH_CHALLENGE_TTL_MS);
    return challenge;
  };

  const consumeAuthChallenge = (challenge: string): boolean => {
    const now: number = Date.now();
    pruneExpiredChallenges(now);
    const expiresAt: number | undefined = pendingChallenges.get(challenge);
    if (expiresAt === undefined || expiresAt <= now) {
      pendingChallenges.delete(challenge);
      return false;
    }
    pendingChallenges.delete(challenge);
    return true;
  };

  const sendBridgePayload = (payload: ConsoleBridgePayload): Promise<Error | null> =>
    sendUtf16TcpPacket({
      port: consolePatchPort,
      text: JSON.stringify(payload),
    });

  const executeConsoleCommand = (
    targetRole: string,
    command: string
  ): Promise<Error | null> =>
    sendBridgePayload({
      action: "execute_console_command",
      targetRole,
      command,
    });

  const setHumanProperty = (
    targetRole: string,
    propertyName: string,
    propertyValue: number
  ): Promise<Error | null> =>
    sendBridgePayload({
      action: "set_human_property",
      targetRole,
      propertyName,
      propertyValue,
    });

  const addInventoryItem = (
    targetRole: string,
    itemBlueprintPath: string,
    itemCount: number
  ): Promise<Error | null> =>
    sendBridgePayload({
      action: "add_inventory_item",
      targetRole,
      itemBlueprintPath,
      itemCount,
    });

  const setPlayerStorageLimit = (
    targetRole: string,
    storageLimit: number
  ): Promise<Error | null> =>
    sendBridgePayload({
      action: "set_storage_limit",
      targetRole,
      storageLimit,
    });

  const sendPlayerMessage = (
    targetRole: string,
    message: string
  ): Promise<Error | null> =>
    sendBridgePayload({
      action: "send_thrall_message",
      targetRole,
      message,
    });

  const kickTargetPlayer = (targetRole: string): Promise<Error | null> =>
    sendBridgePayload({
      action: "kick_player",
      targetRole,
    });

  const broadcastMessageToAll = async (message: string): Promise<Error | null> => {
    for (const role of PLAYER_NAME_LIST) {
      const err: Error | null = await sendPlayerMessage(role, message);
      if (err) return err;
    }
    return null;
  };

  const broadcastMessageExcept = async (
    excludedRole: string,
    message: string
  ): Promise<Error | null> => {
    for (const role of PLAYER_NAME_LIST) {
      if (role === excludedRole) continue;
      const err: Error | null = await sendPlayerMessage(role, message);
      if (err) return err;
    }
    return null;
  };

  const isAuthenticated = (req: Request): boolean => {
    const cookies: Record<string, string> = parseCookies(req.headers.get("cookie"));
    const sessionCookie: string | undefined = cookies[COOKIE_NAME];
    if (!sessionCookie) return false;
    return cipher.verify(sessionCookie, authId, COOKIE_AAD);
  };

  const withGetAuth = (handler: RouteHandler): RouteHandler =>
    (req: Request): Response | Promise<Response> => {
      if (!isAuthenticated(req)) {
        return redirectToUnAuth();
      }
      return handler(req);
    };

  const encryptedResponse = (
    pathname: string,
    payload: unknown,
    extraHeaders?: Record<string, string>
  ): Response => {
    const ciphertext: string = cipher.encrypt(
      JSON.stringify(payload),
      `RES:${pathname}`
    );
    return new Response(ciphertext, {
      status: 200,
      headers: {
        "Content-Type": "text/plain; charset=utf-8",
        "Cache-Control": "no-store",
        ...extraHeaders,
      },
    });
  };

  const withEncryptedPostAuth = (
    pathname: string,
    handler: EncryptedRouteHandler
  ): RouteHandler =>
    async (req: Request): Promise<Response> => {
      if (!isAuthenticated(req)) {
        return new Response(null, { status: 401 });
      }
      const rawCiphertext: string = (await req.text()).trim();
      const decrypted: string | null = cipher.decrypt(
        rawCiphertext,
        `REQ:${pathname}`
      );
      if (decrypted === null) {
        return new Response(null, { status: 401 });
      }
      const data: Record<string, unknown> | null = parseJsonObjectString(decrypted);
      if (!data) {
        return encryptedResponse(
          pathname,
          apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError)
        );
      }
      const result: ApiResultPayload = await handler(data);
      return encryptedResponse(pathname, result);
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

  const handleAuth: RouteHandler = (): Response => {
    const challenge: string = issueAuthChallenge();
    return new Response(buildAuthBootstrapHtml(challenge), {
      status: 200,
      headers: {
        "Content-Type": "text/html; charset=utf-8",
        "Cache-Control": "no-store",
        "Referrer-Policy": "no-referrer",
      },
    });
  };

  const handleAuthVerify: RouteHandler = async (req: Request): Promise<Response> => {
    const rawCiphertext: string = (await req.text()).trim();
    const decrypted: string | null = cipher.decrypt(
      rawCiphertext,
      `REQ:${AUTH_VERIFY_PATH}`
    );
    if (decrypted === null) {
      return new Response(null, { status: 401 });
    }
    const data: Record<string, unknown> | null = parseJsonObjectString(decrypted);
    const challenge: unknown = data?.challenge;
    if (typeof challenge !== "string" || !consumeAuthChallenge(challenge)) {
      return new Response(null, { status: 401 });
    }
    const encryptedCookieValue: string = cipher.encrypt(authId, COOKIE_AAD);
    return encryptedResponse(
      AUTH_VERIFY_PATH,
      { ok: true, challenge },
      {
        "Set-Cookie": `${COOKIE_NAME}=${encodeURIComponent(
          encryptedCookieValue
        )}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${COOKIE_MAX_AGE_SECONDS}`,
      }
    );
  };

  const handleHomePage: RouteHandler = withGetAuth((req: Request): Response =>
    htmlResponse(req, controlPageCache, true));

  const handleConsoleCommand: RouteHandler = withEncryptedPostAuth(
    "/dh-console/console_command",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const command: unknown = data.command;
      if (typeof targetRole !== "string" || typeof command !== "string") {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const execErr: Error | null = await executeConsoleCommand(targetRole, command);
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }

      const spaceIdx: number = command.indexOf(" ");
      const commandName: string = spaceIdx === -1 ? command : command.slice(0, spaceIdx);
      const commandArg: string = spaceIdx === -1 ? "" : command.slice(spaceIdx + 1);

      if (commandName === "setisthrall") {
        const daggerErr: Error | null = await addInventoryItem(
          targetRole,
          BONE_DAGGER_ITEM,
          1
        );
        if (daggerErr) {
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }

      if (autoBroadcast) {
        const notice: string | null = buildConsoleBroadcastMessage(
          targetRole,
          commandName,
          commandArg
        );
        if (notice !== null) {
          const broadcastErr: Error | null = await broadcastMessageExcept(
            targetRole,
            notice
          );
          if (broadcastErr) {
            return apiResult(
              STATUS_CODE.partialSuccess,
              CONSOLE_MESSAGES.broadcastPartialError
            );
          }
        }
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleSetHuman: RouteHandler = withEncryptedPostAuth(
    "/dh-console/set_human",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const propertyName: unknown = data.prop_name;
      const rawValue: unknown = data.value;
      if (
        typeof targetRole !== "string" ||
        typeof propertyName !== "string" ||
        rawValue === undefined ||
        rawValue === null ||
        rawValue === ""
      ) {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const propertyValue: number = Number(rawValue);
      if (!Number.isFinite(propertyValue) || propertyValue < 0) {
        return apiResult(STATUS_CODE.invalidValue, CONSOLE_MESSAGES.invalidValueError);
      }

      const execErr: Error | null = await setHumanProperty(
        targetRole,
        propertyName,
        propertyValue
      );
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }

      if (autoBroadcast) {
        const broadcastErr: Error | null = await broadcastMessageExcept(
          targetRole,
          `${roleDisplay(targetRole)} が自分のステータスを操作しました`
        );
        if (broadcastErr) {
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      if (propertyValue > HIGH_STAT_WARNING_THRESHOLD) {
        return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.highValueWarning);
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleAddItem: RouteHandler = withEncryptedPostAuth(
    "/dh-console/add_item",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const itemBlueprintPath: unknown = data.item_name;
      const rawItemCount: unknown = data.count;
      if (
        typeof targetRole !== "string" ||
        typeof itemBlueprintPath !== "string" ||
        rawItemCount === undefined ||
        rawItemCount === null
      ) {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const itemCount: number = Number.parseInt(toStringValue(rawItemCount), 10);
      if (!Number.isInteger(itemCount) || itemCount <= 0) {
        return apiResult(STATUS_CODE.invalidValue, CONSOLE_MESSAGES.invalidValueError);
      }

      const execErr: Error | null = await addInventoryItem(
        targetRole,
        itemBlueprintPath,
        itemCount
      );
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }

      if (autoBroadcast) {
        const itemLabel: string =
          ACTOR_COMMENT_DICT[itemBlueprintPath] ?? itemBlueprintPath;
        const broadcastErr: Error | null = await broadcastMessageExcept(
          targetRole,
          `${roleDisplay(targetRole)} が ${itemLabel} ${itemCount} を獲得しました`
        );
        if (broadcastErr) {
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleSetStorage: RouteHandler = withEncryptedPostAuth(
    "/dh-console/set_storage",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const rawStorageLimit: unknown = data.limit;
      if (
        typeof targetRole !== "string" ||
        rawStorageLimit === undefined ||
        rawStorageLimit === null
      ) {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const storageLimit: number = Number.parseInt(toStringValue(rawStorageLimit), 10);
      if (!Number.isInteger(storageLimit) || storageLimit <= 0) {
        return apiResult(STATUS_CODE.invalidValue, CONSOLE_MESSAGES.invalidValueError);
      }

      const execErr: Error | null = await setPlayerStorageLimit(targetRole, storageLimit);
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }

      if (autoBroadcast) {
        const broadcastErr: Error | null = await broadcastMessageExcept(
          targetRole,
          `${roleDisplay(targetRole)} のストレージ容量が ${storageLimit} になりました`
        );
        if (broadcastErr) {
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleSendThrallMsg: RouteHandler = withEncryptedPostAuth(
    "/dh-console/send_thrall_msg",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      const message: unknown = data.msg;
      if (typeof targetRole !== "string" || typeof message !== "string") {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const execErr: Error | null = await sendPlayerMessage(targetRole, message);
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleBroadcast: RouteHandler = withEncryptedPostAuth(
    "/dh-console/send_thrall_msg/broadcast",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const message: unknown = data.msg;
      if (typeof message !== "string") {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const execErr: Error | null = await broadcastMessageToAll(message);
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const handleKickPlayer: RouteHandler = withEncryptedPostAuth(
    "/dh-console/kick_player",
    async (data: Record<string, unknown>): Promise<ApiResultPayload> => {
      const targetRole: unknown = data.player_name;
      if (typeof targetRole !== "string") {
        return apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError);
      }

      const execErr: Error | null = await kickTargetPlayer(targetRole);
      if (execErr) {
        return apiResult(STATUS_CODE.serverError, CONSOLE_MESSAGES.serverExecutionError);
      }

      if (autoBroadcast) {
        const broadcastErr: Error | null = await broadcastMessageExcept(
          targetRole,
          `${roleDisplay(targetRole)} がキックされました`
        );
        if (broadcastErr) {
          return apiResult(
            STATUS_CODE.partialSuccess,
            CONSOLE_MESSAGES.broadcastPartialError
          );
        }
      }
      return apiResult(STATUS_CODE.success, CONSOLE_MESSAGES.success);
    }
  );

  const server: Bun.Server<undefined> = Bun.serve({
    hostname: "0.0.0.0",
    port: webPort,
    routes: {
      "/favicon.ico": { GET: handleFavicon },
      "/": { GET: handleRoot },
      "/dh-console/un-auth": { GET: handleUnAuth },
      "/dh-console/auth": { GET: handleAuth },
      "/dh-console/auth/verify": { POST: handleAuthVerify },
      "/dh-console/home_page": { GET: handleHomePage },
      "/dh-console/console_command": { POST: handleConsoleCommand },
      "/dh-console/set_human": { POST: handleSetHuman },
      "/dh-console/add_item": { POST: handleAddItem },
      "/dh-console/set_storage": { POST: handleSetStorage },
      "/dh-console/send_thrall_msg": { POST: handleSendThrallMsg },
      "/dh-console/send_thrall_msg/broadcast": { POST: handleBroadcast },
      "/dh-console/kick_player": { POST: handleKickPlayer },
    },
    fetch(req: Request): Response {
      if (req.method !== "GET") {
        return new Response(null, { status: 401 });
      }
      return redirectToUnAuth();
    },
  });

  const boundPort: number = server.port ?? webPort;
  const displayPublicIp: string = publicIp ?? "127.0.0.1";
  const authSuffix: string = `${AUTH_PATH}${AUTH_PSK_SEPARATOR}${authToken}`;
  const publicUrl: string = `http://${displayPublicIp}:${boundPort}${authSuffix}`;
  const localUrl: string = `http://127.0.0.1:${boundPort}${authSuffix}`;
  const displayInfo: WebConsoleDisplayInfo = {
    port: boundPort,
    consolePatchPort,
    autoBroadcast,
    publicUrl,
    localUrl,
  };

  if (!isInteractiveTty()) {
    const broadcastStatusText: string = autoBroadcast ? "オン" : "オフ";
    println();
    println(
      `${C.LIGHT_CYAN} ==================== Webコンソール起動 ====================`
    );
    println(`${C.LIGHT_BLACK} * 外部URL: ${publicUrl}`);
    println(`${C.LIGHT_BLACK} * 内部URL: ${localUrl}`);
    println(
      `${C.LIGHT_YELLOW} * URL末尾の #PSK は事前共有鍵(AES-256-GCM)です。${C.LIGHT_WHITE}全API通信が密文で保護されます。${C.LIGHT_MAGENTA}信用していない人に渡さないで下さい`
    );
    println(
      `${C.LIGHT_BLACK} * 内部連携ポート(console_win64.js): 127.0.0.1:${consolePatchPort} / 自動ブロードキャスト: ${broadcastStatusText}`
    );
    println(
      `${C.LIGHT_CYAN} ===========================================================`
    );
  }

  return {
    authId,
    authToken,
    port: boundPort,
    displayInfo,
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
