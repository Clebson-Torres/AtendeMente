import { API } from "./api-base";

// ─── State ───────────────────────────────────────────────────────────────

export interface AuthUserInfo {
  uid: string;
  email: string | null;
  onboarding_completed: boolean;
  /**
   * Sessão válida, mas sem a chave de dados carregada no backend.
   *
   * O token vive no sessionStorage e sobrevive a um F5; o cache de chaves morre
   * com o processo do backend. Nesse estado a tela abria "logada" e os campos de
   * PII vinham vazios, como se o paciente não tivesse telefone nem histórico — e
   * uma edição a partir dali gravaria o vazio. Só `/auth/me` informa isso.
   */
  locked?: boolean;
  /**
   * A conta não tem segunda via da chave de dados.
   *
   * Quem vem de versão anterior tem um código de recuperação válido mas nenhum
   * envelope, porque um envelope só nasce do segredo em claro e num login comum
   * o servidor só tem o hash. Enquanto for assim, essa conta depende só da
   * senha — e esquecê-la, depois da rotação de chave, deixa o prontuário
   * inacessível para sempre.
   */
  recovery_wrap_missing?: boolean;
  /**
   * A chave de dados ainda é a derivada do pepper da máquina, ou há uma rotação
   * pela metade.
   *
   * Enquanto for assim, quem tem a conta do sistema operacional abre os
   * prontuários sem saber a senha — foi isso que a reforma existe para fechar.
   */
  key_rotation_pending?: boolean;
}

type AuthUser = AuthUserInfo | null;
type AuthCallback = (user: AuthUser) => void;

let currentUser: AuthUser = null;

let listeners: AuthCallback[] = [];

function notify(user: AuthUser) {
  currentUser = user;
  for (const cb of listeners) cb(user);
}

// ─── API helpers ─────────────────────────────────────────────────────────

/**
 * O método é EXPLÍCITO, e isso não é preciosismo.
 *
 * Antes ele era inferido de `body ? "POST" : "GET"`, e três rotas `POST` sem
 * corpo acabavam sendo chamadas com GET. O Axum responde 405 com **corpo
 * vazio**, então `res.json()` estourava com "Unexpected end of JSON input" — uma
 * mensagem que não aponta para nada. Pior: onde o erro era engolido, a falha
 * ficava invisível.
 *
 * O que estava quebrado por causa disso:
 *
 * - `POST /auth/lock` — o bloqueio por inatividade nunca limpava a chave no
 *   servidor. O overlay subia de qualquer forma porque o erro era ignorado, então
 *   a tela parecia bloqueada enquanto a chave seguia na memória do processo.
 * - `POST /auth/logout` — a sessão nunca era revogada no banco e a chave nunca
 *   saía da memória; o token era apagado só localmente.
 * - `POST /auth/recovery-code/ack` — quebrava na cara do usuário.
 */
