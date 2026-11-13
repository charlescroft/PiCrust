#!/usr/bin/env node
import { mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const sidecarDir = dirname(fileURLToPath(import.meta.url));
const piGpuiDir = dirname(sidecarDir);
const repoRoot = dirname(piGpuiDir);
const apiRoot = join(repoRoot, "app", "api");
const distDir = join(piGpuiDir, "dist-sidecar");
const generatedEntry = join(sidecarDir, ".generated-entry.ts");
const nextShimPath = join(sidecarDir, "next-server-shim.ts");

function loadEsbuild() {
  const candidates = [
    process.env.ESBUILD_PATH,
    join(sidecarDir, "package.json"),
    join(repoRoot, "package.json"),
  ].filter(Boolean);

  for (const candidate of candidates) {
    try {
      const req = candidate.endsWith(".json")
        ? createRequire(candidate)
        : createRequire(import.meta.url);
      return candidate.endsWith(".json") ? req("esbuild") : req(candidate);
    } catch {
      // try next
    }
  }
  throw new Error(
    "esbuild not found. Install dependencies in repo root (`npm ci`) or inside `pi-gpui/sidecar` (`npm install esbuild`).",
  );
}

async function findRouteFiles(dir) {
  if (!existsSync(dir)) return [];
  const entries = await readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await findRouteFiles(full)));
    } else if (entry.isFile() && entry.name === "route.ts") {
      files.push(full);
    }
  }
  return files;
}

function parseRouteSegments(routeFile) {
  const relDir = relative(apiRoot, dirname(routeFile));
  const parts = relDir ? relDir.split(sep).filter(Boolean) : [];
  const segments = [{ kind: "static", value: "api" }];
  for (const part of parts) {
    if (part.startsWith("[[...") && part.endsWith("]]")) {
      segments.push({ kind: "optional-catchall", name: part.slice(5, -2) });
    } else if (part.startsWith("[...") && part.endsWith("]")) {
      segments.push({ kind: "catchall", name: part.slice(4, -1) });
    } else if (part.startsWith("[") && part.endsWith("]")) {
      segments.push({ kind: "param", name: part.slice(1, -1) });
    } else {
      segments.push({ kind: "static", value: part });
    }
  }
  return segments;
}

function routePriority(segments) {
  return segments.map((seg) => {
    if (seg.kind === "static") return `3_${seg.value}`;
    if (seg.kind === "param") return "2";
    if (seg.kind === "catchall") return "1";
    return "0";
  });
}

function compareRoutes(a, b) {
  const pa = routePriority(a.segments);
  const pb = routePriority(b.segments);
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const va = pa[i] ?? "";
    const vb = pb[i] ?? "";
    if (va !== vb) return vb.localeCompare(va);
  }
  return b.segments.length - a.segments.length;
}

async function main() {
  const esbuild = loadEsbuild();
  const routeFiles = await findRouteFiles(apiRoot);
  const routes = routeFiles.map((file, idx) => ({
    id: `route_${idx}`,
    file,
    importPath: relative(sidecarDir, file).split(sep).join("/").replace(/\.ts$/, ""),
    pattern:
      `/api/${relative(apiRoot, dirname(file)).split(sep).filter(Boolean).join("/")}`.replace(
        /\/$/,
        "",
      ) || "/api",
    segments: parseRouteSegments(file),
  }));

  routes.sort(compareRoutes);

  const lines = [
    `import { registerNodeInstrumentation } from ${JSON.stringify(relative(sidecarDir, join(repoRoot, "instrumentation-node")).split(sep).join("/"))};`,
  ];

  for (const r of routes) {
    lines.push(`import * as ${r.id} from ${JSON.stringify(r.importPath)};`);
  }

  lines.push("");
  lines.push("export interface RouteSegment {");
  lines.push('  kind: "static" | "param" | "catchall" | "optional-catchall";');
  lines.push("  value?: string;");
  lines.push("  name?: string;");
  lines.push("}");
  lines.push("");
  lines.push("export interface SidecarRoute {");
  lines.push("  pattern: string;");
  lines.push("  segments: RouteSegment[];");
  lines.push("  handlers: Record<string, (req: any, ctx: any) => Promise<Response> | Response>;");
  lines.push("}");
  lines.push("");
  lines.push("export const routes: SidecarRoute[] = [");
  for (const r of routes) {
    lines.push(
      `  { pattern: ${JSON.stringify(r.pattern)}, segments: ${JSON.stringify(r.segments)}, handlers: ${r.id} as any },`,
    );
  }
  lines.push("];");
  lines.push("");
  lines.push("export { registerNodeInstrumentation };");
  lines.push("");

  await mkdir(distDir, { recursive: true });
  await writeFile(generatedEntry, lines.join("\n"), "utf8");

  const start = Date.now();
  try {
    await esbuild.build({
      entryPoints: [generatedEntry],
      outfile: join(distDir, "server.cjs"),
      bundle: true,
      platform: "node",
      target: "node22",
      format: "cjs",
      sourcemap: false,
      logLevel: "info",
      absWorkingDir: repoRoot,
      alias: {
        "@": repoRoot,
        "next/server": nextShimPath,
        "next/server.js": nextShimPath,
      },
      external: [
        "@earendil-works/*",
        "jiti",
        "quickjs-wasi",
        "node-pty",
        "proper-lockfile",
        "undici",
        "ws",
        "mammoth",
        "js-yaml",
        "croner",
        "semver",
        "typebox",
        "web-push",
        "qrcode-generator",
        "esbuild",
      ],
      define: {
        "process.env.NEXT_RUNTIME": '"nodejs"',
      },
    });
  } finally {
    await rm(generatedEntry, { force: true });
  }

  const elapsed = Date.now() - start;
  console.log(
    `[pi-gpui sidecar] Built ${routes.length} API routes into ${join(distDir, "server.cjs")} in ${elapsed}ms`,
  );
}

main().catch((err) => {
  console.error("[pi-gpui sidecar] Build failed:", err);
  process.exit(1);
});
