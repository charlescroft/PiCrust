#!/usr/bin/env node
import http from "node:http";
import { existsSync } from "node:fs";
import Module, { createRequire } from "node:module";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { Readable } from "node:stream";
import { fileURLToPath, pathToFileURL } from "node:url";
import { matchRoute, toWebHeaders } from "./router-utils.mjs";

// Prevent transient stream/socket/undici errors from crashing the persistent sidecar daemon
process.on("uncaughtException", (err) => {
  console.error("[pi-gpui sidecar] Uncaught exception (kept alive):", err?.message || err);
});
process.on("unhandledRejection", (reason) => {
  console.error("[pi-gpui sidecar] Unhandled rejection (kept alive):", reason?.message || reason);
});

// Keep RPC sessions alive indefinitely; never evict idle sessions
if (!process.env.PI_WEB_IDLE_TIMEOUT_MS) {
  process.env.PI_WEB_IDLE_TIMEOUT_MS = "0";
}

const sidecarDir = dirname(fileURLToPath(import.meta.url));
const piGpuiDir = dirname(sidecarDir);
const repoRoot = dirname(piGpuiDir);

const candidateNodeDirs = [
  process.env.PI_SIDECAR_NODE_PATH,
  join(sidecarDir, "node_modules"),
  join(piGpuiDir, "node_modules"),
  join(repoRoot, "node_modules"),
  join(homedir(), ".pi", "agent", "npm", "node_modules"),
  join(dirname(process.execPath), "..", "lib", "node_modules"),
  join(dirname(process.execPath), "node_modules"),
  process.env.APPDATA ? join(process.env.APPDATA, "npm", "node_modules") : null,
  process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, "npm", "node_modules") : null,
  "/opt/homebrew/lib/node_modules",
  "/usr/local/lib/node_modules",
  "/Applications/Pi Agent.app/Contents/Resources/resources/server/node_modules",
].filter((p) => p && existsSync(p));

const extraNodePaths = [];
for (const dir of candidateNodeDirs) {
  if (!extraNodePaths.includes(dir)) {
    extraNodePaths.push(dir);
  }
  const nestedPiModules = join(dir, "@earendil-works", "pi-coding-agent", "node_modules");
  if (existsSync(nestedPiModules) && !extraNodePaths.includes(nestedPiModules)) {
    extraNodePaths.push(nestedPiModules);
  }
}

if (extraNodePaths.length > 0) {
  const existing = process.env.NODE_PATH ? process.env.NODE_PATH.split(delimiter).filter(Boolean) : [];
  process.env.NODE_PATH = [...new Set([...extraNodePaths, ...existing])].join(delimiter);
  Module._initPaths();
}

// Register ESM resolution fallback so packaged .app / .exe / .AppImage bundles resolve
// external packages from global/system node_modules without requiring a local symlink.
if (extraNodePaths.length > 0) {
  const webPushStubUrl =
    "data:text/javascript,export default { setVapidDetails(){}, sendNotification: async () => ({}), generateVAPIDKeys: () => ({ publicKey: '', privateKey: '' }) };";

  if (typeof Module.registerHooks === "function") {
    Module.registerHooks({
      resolve(specifier, context, nextResolve) {
        try {
          return nextResolve(specifier, context);
        } catch (err) {
          if (specifier === "web-push") {
            return { url: webPushStubUrl, shortCircuit: true };
          }
          for (const dir of extraNodePaths) {
            try {
              return nextResolve(specifier, {
                ...context,
                parentURL: pathToFileURL(join(dir, "__picrust_resolve__.js")).href,
              });
            } catch {
              // try next candidate
            }
          }
          throw err;
        }
      },
    });
  } else if (typeof Module.register === "function") {
    const loaderSource = `
import { pathToFileURL } from "node:url";
import { join } from "node:path";
const searchDirs = ${JSON.stringify(extraNodePaths)};
const webPushStubUrl = ${JSON.stringify(webPushStubUrl)};
export async function resolve(specifier, context, nextResolve) {
  try {
    return await nextResolve(specifier, context);
  } catch (err) {
    if (specifier === "web-push") {
      return { url: webPushStubUrl, shortCircuit: true };
    }
    for (const dir of searchDirs) {
      try {
        return await nextResolve(specifier, {
          ...context,
          parentURL: pathToFileURL(join(dir, "__picrust_resolve__.js")).href,
        });
      } catch {}
    }
    throw err;
  }
}
`;
    Module.register(`data:text/javascript,${encodeURIComponent(loaderSource)}`);
  }
}

