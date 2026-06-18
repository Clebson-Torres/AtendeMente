import { API } from "./api-base";

// ─── State ───────────────────────────────────────────────────────────────

type AuthUser = { uid: string; email: string | null } | null;
type AuthCallback = (user: AuthUser) => void;

let currentUser: AuthUser = null;

let listeners: AuthCallback[] = [];

function notify(user: AuthUser) {
  currentUser = user;
  for (const cb of listeners) cb(user);
}

// ─── API helpers ─────────────────────────────────────────────────────────

async function apiRequest<T>(path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getStoredToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(`${API}${path}`, {
    method: body ? "POST" : "GET",
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  const json = await res.json();

  if (!res.ok || !json.success) {
    throw new Error(json.message || "Erro na requisição");
  }
  return json.data;
}

// ─── Token management (in-memory only — lost on app close) ───────────────

let _token: string | null = null;

// E2E support: allow injecting a token before page scripts run
// (set by Playwright fixture via page.addInitScript)
if (
  import.meta.env.DEV &&
  typeof window !== "undefined" &&
  (window as any).__E2E_TOKEN__
) {
  _token = (window as any).__E2E_TOKEN__;
}

function getStoredToken(): string | null {
  return _token;
}

function storeToken(token: string) {
  _token = token;
}

function clearToken() {
  _token = null;
}

// ─── Public API ──────────────────────────────────────────────────────────

export function onAuthChange(cb: AuthCallback): () => void {
  listeners.push(cb);
  cb(currentUser);
  return () => {
    listeners = listeners.filter((l) => l !== cb);
  };
}

export async function login(email: string, password: string): Promise<void> {
  const data = await apiRequest<{
    token: string;
    user_id: string;
    email: string;
    full_name: string;
  }>("/auth/login", { email, password });

  storeToken(data.token);
  notify({ uid: data.user_id, email: data.email });
}

/** Complete authentication from stored token after registration */
export async function completeFromStoredToken(): Promise<void> {
  const token = getStoredToken();
  if (!token) return;
  try {
    const data = await apiRequest<{ user_id: string; email: string }>("/auth/me");
    notify({ uid: data.user_id, email: data.email });
  } catch {
    clearToken();
  }
}

// Clean up any stale token in localStorage (legacy from Firebase)
localStorage.removeItem("token");

export async function register(
  email: string,
  password: string,
  fullName: string
): Promise<{ user_id: string; recovery_secret: string }> {
  const data = await apiRequest<{
    token: string;
    user_id: string;
    email: string;
    full_name: string;
    recovery_secret: string;
  }>("/auth/register", { email, password, full_name: fullName });

  storeToken(data.token);

  return {
    user_id: data.user_id,
    recovery_secret: data.recovery_secret,
  };
}

export async function logout() {
  const token = getStoredToken();
  if (token) {
    try {
      await apiRequest("/auth/logout");
    } catch {
      // Ignore errors, clear locally anyway
    }
  }
  clearToken();
  notify(null);
}

export async function lock(): Promise<void> {
  await apiRequest<void>("/auth/lock");
}

export async function unlock(password: string): Promise<void> {
  await apiRequest<void>("/auth/unlock", { password });
}

export function getCurrentToken(): string | null {
  return getStoredToken();
}

export async function recoverPassword(
  userId: string,
  recoverySecret: string
): Promise<string> {
  const data = await apiRequest<{ reset_token: string }>("/auth/recover", {
    user_id: userId,
    recovery_secret: recoverySecret,
  });
  return data.reset_token;
}

export async function resetPassword(
  resetToken: string,
  newPassword: string
): Promise<void> {
  await apiRequest("/auth/reset-password", {
    reset_token: resetToken,
    new_password: newPassword,
  });
}

// ─── Session restore ─────────────────────────────────────────────────────

export async function restoreSession(): Promise<void> {
  const token = getStoredToken();
  if (token) {
    return completeFromStoredToken();
  }
  notify(null);
}
