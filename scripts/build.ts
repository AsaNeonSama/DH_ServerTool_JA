import type { BuildOutput, OnLoadArgs, OnLoadResult, PluginBuilder } from "bun";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

const projectRoot: string = resolve(import.meta.dir, "..");
const fridaBindingAbs: string = resolve(
  projectRoot,
  "node_modules",
  "frida",
  "build",
  "frida_binding.node"
);
const cacheDir: string = resolve(projectRoot, ".build-cache");
const distDir: string = resolve(projectRoot, "dist");
mkdirSync(cacheDir, { recursive: true });
mkdirSync(distDir, { recursive: true });

// Pre-compress frida_binding.node with zstd level 22 (~71.8 MB -> ~22.2 MB, cached by SHA-256)
const rawBinding: Buffer = readFileSync(fridaBindingAbs);
const expectedSize: number = rawBinding.byteLength;
const hashHex: string = new Bun.CryptoHasher("sha256")
  .update(rawBinding)
  .digest("hex")
  .slice(0, 12);
const compressedBindingAbs: string = resolve(
  cacheDir,
  `frida_binding_${hashHex}.node.zst`
);

if (!existsSync(compressedBindingAbs)) {
  console.log(
    `[build] Compressing frida_binding.node (${(expectedSize / 1048576).toFixed(2)} MB) with zstd level 22...`
  );
  const t0: number = performance.now();
  const compressed: Uint8Array = Bun.zstdCompressSync(rawBinding, { level: 22 });
  writeFileSync(compressedBindingAbs, compressed);
  const ratio: string = ((compressed.byteLength / expectedSize) * 100).toFixed(1);
  const elapsedSeconds: string = ((performance.now() - t0) / 1000).toFixed(2);
  console.log(
    `[build] Compressed -> ${(compressed.byteLength / 1048576).toFixed(2)} MB (${ratio}%) in ${elapsedSeconds}s`
  );
} else {
  const cachedSize: number = Bun.file(compressedBindingAbs).size;
  console.log(
    `[build] Using cached zstd frida binding: ${(cachedSize / 1048576).toFixed(2)} MB (${hashHex})`
  );
}

const outfile: string = resolve(distDir, "DH_ServerTool_JA.exe");
const iconPath: string = resolve(projectRoot, "assets", "icon.ico");

console.log("[build] Compiling standalone executable:", outfile);
console.log("[build] Embedding Windows icon:", iconPath);

const result: BuildOutput = await Bun.build({
  entrypoints: [resolve(projectRoot, "index.ts")],
  minify: true,
  compile: {
    outfile,
    windows: {
      icon: iconPath,
      title: "DH_ServerTool_JA",
      publisher: "アサネオン / Build With Antigravity",
      version: "2.0.0.0",
      description: "Dread Hunger Dedicated Server Controller & Web Console",
    },
  },
  plugins: [
    {
      name: "frida-zstd-native-binding",
      setup(build: PluginBuilder): void {
        build.onLoad(
          { filter: /node_modules[\\/]frida[\\/]build[\\/]src[\\/]frida\.js$/ },
          async (args: OnLoadArgs): Promise<OnLoadResult> => {
            let contents: string = await Bun.file(args.path).text();

            // Replace only the dynamic `bindings(...)` import and lookup with a zstd-compressed embedded loader
            // while preserving `util`, `Minimatch`, and `Duplex` imports untouched!
            const importRe: RegExp = /^import bindings from "bindings";\r?\n/m;
            const callRe: RegExp = /const binding = bindings\(\{[\s\S]*?\}\);/;

            if (!importRe.test(contents) || !callRe.test(contents)) {
              throw new Error(
                "Unexpected frida.js structure; could not patch bindings() loader."
              );
            }

            const loaderImport: string = [
              `import __fridaZstPath from ${JSON.stringify(compressedBindingAbs)} with { type: "file" };`,
              `import { createRequire as __createRequire } from "module";`,
              `import { existsSync as __existsSync, mkdirSync as __mkdirSync, readFileSync as __readFileSync, statSync as __statSync, writeFileSync as __writeFileSync, renameSync as __renameSync, unlinkSync as __unlinkSync } from "fs";`,
              `import { join as __join } from "path";`,
              `import { tmpdir as __tmpdir } from "os";`,
            ].join("\n") + "\n";

            const loaderBody: string = `const binding = (() => {
  const expectedSize = ${expectedSize};
  const fileName = "frida_binding_16.7.19_${hashHex}.node";
  const candidates = [];
  if (process.env.LOCALAPPDATA) candidates.push(__join(process.env.LOCALAPPDATA, "DH_ServerTool_JA"));
  candidates.push(__join(__tmpdir(), "DH_ServerTool_JA"));
  candidates.push(__join(process.cwd(), ".dh_cache"));

  const extractTo = (targetPath) => {
    const compressed = __readFileSync(__fridaZstPath);
    const decompressed = Bun.zstdDecompressSync(compressed);
    const tmpPath = targetPath + "." + process.pid + ".tmp";
    try {
      __writeFileSync(tmpPath, decompressed);
      __renameSync(tmpPath, targetPath);
    } catch {
      try { __unlinkSync(tmpPath); } catch {}
      if (!__existsSync(targetPath) || __statSync(targetPath).size !== expectedSize) {
        __writeFileSync(targetPath, decompressed);
      }
    }
  };

  let lastErr;
  for (const cacheDir of candidates) {
    try {
      __mkdirSync(cacheDir, { recursive: true });
      const targetPath = __join(cacheDir, fileName);
      let needExtract = true;
      if (__existsSync(targetPath)) {
        try {
          if (__statSync(targetPath).size === expectedSize) needExtract = false;
        } catch {}
      }
      if (needExtract) {
        extractTo(targetPath);
      }
      try {
        return __createRequire(import.meta.url)(targetPath);
      } catch (loadErr) {
        if (!needExtract) {
          extractTo(targetPath);
          return __createRequire(import.meta.url)(targetPath);
        }
        throw loadErr;
      }
    } catch (err) {
      lastErr = err;
    }
  }
  throw lastErr || new Error("Failed to extract frida native binding");
})();`;

            contents = contents.replace(importRe, loaderImport);
            contents = contents.replace(callRe, loaderBody);

            return { contents, loader: "js" };
          }
        );
      },
    },
  ],
});

if (!result.success) {
  console.error("[build] Build FAILED:");
  for (const log of result.logs) {
    console.error(log);
  }
  process.exit(1);
}

copyFileSync(
  resolve(projectRoot, "config.example.json"),
  resolve(distDir, "config.json")
);

const exeStat: Bun.BunFile = Bun.file(outfile);
console.log(
  `[build] Success! ${outfile} (${(exeStat.size / (1024 * 1024)).toFixed(2)} MB)`
);
console.log(`[build] Copied default config to ${resolve(distDir, "config.json")}`);