const HOST = process.env.PI_SIDECAR_HOST || "127.0.0.1";
const PORT = Number(process.env.PI_SIDECAR_PORT || "38471");
const EXPECTED_TOKEN = process.env.PI_DESKTOP_API_TOKEN || "";
const INSTANCE_ID = process.env.PI_DESKTOP_INSTANCE_ID || "";
const PARENT_PID = process.env.PI_PARENT_PID ? parseInt(process.env.PI_PARENT_PID, 10) : null;

// Auto-terminate when parent GPUI process exits to prevent orphaned processes and port collisions
if (process.platform === "win32") {
  // On Windows, stdin pipe closure is the canonical, non-failing mechanism for child process exit
  process.stdin.resume();
  process.stdin.on("end", () => process.exit(0));
  process.stdin.on("close", () => process.exit(0));
} else if (PARENT_PID && !isNaN(PARENT_PID)) {
  const watchParent = () => {
    try {
      process.kill(PARENT_PID, 0);
    } catch (err) {
      if (err && err.code === "ESRCH") {
        process.exit(0);
      }
    }
  };
  setInterval(watchParent, 3000).unref();
}

process.on("SIGTERM", () => process.exit(0));
process.on("SIGINT", () => process.exit(0));

async function loadSidecarBundle() {
  const candidates = [
    process.env.PI_SIDECAR_BUNDLE,
    join(sidecarDir, "server.mjs"),
    join(piGpuiDir, "dist-sidecar", "server.mjs"),
    join(sidecarDir, "server.cjs"),
    join(piGpuiDir, "dist-sidecar", "server.cjs"),
  ].filter((p) => p && existsSync(p));

  if (candidates.length === 0) {
    throw new Error(
      `Sidecar bundle not found. Checked: ${[
        process.env.PI_SIDECAR_BUNDLE,
        join(sidecarDir, "server.mjs"),
        join(piGpuiDir, "dist-sidecar", "server.mjs"),
      ]
        .filter(Boolean)
        .join(", ")}`,
    );
  }

  const chosen = candidates[0];
  if (chosen.endsWith(".mjs")) {
    return await import(pathToFileURL(chosen).href);
  }
  const require = createRequire(import.meta.url);
  return require(chosen);
}

async function writeWebResponse(res, webRes) {
  const headerEntries = {};
  const setCookies =
    typeof webRes.headers.getSetCookie === "function" ? webRes.headers.getSetCookie() : [];

  for (const [k, v] of webRes.headers.entries()) {
    if (k.toLowerCase() === "set-cookie") continue;
    headerEntries[k] = v;
  }
  if (setCookies.length > 0) {
    headerEntries["set-cookie"] = setCookies;
  }

  res.writeHead(webRes.status, webRes.statusText || undefined, headerEntries);

  if (!webRes.body) {
    res.end();
    return;
  }

  const reader = webRes.body.getReader();
  let closed = false;
  res.on("close", () => {
    closed = true;
    reader.cancel().catch(() => {});
  });

  try {
    while (!closed) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value && value.byteLength > 0) {
        const ok = res.write(value);
        if (!ok && !closed) {
          await new Promise((resolve) => res.once("drain", resolve));
        }
      }
    }
  } catch (err) {
    if (!closed) {
      console.error("[pi-gpui sidecar] Stream error:", err?.message || err);
    }
  } finally {
    if (!res.writableEnded) {
      res.end();
    }
  }
}

