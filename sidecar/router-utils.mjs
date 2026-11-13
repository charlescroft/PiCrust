export function matchRouteSegments(segments, pathParts) {
  const params = {};
  let segIdx = 0;
  let pathIdx = 0;

  while (segIdx < segments.length) {
    const seg = segments[segIdx];
    if (seg.kind === "static") {
      if (pathIdx >= pathParts.length || pathParts[pathIdx] !== seg.value) {
        return null;
      }
      segIdx += 1;
      pathIdx += 1;
      continue;
    }

    if (seg.kind === "param") {
      if (pathIdx >= pathParts.length) return null;
      params[seg.name] = safeDecode(pathParts[pathIdx]);
      segIdx += 1;
      pathIdx += 1;
      continue;
    }

    if (seg.kind === "catchall") {
      if (pathIdx >= pathParts.length) return null;
      params[seg.name] = pathParts.slice(pathIdx).map(safeDecode);
      return params;
    }

    if (seg.kind === "optional-catchall") {
      const rest = pathParts.slice(pathIdx).map(safeDecode);
      params[seg.name] = rest.length > 0 ? rest : undefined;
      return params;
    }

    return null;
  }

  if (pathIdx !== pathParts.length) return null;
  return params;
}

export function matchRoute(routes, pathname) {
  const normalized = pathname === "/" ? [] : pathname.split("/").filter(Boolean);
  for (const route of routes) {
    const params = matchRouteSegments(route.segments, normalized);
    if (params !== null) {
      return { route, params };
    }
  }
  return null;
}

function safeDecode(part) {
  try {
    return decodeURIComponent(part);
  } catch {
    return part;
  }
}

export function toWebHeaders(rawHeaders) {
  const headers = new Headers();
  for (const [key, value] of Object.entries(rawHeaders)) {
    if (value === undefined) continue;
    if (Array.isArray(value)) {
      for (const v of value) headers.append(key, v);
    } else {
      headers.set(key, String(value));
    }
  }
  return headers;
}
