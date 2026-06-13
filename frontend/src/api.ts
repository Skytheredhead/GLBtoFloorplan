import type {
  AuthResponse,
  FloorplanDetail,
  FloorplanSummary,
  MeResponse,
  UploadResponse,
} from "./types";

export const API_BASE = (
  import.meta.env.VITE_API_BASE_URL || "https://floorplanapi.skylarenns.com"
).replace(/\/$/, "");

export class ApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

async function request<T>(
  path: string,
  options: RequestInit = {},
  token?: string | null,
): Promise<T> {
  const headers = new Headers(options.headers);
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  if (!(options.body instanceof FormData) && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
    credentials: "include",
  });

  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try {
      const body = (await response.json()) as { error?: string };
      message = body.error || message;
    } catch {
      // Keep the generic message when the server did not send JSON.
    }
    throw new ApiError(message, response.status);
  }

  return response.json() as Promise<T>;
}

export function loginWithGoogle(idToken: string) {
  return request<AuthResponse>("/api/auth/google", {
    method: "POST",
    body: JSON.stringify({ id_token: idToken }),
  });
}

export function getMe(token: string) {
  return request<MeResponse>("/api/me", {}, token);
}

export function listFloorplans(token: string) {
  return request<FloorplanSummary[]>("/api/floorplans", {}, token);
}

export function getFloorplan(token: string, id: string) {
  return request<FloorplanDetail>(`/api/floorplans/${id}`, {}, token);
}

export function uploadFloorplan(token: string, file: File) {
  const form = new FormData();
  form.append("file", file);
  return request<UploadResponse>(
    "/api/floorplans",
    {
      method: "POST",
      body: form,
    },
    token,
  );
}

export function withToken(path: string | undefined, token: string | null) {
  if (!path || !token) return undefined;
  const separator = path.includes("?") ? "&" : "?";
  return `${API_BASE}${path}${separator}token=${encodeURIComponent(token)}`;
}
