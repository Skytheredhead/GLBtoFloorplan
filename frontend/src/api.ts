import type {
  FloorplanDetail,
  Quota,
  UploadResponse,
} from "./types";

export const API_BASE = (
  import.meta.env.VITE_API_BASE_URL || "http://localhost:8080"
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
): Promise<T> {
  const headers = new Headers(options.headers);
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

export function getQuota() {
  return request<Quota>("/api/quota");
}

export function getFloorplan(id: string) {
  return request<FloorplanDetail>(`/api/floorplans/${id}`);
}

export function uploadFloorplan(file: File) {
  const form = new FormData();
  form.append("file", file);
  return request<UploadResponse>(
    "/api/floorplans",
    {
      method: "POST",
      body: form,
    },
  );
}

export function apiUrl(path: string | undefined) {
  if (!path) return undefined;
  return `${API_BASE}${path}`;
}
