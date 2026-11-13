import test from "node:test";
import assert from "node:assert/strict";
import { matchRoute, toWebHeaders } from "./router-utils.mjs";

const sampleRoutes = [
  {
    pattern: "/api/sessions/search",
    segments: [
      { kind: "static", value: "api" },
      { kind: "static", value: "sessions" },
      { kind: "static", value: "search" },
    ],
    handlers: {},
  },
  {
    pattern: "/api/sessions/[id]/context",
    segments: [
      { kind: "static", value: "api" },
      { kind: "static", value: "sessions" },
      { kind: "param", name: "id" },
      { kind: "static", value: "context" },
    ],
    handlers: {},
  },
  {
    pattern: "/api/sessions/[id]",
    segments: [
      { kind: "static", value: "api" },
      { kind: "static", value: "sessions" },
      { kind: "param", name: "id" },
    ],
    handlers: {},
  },
  {
    pattern: "/api/files/[...path]",
    segments: [
      { kind: "static", value: "api" },
      { kind: "static", value: "files" },
      { kind: "catchall", name: "path" },
    ],
    handlers: {},
  },
];

test("matchRoute matches static before param", () => {
  const res = matchRoute(sampleRoutes, "/api/sessions/search");
  assert.equal(res?.route.pattern, "/api/sessions/search");
  assert.deepEqual(res?.params, {});
});

test("matchRoute extracts decoded param segments", () => {
  const res = matchRoute(sampleRoutes, "/api/sessions/abc%20123/context");
  assert.equal(res?.route.pattern, "/api/sessions/[id]/context");
  assert.deepEqual(res?.params, { id: "abc 123" });
});

test("matchRoute extracts catchall segments", () => {
  const res = matchRoute(sampleRoutes, "/api/files/Users/charles/src/main.rs");
  assert.equal(res?.route.pattern, "/api/files/[...path]");
  assert.deepEqual(res?.params, { path: ["Users", "charles", "src", "main.rs"] });
});

test("matchRoute returns null when unmatched", () => {
  assert.equal(matchRoute(sampleRoutes, "/api/unknown"), null);
  assert.equal(matchRoute(sampleRoutes, "/api/files"), null);
});

test("toWebHeaders preserves single and multi-value headers", () => {
  const headers = toWebHeaders({
    "x-pi-desktop-token": "secret",
    "x-multi": ["a", "b"],
    "x-ignored": undefined,
  });
  assert.equal(headers.get("x-pi-desktop-token"), "secret");
  assert.equal(headers.get("x-multi"), "a, b");
  assert.equal(headers.has("x-ignored"), false);
});
