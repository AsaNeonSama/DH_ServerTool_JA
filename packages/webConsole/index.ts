import { AUTH_PATH, AUTH_PSK_SEPARATOR, C } from "../constants";
import { createAesGcmCipher } from "../core";
import { TERMINAL_MESSAGES, WEB_CONSOLE_MESSAGES } from "../messages";
import { isInteractiveTty, println } from "../terminal";
import type {
  AesGcmSessionCipher,
  RouteHandler,
  WebConsoleDisplayInfo,
  WebConsoleInstance,
  WebConsoleOptions,
} from "../types";
import {
  controlPageCache,
  faviconBytes,
  htmlResponse,
  unAuthPageCache,
} from "./assets";
import { createWebConsoleAuth } from "./auth";
import type { WebConsoleAuth } from "./auth";
import { createConsoleBridge } from "./bridge";
import type { ConsoleBridge } from "./bridge";
import { redirectTo, redirectToUnAuth } from "./httpUtils";
import { emitWebLog, normalizeClientIp } from "./logger";
import { createApiRoutes } from "./routes";
import type { ApiRouteHandlers } from "./routes";

export function startWebConsole(
  options: WebConsoleOptions
): WebConsoleInstance {
  const {
    webPort,
    consolePatchPort,
    autoBroadcast,
    publicIp,
  }: WebConsoleOptions = options;
  const authId: string = crypto.randomUUID().toUpperCase();
  const cipher: AesGcmSessionCipher = createAesGcmCipher();
  const authToken: string = cipher.psk;
  let httpServer: Bun.Server<undefined> | null = null;

  const resolveClientIp = (req: Request): string => {
    const socketAddr: string | undefined = httpServer?.requestIP(req)?.address;
    if (socketAddr) {
      return normalizeClientIp(socketAddr);
    }
    const forwarded: string | null = req.headers.get("x-forwarded-for");
    const firstHop: string | undefined = forwarded?.split(",")[0];
    return normalizeClientIp(firstHop);
  };

  const auth: WebConsoleAuth = createWebConsoleAuth({
    authId,
    cipher,
    resolveClientIp,
  });
  const bridge: ConsoleBridge = createConsoleBridge(consolePatchPort);
  const apiRoutes: ApiRouteHandlers = createApiRoutes({
    auth,
    bridge,
    autoBroadcast,
  });

  const handleFavicon: RouteHandler = (): Response =>
    new Response(faviconBytes, {
      status: 200,
      headers: {
        "Content-Type": "image/x-icon",
        "Cache-Control": "public, max-age=86400",
      },
    });

  const handleRoot: RouteHandler = (req: Request): Response =>
    auth.isAuthenticated(req)
      ? redirectTo("/dh-console/home_page")
      : redirectToUnAuth();

  const handleUnAuth: RouteHandler = (req: Request): Response =>
    htmlResponse(req, unAuthPageCache);

  const handleHomePage: RouteHandler = auth.withGetAuth((req: Request): Response =>
    htmlResponse(req, controlPageCache, true));

  const server: Bun.Server<undefined> = Bun.serve({
    hostname: "0.0.0.0",
    port: webPort,
    routes: {
      "/favicon.ico": { GET: handleFavicon },
      "/": { GET: handleRoot },
      "/dh-console/un-auth": { GET: handleUnAuth },
      "/dh-console/auth": { GET: auth.handleAuth },
      "/dh-console/auth/verify": { POST: auth.handleAuthVerify },
      "/dh-console/home_page": { GET: handleHomePage },
      "/dh-console/console_command": { POST: apiRoutes.handleConsoleCommand },
      "/dh-console/set_human": { POST: apiRoutes.handleSetHuman },
      "/dh-console/add_item": { POST: apiRoutes.handleAddItem },
      "/dh-console/set_storage": { POST: apiRoutes.handleSetStorage },
      "/dh-console/send_thrall_msg": { POST: apiRoutes.handleSendThrallMsg },
      "/dh-console/send_thrall_msg/broadcast": { POST: apiRoutes.handleBroadcast },
      "/dh-console/kick_player": { POST: apiRoutes.handleKickPlayer },
    },
    fetch(req: Request): Response {
      if (req.method !== "GET") {
        return new Response(null, { status: 401 });
      }
      return redirectToUnAuth();
    },
    error(): Response {
      return new Response(null, { status: 500 });
    },
  });
  httpServer = server;

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

  const broadcastStatusText: string = autoBroadcast
    ? TERMINAL_MESSAGES.toggleOn
    : TERMINAL_MESSAGES.toggleOff;
  emitWebLog({
    category: WEB_CONSOLE_MESSAGES.logCategories.startup,
    categoryColor: C.LIGHT_GREEN,
    detail: WEB_CONSOLE_MESSAGES.logDetails.startupDetail(
      boundPort,
      consolePatchPort,
      broadcastStatusText
    ),
  });

  if (!isInteractiveTty()) {
    println();
    println(`${C.LIGHT_CYAN} ${TERMINAL_MESSAGES.webStartedHeader}`);
    println(`${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.publicUrlLabel} ${publicUrl}`);
    println(`${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.localUrlLabel} ${localUrl}`);
    println(
      `${C.LIGHT_YELLOW} * ${TERMINAL_MESSAGES.pskNoticeLead}${C.LIGHT_WHITE}${TERMINAL_MESSAGES.pskNoticeBody}${C.LIGHT_MAGENTA}${TERMINAL_MESSAGES.pskNoticeWarn}`
    );
    println(
      `${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.internalBridgePortSummary(consolePatchPort, broadcastStatusText)}`
    );
    println(`${C.LIGHT_CYAN} ${TERMINAL_MESSAGES.webStartedFooter}`);
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
      emitWebLog({
        category: WEB_CONSOLE_MESSAGES.logCategories.shutdown,
        categoryColor: C.LIGHT_YELLOW,
        detail: WEB_CONSOLE_MESSAGES.logDetails.shutdownDetail,
      });
      try {
        void server.stop(true);
      } catch {
        // Ignore stop failure
      }
    },
  };
}
