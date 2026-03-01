/**
 * REST API fetch wrapper.
 *
 * Uses relative paths so the Vite dev proxy (/api → :18790) works
 * transparently. In production, the Rust server serves both API and frontend.
 */

export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

export type FetchParams = Record<
  string,
  string | number | boolean | string[] | undefined | null
>;

export interface FetchOptions extends Omit<RequestInit, 'body'> {
  body?: unknown;
  params?: FetchParams;
}

/**
 * Typed fetch wrapper. Returns parsed JSON on success; throws ApiError on
 * non-2xx responses.
 */
export async function apiFetch<T>(
  path: string,
  options: FetchOptions = {},
): Promise<T> {
  const { body, params, method = body !== undefined ? 'POST' : 'GET', ...rest } = options;

  // Build URL with query params
  let url = path;
  if (params) {
    const searchParams = new URLSearchParams();
    for (const [key, value] of Object.entries(params)) {
      if (value === undefined || value === null) continue;
      if (Array.isArray(value)) {
        if (value.length > 0) {
          searchParams.set(key, value.join(','));
        }
      } else {
        searchParams.set(key, String(value));
      }
    }
    const queryString = searchParams.toString();
    if (queryString) {
      url = `${path}?${queryString}`;
    }
  }

  const fetchInit: RequestInit = { method, ...rest };

  if (body !== undefined) {
    fetchInit.body = JSON.stringify(body);
    fetchInit.headers = {
      'Content-Type': 'application/json',
      ...fetchInit.headers,
    };
  }

  const response = await fetch(url, fetchInit);

  if (!response.ok) {
    let message = `HTTP ${response.status}`;
    try {
      const errorBody = (await response.json()) as { message?: string };
      if (errorBody.message) {
        message = errorBody.message;
      }
    } catch {
      // Ignore JSON parse errors — keep the default message
    }
    throw new ApiError(response.status, message);
  }

  // 202 Accepted / 204 No Content have no body — return undefined
  if (response.status === 202 || response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}
