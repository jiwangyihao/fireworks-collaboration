import { invoke } from "./tauri";
import type { HttpResponseOutput } from "./http";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_REDIRECTS = 5;
const DEFAULT_USER_AGENT = "fireworks-collaboration/tauri-fetch";

export interface TauriRequestInit extends RequestInit {
  timeoutMs?: number;
  maxRedirects?: number;
  forceRealSni?: boolean;
}

interface PreparedRequest {
  request: Request;
  timeoutMs: number;
  maxRedirects: number;
  forceRealSni: boolean;
  followRedirects: boolean;
  redirectMode: RequestRedirect;
}

interface SerializedRequest {
  url: string;
  method: string;
  headers: Record<string, string>;
  bodyBase64: string | null;
  timeoutMs: number;
  forceRealSni: boolean;
  followRedirects: boolean;
  maxRedirects: number;
}

const STATUS_TEXTS: Record<number, string> = {
  100: "Continue",
  101: "Switching Protocols",
  102: "Processing",
  103: "Early Hints",
  200: "OK",
  201: "Created",
  202: "Accepted",
  203: "Non-Authoritative Information",
  204: "No Content",
  205: "Reset Content",
  206: "Partial Content",
  207: "Multi-Status",
  208: "Already Reported",
  226: "IM Used",
  300: "Multiple Choices",
  301: "Moved Permanently",
  302: "Found",
  303: "See Other",
  304: "Not Modified",
  305: "Use Proxy",
  307: "Temporary Redirect",
  308: "Permanent Redirect",
  400: "Bad Request",
  401: "Unauthorized",
  402: "Payment Required",
  403: "Forbidden",
  404: "Not Found",
  405: "Method Not Allowed",
  406: "Not Acceptable",
  407: "Proxy Authentication Required",
  408: "Request Timeout",
  409: "Conflict",
  410: "Gone",
  411: "Length Required",
  412: "Precondition Failed",
  413: "Content Too Large",
  414: "URI Too Long",
  415: "Unsupported Media Type",
  416: "Range Not Satisfiable",
  417: "Expectation Failed",
  418: "I'm a teapot",
  421: "Misdirected Request",
  422: "Unprocessable Content",
  423: "Locked",
  424: "Failed Dependency",
  425: "Too Early",
  426: "Upgrade Required",
  428: "Precondition Required",
  429: "Too Many Requests",
  431: "Request Header Fields Too Large",
  451: "Unavailable For Legal Reasons",
  500: "Internal Server Error",
  501: "Not Implemented",
  502: "Bad Gateway",
  503: "Service Unavailable",
  504: "Gateway Timeout",
  505: "HTTP Version Not Supported",
  506: "Variant Also Negotiates",
  507: "Insufficient Storage",
  508: "Loop Detected",
  510: "Not Extended",
  511: "Network Authentication Required",
};

export async function tauriFetch(
  input: RequestInfo | URL,
  init?: TauriRequestInit,
): Promise<Response> {
  const prepared = prepareRequest(input, init);
  const { request, redirectMode } = prepared;

  if (request.signal?.aborted) {
    throw toAbortError();
  }

  const payload = await buildPayload(prepared);
  const invokePromise = invoke<HttpResponseOutput>("http_fake_request", { input: payload });

  const abortPromise = request.signal
    ? new Promise<never>((_, reject) => {
        const onAbort = () => {
          reject(toAbortError());
        };
        request.signal?.addEventListener("abort", onAbort, { once: true });
      })
    : null;

  let raw: HttpResponseOutput;
  try {
    raw = abortPromise ? await Promise.race([invokePromise, abortPromise]) : await invokePromise;
  } catch (error) {
    throw new TypeError(String(error));
  }

  if (redirectMode === "error" && raw.status >= 300 && raw.status < 400) {
    throw new TypeError("redirect was blocked");
  }

  const bodyBytes = decodeBase64(raw.bodyBase64);
  const responseInit: ResponseInit = {
    status: raw.status,
    statusText: canonicalStatusText(raw.status),
    headers: raw.headers,
  };
  const bodyInit: BodyInit | undefined = bodyBytes.length > 0 ? bodyBytes.slice().buffer : undefined;
  const response = new Response(bodyInit, responseInit);

  applyReadOnly(response, "url", computeFinalUrl(payload.url, raw.redirects));
  applyReadOnly(response, "redirected", raw.redirects.length > 0);

  return response;
}

