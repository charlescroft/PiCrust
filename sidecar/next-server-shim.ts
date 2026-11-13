export interface RequestCookie {
  name: string;
  value: string;
}

class RequestCookies {
  private readonly map: Map<string, string>;

  constructor(cookieHeader: string | null) {
    this.map = new Map();
    if (!cookieHeader) return;
    for (const part of cookieHeader.split(";")) {
      const idx = part.indexOf("=");
      if (idx === -1) continue;
      const name = part.slice(0, idx).trim();
      const value = part.slice(idx + 1).trim();
      if (name) {
        try {
          this.map.set(name, decodeURIComponent(value));
        } catch {
          this.map.set(name, value);
        }
      }
    }
  }

  get(name: string): RequestCookie | undefined {
    const value = this.map.get(name);
    return value === undefined ? undefined : { name, value };
  }

  has(name: string): boolean {
    return this.map.has(name);
  }

  getAll(): RequestCookie[] {
    return Array.from(this.map.entries(), ([name, value]) => ({ name, value }));
  }
}

export class NextRequest extends Request {
  readonly nextUrl: URL;
  readonly cookies: RequestCookies;

  constructor(input: RequestInfo | URL, init?: RequestInit) {
    super(input, init);
    this.nextUrl = new URL(this.url);
    this.cookies = new RequestCookies(this.headers.get("cookie"));
  }
}

export class NextResponse extends Response {
  static json<T>(body: T, init?: ResponseInit): NextResponse {
    const headers = new Headers(init?.headers);
    if (!headers.has("content-type")) {
      headers.set("content-type", "application/json");
    }
    return new NextResponse(JSON.stringify(body), {
      ...init,
      headers,
    });
  }

  static next(init?: ResponseInit): NextResponse {
    const headers = new Headers(init?.headers);
    headers.set("x-middleware-next", "1");
    return new NextResponse(null, {
      status: 200,
      ...init,
      headers,
    });
  }

  static redirect(url: string | URL, init?: number | ResponseInit): NextResponse {
    const status = typeof init === "number" ? init : init?.status ?? 307;
    const headers = new Headers(typeof init === "number" ? undefined : init?.headers);
    headers.set("location", url.toString());
    return new NextResponse(null, {
      ...(typeof init === "number" ? {} : init),
      status,
      headers,
    });
  }
}