function sendJson(res, status, data) {
  if (res.writableEnded) return;
  const body = JSON.stringify(data);
  if (!res.headersSent) {
    res.writeHead(status, {
      "content-type": "application/json",
      "content-length": Buffer.byteLength(body),
      "cache-control": "no-store",
    });
  }
  res.end(body);
}

function isAuthorizedDesktopRequest(req) {
  const remote = req.socket?.remoteAddress;
  if (
    remote === "127.0.0.1" ||
    remote === "::1" ||
    remote === "::ffff:127.0.0.1" ||
    remote === "localhost"
  ) {
    return true;
  }
  if (!EXPECTED_TOKEN) return true;
  const token = req.headers["x-pi-desktop-token"];
  return typeof token === "string" && token === EXPECTED_TOKEN;
}

async function start() {
  const { routes, registerNodeInstrumentation } = await loadSidecarBundle();
  if (typeof registerNodeInstrumentation === "function") {
    try {
      registerNodeInstrumentation();
    } catch (err) {
      console.error("[pi-gpui sidecar] Instrumentation warning:", err?.message || err);
    }
  }

  const startedAt = Date.now();
  const server = http.createServer(async (req, res) => {
    const rawUrl = req.url || "/";
    const baseUrl = `http://${HOST}:${PORT}`;
    let parsedUrl;
    try {
      parsedUrl = new URL(rawUrl, baseUrl);
    } catch {
      sendJson(res, 400, { error: "Invalid request URL" });
      return;
    }

    if (parsedUrl.pathname === "/health") {
      sendJson(res, 200, {
        ok: true,
        pid: process.pid,
        instanceId: INSTANCE_ID || null,
        uptimeMs: Date.now() - startedAt,
        routes: routes.length,
      });
      return;
    }

    if (!isAuthorizedDesktopRequest(req)) {
      sendJson(res, 403, { error: "Forbidden: invalid x-pi-desktop-token" });
      return;
    }

    const matched = matchRoute(routes, parsedUrl.pathname);
    if (!matched) {
      sendJson(res, 404, { error: `Route not found: ${parsedUrl.pathname}` });
      return;
    }

    const method = (req.method || "GET").toUpperCase();
    const handler =
      matched.route.handlers[method] ||
      (method === "HEAD" ? matched.route.handlers.GET : undefined);

    if (typeof handler !== "function") {
      sendJson(res, 405, { error: `Method ${method} not allowed` });
      return;
    }

    const ac = new AbortController();
    req.on("aborted", () => ac.abort());
    res.on("close", () => {
      if (!res.writableEnded) ac.abort();
    });

    try {
      const hasBody = method !== "GET" && method !== "HEAD";
      const webReqInit = {
        method,
        headers: toWebHeaders(req.headers),
        signal: ac.signal,
        ...(hasBody ? { body: Readable.toWeb(req), duplex: "half" } : {}),
      };
      const webReq = new Request(parsedUrl.toString(), webReqInit);
      Object.defineProperty(webReq, "nextUrl", { value: parsedUrl, enumerable: false });

      const webRes = await handler(webReq, { params: Promise.resolve(matched.params) });
      await writeWebResponse(res, webRes);
    } catch (err) {
      if (ac.signal.aborted) return;
      console.error(`[pi-gpui sidecar] Error in ${method} ${parsedUrl.pathname}:`, err);
      if (!res.headersSent) {
        sendJson(res, 500, {
          error: err instanceof Error ? err.message : "Internal sidecar error",
        });
      } else if (!res.writableEnded) {
        res.end();
      }
    }
  });

  server.requestTimeout = 0;
  server.keepAliveTimeout = 65_000;
  server.headersTimeout = 70_000;

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(PORT, HOST, resolve);
  });

  console.log(`[pi-gpui sidecar] Listening on http://${HOST}:${PORT} (${routes.length} routes)`);

  const shutdown = () => {
    server.close(() => process.exit(0));
    setTimeout(() => process.exit(0), 1500).unref();
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}

start().catch((err) => {
  console.error("[pi-gpui sidecar] Startup failed:", err);
  process.exit(1);
});