async function apiRequest<T>(
  path: string,
  body?: unknown,
  method: "GET" | "POST" | "PATCH" = body ? "POST" : "GET"
): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getStoredToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(`${API}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  // Uma resposta sem corpo — 405, 502, um proxy no meio — não pode virar
  // "Unexpected end of JSON input", que não aponta para nada. O status é a
  // informação que resolve, então ele entra na mensagem.
  let json: { success?: boolean; message?: string; data?: T };
  try {
    json = await res.json();
  } catch {
    throw new Error(
      res.ok
        ? "O servidor respondeu sem conteúdo."
        : `O servidor recusou a requisição (HTTP ${res.status}).`
    );
  }

  if (!res.ok || !json.success) {
    throw new Error(json.message || "Erro na requisição");
  }
  return json.data as T;
}

// ─── Token management (sessionStorage — persists on F5, cleared on tab close) ──

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
  if (_token) return _token;
  if (typeof window !== "undefined") {
    _token = sessionStorage.getItem("token");
  }
  return _token;
}

function storeToken(token: string) {
  _token = token;
  if (typeof window !== "undefined") {
    sessionStorage.setItem("token", token);
  }
}

function clearToken() {
  _token = null;
  if (typeof window !== "undefined") {
    sessionStorage.removeItem("token");
  }
}

// ─── Pending recovery secret (for onboarding after register) ────────────

let _pendingRecoverySecret: string | null = null;

export function getPendingRecoverySecret(): string | null {
  return _pendingRecoverySecret;
}

function setPendingRecoverySecret(secret: string) {
  _pendingRecoverySecret = secret;
}

export function clearPendingRecoverySecret() {
  _pendingRecoverySecret = null;
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
    onboarding_completed: boolean;
  }>("/auth/login", { email, password });

  storeToken(data.token);
  notify({
    uid: data.user_id,
    email: data.email,
    onboarding_completed: data.onboarding_completed,
  });

  // O `/auth/login` não informa o estado da chave, e é justamente no login que o
  // envelope é criado para quem vem de versão anterior. Sem esta chamada, os
  // avisos de segurança só apareciam depois de um F5 — foi por isso que os dois
  // cartões pendentes surgiram "do nada" ao importar um backup: o import
  // recarrega a tela e aí sim o /auth/me rodava.
  //
  // Best-effort: se falhar, o login continua válido e o aviso aparece na próxima
  // navegação. Bloquear a entrada por causa de um indicador seria pior.
  try {
    await completeFromStoredToken();
  } catch {
    // silencioso de propósito
  }
}

/** Complete authentication from stored token after registration */
export async function completeFromStoredToken(): Promise<void> {
  const token = getStoredToken();
  if (!token) return;
  try {
    const data = await apiRequest<{
      user_id: string;
      email: string;
      onboarding_completed: boolean;
      locked?: boolean;
      recovery_wrap_missing?: boolean;
      key_rotation_pending?: boolean;
    }>("/auth/me");
    notify({
      uid: data.user_id,
      email: data.email,
      onboarding_completed: data.onboarding_completed,
      locked: data.locked === true,
      recovery_wrap_missing: data.recovery_wrap_missing === true,
      key_rotation_pending: data.key_rotation_pending === true,
    });
  } catch {
    clearToken();
  }
}

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
    onboarding_completed: boolean;
  }>("/auth/register", { email, password, full_name: fullName });

  storeToken(data.token);
  setPendingRecoverySecret(data.recovery_secret);
  notify({
    uid: data.user_id,
    email: data.email,
    onboarding_completed: data.onboarding_completed,
  });

  return {
    user_id: data.user_id,
    recovery_secret: data.recovery_secret,
  };
}

export async function completeOnboarding(): Promise<void> {
  const token = getStoredToken();
  if (token) {
    try {
      const res = await fetch(`${API}/auth/onboarding`, {
        method: "PATCH",
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      });
      if (!res.ok) {
        console.warn("Onboarding completion not persisted on server (PATCH returned %d)", res.status);
      }
    } catch {
      console.warn("Onboarding completion PATCH request failed (network/CORS)");
    }
  }

  if (currentUser) {
    notify({ ...currentUser, onboarding_completed: true });
  }

  clearPendingRecoverySecret();
}

export async function logout() {
  const token = getStoredToken();
  if (token) {
    try {
      await apiRequest("/auth/logout", undefined, "POST");
    } catch {
      // Ignore errors, clear locally anyway
    }
  }
  clearToken();
  notify(null);
}

export async function lock(): Promise<void> {
  await apiRequest<void>("/auth/lock", undefined, "POST");
}

export async function unlock(password: string): Promise<void> {
  await apiRequest<void>("/auth/unlock", { password });
}

export function getCurrentToken(): string | null {
  return getStoredToken();
}

export async function recoverPassword(
  payload: { user_id: string; recovery_secret: string } | { email: string; recovery_secret: string }
): Promise<string> {
  const data = await apiRequest<{ reset_token: string }>("/auth/recover", payload);
  return data.reset_token;
}

/**
 * Redefine a senha e devolve o NOVO código de recuperação.
 *
 * O código usado para autorizar o reset é invalidado no servidor, então o
 * usuário precisa guardar este substituto — sem ele, fica sem via de
 * recuperação.
 */
export async function resetPassword(
  resetToken: string,
  newPassword: string
): Promise<{ user_id: string; recovery_secret: string }> {
  return apiRequest<{ user_id: string; recovery_secret: string }>(
    "/auth/reset-password",
    { reset_token: resetToken, new_password: newPassword }
  );
}

/**
 * Emite um novo código de recuperação e cria a segunda via da chave de dados.
 *
 * Exige a senha, e não apenas a sessão: é ela que abre a chave para poder
 * embrulhá-la de novo. **O código anterior deixa de valer no mesmo instante** —
 * por isso a tela que chama isto precisa exigir que o novo seja anotado antes
 * de prosseguir.
 */
export async function rotateRecoveryCode(
  password: string
): Promise<{ user_id: string; recovery_secret: string }> {
  return apiRequest<{ user_id: string; recovery_secret: string }>(
    "/auth/recovery-code/rotate",
    { password }
  );
}

/** Resultado da rotação da chave de dados. */
export interface RotationResult {
  patients: number;
  session_records: number;
  files: number;
  resumed: boolean;
  safety_backup: string | null;
}

/**
 * Troca a chave dos dados por uma aleatória e re-cifra o acervo.
 *
 * É a operação que faz a senha passar a ser indispensável. Exige os dois
 * segredos porque a chave nova precisa nascer com os dois envelopes, e um
 * envelope só pode ser criado a partir do segredo em claro — um único envelope
 * significaria que esquecer aquele segredo apaga o prontuário.
 *
 * Gera um backup de segurança verificado antes de tocar em qualquer registro, e
 * é retomável: se for interrompida, chamar de novo continua de onde parou.
 */
export async function rotateDataKey(
  password: string,
  recoveryCode: string
): Promise<RotationResult> {
  return apiRequest<RotationResult>("/auth/rotate-key", {
    password,
    recovery_code: recoveryCode,
  });
}

/**
 * Reconsulta `/auth/me` e atualiza o contexto de autenticação.
 *
 * Depois de resolver uma pendência de segurança, o estado real está no servidor —
 * não num booleano local do componente. Marcar só o estado local fazia o cartão
 * ficar verde até a próxima remontagem, e então voltar a amarelo: foi o que
 * aconteceu ao importar um backup logo após configurar a chave, dando a impressão
 * de que a configuração havia sido desfeita.
 */
export async function refreshAuthState(): Promise<void> {
  await completeFromStoredToken();
}

/** Confirma que o código atual foi guardado, descartando o anterior. */
export async function ackRecoveryCode(): Promise<void> {
  await apiRequest<void>("/auth/recovery-code/ack", undefined, "POST");
}

// ─── Session restore ─────────────────────────────────────────────────────

export async function restoreSession(): Promise<void> {
  const token = getStoredToken();
  if (token) {
    return completeFromStoredToken();
  }
  notify(null);
}
