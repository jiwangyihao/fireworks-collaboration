import { invoke } from "./tauri";

export interface HttpRequestInput {
  url: string;
  method: string;
  headers: Record<string, string>;
  bodyBase64?: string | null;
  timeoutMs: number;
  forceRealSni: boolean;
  followRedirects: boolean;
  maxRedirects: number;
}

export interface TimingInfo {
  connectMs: number; tlsMs: number; firstByteMs: number; totalMs: number;
}
export interface RedirectInfo { status: number; location: string; count: number }
export interface HttpResponseOutput {
  ok: boolean; status: number; headers: Record<string, string>;
  bodyBase64: string; usedFakeSni: boolean; ip?: string | null;
  timing: TimingInfo; redirects: RedirectInfo[]; bodySize: number;
}

export async function httpFakeRequest(req: HttpRequestInput): Promise<HttpResponseOutput> {
  return invoke<HttpResponseOutput>("http_fake_request", { input: req });
}
