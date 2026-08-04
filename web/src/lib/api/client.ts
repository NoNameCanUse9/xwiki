export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  /** Structured details from the server (e.g. the lock info on page_locked). */
  readonly data: unknown;

  constructor(status: number, code: string, message: string, data?: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.data = data;
  }
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string; data?: unknown };
}

const BASE = "/api/v1";

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...init.headers,
    },
  });
  if (!res.ok) {
    let code = "internal_error";
    let message = `请求失败（HTTP ${res.status}）`;
    let data: unknown;
    try {
      const body = (await res.json()) as ErrorEnvelope;
      code = body.error?.code ?? code;
      message = body.error?.message ?? message;
      data = body.error?.data;
    } catch {
      // non-JSON error body
    }
    throw new ApiError(res.status, code, message, data);
  }
  return (await res.json()) as T;
}