export { tauriFetch as fetch };

function prepareRequest(input: RequestInfo | URL, init?: TauriRequestInit): PreparedRequest {
  const { timeoutMs, maxRedirects, forceRealSni, ...rest } = init ?? {};
  const request = new Request(input, rest);
  return {
    request,
    timeoutMs: timeoutMs ?? DEFAULT_TIMEOUT_MS,
    maxRedirects: maxRedirects ?? DEFAULT_MAX_REDIRECTS,
    forceRealSni: forceRealSni ?? false,
    followRedirects: shouldFollowRedirect(request.redirect),
    redirectMode: request.redirect,
  };
}

async function buildPayload(prepared: PreparedRequest): Promise<SerializedRequest> {
  const { request, timeoutMs, maxRedirects, followRedirects, forceRealSni } = prepared;
  const bodyBase64 = await serializeBody(request);
  const headers = headersToRecord(request.headers);
  ensureUserAgent(headers);
  return {
    url: request.url,
    method: request.method.toUpperCase(),
    headers,
    bodyBase64,
    timeoutMs,
    forceRealSni,
    followRedirects,
    maxRedirects,
  };
}

async function serializeBody(request: Request): Promise<string | null> {
  const method = request.method.toUpperCase();
  if (method === "GET" || method === "HEAD") {
    return null;
  }
  const clone = request.clone();
  if (!clone.body) {
    return null;
  }
  const buffer = await clone.arrayBuffer();
  if (buffer.byteLength === 0) {
    return "";
  }
  return encodeBase64(new Uint8Array(buffer));
}

function headersToRecord(headers: Headers): Record<string, string> {
  const record: Record<string, string> = {};
  for (const [key, value] of headers.entries()) {
    record[key] = value;
  }
  return record;
}

function ensureUserAgent(headers: Record<string, string>): void {
  const hasUserAgent = Object.keys(headers).some((key) => key.toLowerCase() === "user-agent");
  if (!hasUserAgent) {
    headers["user-agent"] = DEFAULT_USER_AGENT;
  }
}

function shouldFollowRedirect(mode: RequestRedirect | undefined): boolean {
  if (!mode || mode === "follow") {
    return true;
  }
  if (mode === "manual" || mode === "error") {
    return false;
  }
  return true;
}

function computeFinalUrl(initial: string, redirects: HttpResponseOutput["redirects"]): string {
  if (!redirects || redirects.length === 0) {
    return initial;
  }
  return redirects[redirects.length - 1].location;
}

function canonicalStatusText(status: number): string {
  return STATUS_TEXTS[status] ?? "";
}

function decodeBase64(value: string): Uint8Array {
  if (!value) {
    return new Uint8Array();
  }
  const BufferCtor = resolveBuffer();
  if (BufferCtor) {
    return Uint8Array.from(BufferCtor.from(value, "base64"));
  }
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function encodeBase64(bytes: Uint8Array): string {
  if (bytes.byteLength === 0) {
    return "";
  }
  const BufferCtor = resolveBuffer();
  if (BufferCtor) {
    return BufferCtor.from(bytes).toString("base64");
  }
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function applyReadOnly(target: Response, key: string, value: unknown): void {
  try {
    Object.defineProperty(target, key, {
      value,
      configurable: true,
    });
  } catch (_) {
    /* swallow */
  }
}

function toAbortError(): DOMException {
  return new DOMException("操作已中止", "AbortError");
}

function resolveBuffer(): { from: (...args: any[]) => any } | undefined {
  const globalBuffer = (globalThis as unknown as { Buffer?: { from: (...args: any[]) => any } }).Buffer;
  return typeof globalBuffer === "function" || typeof globalBuffer === "object" ? globalBuffer : undefined;
}

