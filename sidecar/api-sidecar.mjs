#!/usr/bin/env node
import http from "node:http";
import { existsSync } from "node:fs";
import Module, { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";
import { fileURLToPath } from "node:url";
import { matchRoute, toWebHeaders } from "./router-utils.mjs";

const sidecarDir = dirname(fileURLToPath(import.meta.url));
const piGpuiDir = dirname(sidecarDir);
const repoRoot = dirname(piGpuiDir);
const bundlePath = process.env.PI_SIDECAR_BUNDLE || join(piGpuiDir, "dist-sidecar", "server.cjs");

const extraNodePaths = [
  process.env.PI_SIDECAR_NODE_PATH,
  join(repoRoot, "node_modules"),
  "/Applications/Pi Agent.app/Contents/Resources/resources/server/node_modules",
].filter((p) => p && existsSync(p));

if (extraNodePaths.length > 0) {
  const existing = process.env.NODE_PATH ? process.env.NODE_PATH.split(":") : [];
  process.env.NODE_PATH = [...extraNodePaths, ...existing].join(":");
  Module._initPaths();
}

const HOST = process.env.PI_SIDECAR_HOST || "127.0.0.1";
const PORT = Number(process.env.PI_SIDECAR_PORT || "38471");
const EXPECTED_TOKEN = process.env.PI_DESKTOP_API_TOKEN || "";
const INSTANCE_ID = process.env.PI_DESKTOP_INSTANCE_ID || "";
const PARENT_PID = process.env.PI_PARENT_PID ? parseInt(process.env.PI_PARENT_PID, 10) : null;

// Auto-terminate when parent GPUI process exits to prevent orphaned processes and port collisions
if (PARENT_PID && !isNaN(PARENT_PID)) {
  const watchParent = () => {
    try {
      process.kill(PARENT_PID, 0);
    } catch {
      process.exit(0);
    }
  };
  setInterval(watchParent, 2000).unref();
}

process.on("SIGTERM", () => process.exit(0));
process.on("SIGINT", () => process.exit(0));

async function loadSidecarBundle() {
  const mjsCandidate = join(piGpuiDir, "dist-sidecar", "server.mjs");
  if (existsSync(mjsCandidate)) {
    const { pathToFileURL } = await import("node:url");
    return await import(pathToFileURL(mjsCandidate).href);
  }
  if (!existsSync(bundlePath)) {
    throw new Error(
      `Sidecar bundle not found at ${bundlePath}. Run: node pi-gpui/sidecar/build-sidecar.mjs`,
    );
  }
  if (bundlePath.endsWith(".mjs")) {
    const { pathToFileURL } = await import("node:url");
    return await import(pathToFileURL(bundlePath).href);
  }
  const require = createRequire(import.meta.url);
  return require(bundlePath);
}

async function writeWebResponse(res, webRes) {
  const headerEntries = {};
  const setCookies = typeof webRes.headers.getSetCookie === "function"
    ? webRes.headers.getSetCookie()
    : [];

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
      console.error("[pi-gpui sidecar] Stream error:", err);
    }
  } finally {
    if (!res.writableEnded) {
      res.end();
    }
  }
}

function sendJson(res, status, data) {
  const body = JSON.stringify(data);
  res.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(body),
    "cache-control": "no-store",
  });
  res.end(body);
}

function isAuthorizedDesktopRequest(req) {
  // Allow all loopback requests from local machine without token friction
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
    registerNodeInstrumentation();
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
    const handler = matched.route.handlers[method]
      || (method === "HEAD" ? matched.route.handlers.GET : undefined);

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
      // Expose NextRequest-compatible fields in case any handler reads them
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
  server.headersTimeout = 60_000;

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
