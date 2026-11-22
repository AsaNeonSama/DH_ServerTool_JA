import { readFileSync } from "fs";
import { brotliCompressSync, gzipSync } from "zlib";
import dhCipherJs from "../../assets/console/dh_cipher.js" with { type: "text" };
import controlPageHtml from "../../assets/console/dh_console.html" with { type: "text" };
import unAuthPageHtml from "../../assets/console/un_auth.html" with { type: "text" };
import appIconIcoPath from "../../assets/icon.ico" with { type: "file" };
import { AUTH_VERIFY_PATH } from "../constants";
import type { CachedPage } from "../types";

const CIPHER_INLINE_TOKEN: string = "/*__DH_CIPHER_INLINE__*/";

function toArrayBufferUint8(data: Uint8Array): Uint8Array<ArrayBuffer> {
  const copy: Uint8Array<ArrayBuffer> = new Uint8Array(data.byteLength);
  copy.set(data);
  return copy;
}

function loadFaviconBytes(filePath: string): Uint8Array<ArrayBuffer> {
  try {
    return toArrayBufferUint8(readFileSync(filePath));
  } catch {
    return new Uint8Array(0);
  }
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

export const controlPageCache: CachedPage = precompressHtml(controlPageWithCipher);
export const unAuthPageCache: CachedPage = precompressHtml(
  unwrapTextImport(unAuthPageHtml)
);
export const faviconBytes: Uint8Array<ArrayBuffer> = loadFaviconBytes(
  appIconIcoPath
);

export function buildAuthBootstrapHtml(challenge: string): string {
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

export function htmlResponse(
  req: Request,
  page: CachedPage,
  noStore: boolean = false
): Response {
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
