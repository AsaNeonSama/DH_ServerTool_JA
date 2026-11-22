import { randomBytes } from "crypto";
import {
  AUTH_CHALLENGE_BYTES,
  AUTH_CHALLENGE_TTL_MS,
  AUTH_VERIFY_PATH,
  C,
  CONSOLE_MESSAGES,
  COOKIE_AAD,
  COOKIE_MAX_AGE_SECONDS,
  COOKIE_NAME,
  MAX_PENDING_CHALLENGES,
  STATUS_CODE,
} from "../constants";
import { sanitizeLogText } from "../core";
import { WEB_CONSOLE_MESSAGES } from "../messages";
import type {
  AesGcmSessionCipher,
  ApiResultPayload,
  EncryptedRouteHandler,
  RouteHandler,
} from "../types";
import { buildAuthBootstrapHtml } from "./assets";
import {
  apiResult,
  parseCookies,
  parseJsonObjectString,
  readBoundedRequestText,
  redirectToUnAuth,
} from "./httpUtils";
import { emitWebLog } from "./logger";

export interface WebConsoleAuthOptions {
  readonly authId: string;
  readonly cipher: AesGcmSessionCipher;
  readonly resolveClientIp: (req: Request) => string;
}

export interface WebConsoleAuth {
  readonly isAuthenticated: (req: Request) => boolean;
  readonly withGetAuth: (handler: RouteHandler) => RouteHandler;
  readonly withEncryptedPostAuth: (
    pathname: string,
    handler: EncryptedRouteHandler
  ) => RouteHandler;
  readonly handleAuth: RouteHandler;
  readonly handleAuthVerify: RouteHandler;
}

export function createWebConsoleAuth(
  options: WebConsoleAuthOptions
): WebConsoleAuth {
  const { authId, cipher, resolveClientIp }: WebConsoleAuthOptions = options;
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
    const challenge: string = randomBytes(AUTH_CHALLENGE_BYTES).toString(
      "base64url"
    );
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

  const isAuthenticated = (req: Request): boolean => {
    const cookies: Record<string, string> = parseCookies(
      req.headers.get("cookie")
    );
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
      const clientIp: string = resolveClientIp(req);
      try {
        if (!isAuthenticated(req)) {
          emitWebLog({
            category: WEB_CONSOLE_MESSAGES.logCategories.auth,
            categoryColor: C.RED,
            detail: WEB_CONSOLE_MESSAGES.logDetails.unauthenticatedRejected(
              clientIp,
              pathname
            ),
          });
          return new Response(null, { status: 401 });
        }
        const rawCiphertext: string | null = await readBoundedRequestText(req);
        if (rawCiphertext === null) {
          emitWebLog({
            category: WEB_CONSOLE_MESSAGES.logCategories.error,
            categoryColor: C.RED,
            detail: WEB_CONSOLE_MESSAGES.logDetails.invalidRequestBody(
              clientIp,
              pathname
            ),
          });
          return new Response(null, { status: 400 });
        }
        const decrypted: string | null = cipher.decrypt(
          rawCiphertext,
          `REQ:${pathname}`
        );
        if (decrypted === null) {
          emitWebLog({
            category: WEB_CONSOLE_MESSAGES.logCategories.auth,
            categoryColor: C.RED,
            detail: WEB_CONSOLE_MESSAGES.logDetails.decryptFailed(
              clientIp,
              pathname
            ),
          });
          return new Response(null, { status: 401 });
        }
        const data: Record<string, unknown> | null =
          parseJsonObjectString(decrypted);
        if (!data) {
          emitWebLog({
            category: WEB_CONSOLE_MESSAGES.logCategories.error,
            categoryColor: C.LIGHT_YELLOW,
            detail: WEB_CONSOLE_MESSAGES.logDetails.invalidJsonParams(
              clientIp,
              pathname
            ),
          });
          return encryptedResponse(
            pathname,
            apiResult(STATUS_CODE.badRequest, CONSOLE_MESSAGES.networkParamError)
          );
        }
        const result: ApiResultPayload = await handler(data, clientIp);
        return encryptedResponse(pathname, result);
      } catch (err: unknown) {
        const msg: string = err instanceof Error ? err.message : String(err);
        emitWebLog({
          category: WEB_CONSOLE_MESSAGES.logCategories.error,
          categoryColor: C.RED,
          detail: WEB_CONSOLE_MESSAGES.logDetails.internalRouteError(
            clientIp,
            pathname,
            sanitizeLogText(msg, 40)
          ),
        });
        return new Response(null, { status: 500 });
      }
    };

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

  const handleAuthVerify: RouteHandler = async (
    req: Request
  ): Promise<Response> => {
    const clientIp: string = resolveClientIp(req);
    try {
      const rawCiphertext: string | null = await readBoundedRequestText(req);
      if (rawCiphertext === null) {
        emitWebLog({
          category: WEB_CONSOLE_MESSAGES.logCategories.auth,
          categoryColor: C.RED,
          detail: WEB_CONSOLE_MESSAGES.logDetails.loginFailedInvalidBody(clientIp),
        });
        return new Response(null, { status: 400 });
      }
      const decrypted: string | null = cipher.decrypt(
        rawCiphertext,
        `REQ:${AUTH_VERIFY_PATH}`
      );
      if (decrypted === null) {
        emitWebLog({
          category: WEB_CONSOLE_MESSAGES.logCategories.auth,
          categoryColor: C.RED,
          detail: WEB_CONSOLE_MESSAGES.logDetails.loginFailedPskMismatch(clientIp),
        });
        return new Response(null, { status: 401 });
      }
      const data: Record<string, unknown> | null =
        parseJsonObjectString(decrypted);
      const challenge: unknown = data?.challenge;
      if (typeof challenge !== "string" || !consumeAuthChallenge(challenge)) {
        emitWebLog({
          category: WEB_CONSOLE_MESSAGES.logCategories.auth,
          categoryColor: C.LIGHT_YELLOW,
          detail:
            WEB_CONSOLE_MESSAGES.logDetails.loginFailedChallengeInvalid(clientIp),
        });
        return new Response(null, { status: 401 });
      }
      const encryptedCookieValue: string = cipher.encrypt(authId, COOKIE_AAD);
      emitWebLog({
        category: WEB_CONSOLE_MESSAGES.logCategories.auth,
        categoryColor: C.LIGHT_GREEN,
        detail: WEB_CONSOLE_MESSAGES.logDetails.loginSucceeded(clientIp),
      });
      return encryptedResponse(
        AUTH_VERIFY_PATH,
        { ok: true, challenge },
        {
          "Set-Cookie": `${COOKIE_NAME}=${encodeURIComponent(
            encryptedCookieValue
          )}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${COOKIE_MAX_AGE_SECONDS}`,
        }
      );
    } catch {
      emitWebLog({
        category: WEB_CONSOLE_MESSAGES.logCategories.auth,
        categoryColor: C.RED,
        detail: WEB_CONSOLE_MESSAGES.logDetails.loginFailedInternal(clientIp),
      });
      return new Response(null, { status: 500 });
    }
  };

  return {
    isAuthenticated,
    withGetAuth,
    withEncryptedPostAuth,
    handleAuth,
    handleAuthVerify,
  };
}
