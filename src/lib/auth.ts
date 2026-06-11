const apiKey = import.meta.env.VITE_FIREBASE_API_KEY || "AIzaSyCMGKi2aHlRpOynxpjhb_7VZMMMqRaUa4Y";
const authDomain = import.meta.env.VITE_FIREBASE_AUTH_DOMAIN || "atendemente-432ab.firebaseapp.com";
const projectId = import.meta.env.VITE_FIREBASE_PROJECT_ID || "atendemente-432ab";
const storageBucket = import.meta.env.VITE_FIREBASE_STORAGE_BUCKET || "atendemente-432ab.firebasestorage.app";
const messagingSenderId = import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID || "416468948741";
const appId = import.meta.env.VITE_FIREBASE_APP_ID || "1:416468948741:web:f08ce79a49801f1fc3496c";

const hasFirebase = !!(apiKey && authDomain && projectId);

// ─── State ───────────────────────────────────────────────────────────────

type AuthUser = { uid: string; email: string | null } | null;
type AuthCallback = (user: AuthUser) => void;

let currentUser: AuthUser = null;
let listeners: AuthCallback[] = [];

function notify(user: AuthUser) {
  currentUser = user;
  for (const cb of listeners) cb(user);
}

// ─── Dev-mode storage ────────────────────────────────────────────────────

function devGetToken(): string | null {
  return localStorage.getItem("token");
}

function devSetToken(id: string) {
  localStorage.setItem("token", id);
}

function devClearToken() {
  localStorage.removeItem("token");
}

// ─── Firebase initialisation (lazy, only if configured) ──────────────────

let firebaseAuth: any = null;
let initPromise: Promise<void> | null = null;

async function initFirebase() {
  if (!hasFirebase || firebaseAuth) return;

  const { initializeApp } = await import("firebase/app");
  const { getAuth, onAuthStateChanged, getIdToken } = await import("firebase/auth");

  const app = initializeApp({ apiKey, authDomain, projectId, storageBucket, messagingSenderId, appId });
  firebaseAuth = getAuth(app);

  onAuthStateChanged(firebaseAuth, async (user) => {
    if (user) {
      const token = await getIdToken(user);
      localStorage.setItem("token", token);
      notify({ uid: user.uid, email: user.email });
    } else {
      localStorage.removeItem("token");
      notify(null);
    }
  });
}

function ensureInit(): Promise<void> {
  if (!initPromise) {
    initPromise = initFirebase();
  }
  return initPromise;
}

// ─── Public API ───────────────────────────────────────────────────────────

export function onAuthChange(cb: AuthCallback): () => void {
  listeners.push(cb);
  // If we already have a user, notify immediately
  cb(currentUser);
  return () => {
    listeners = listeners.filter((l) => l !== cb);
  };
}

export async function login(email: string, password: string): Promise<void> {
  if (hasFirebase) {
    await ensureInit();
    const { signInWithEmailAndPassword, getIdToken } = await import("firebase/auth");
    const cred = await signInWithEmailAndPassword(firebaseAuth, email, password);
    const token = await getIdToken(cred.user);
    localStorage.setItem("token", token);
  } else {
    // Dev mode: accept any credentials, use email as user ID
    devSetToken(email);
    notify({ uid: email, email });
  }
}

export async function logout() {
  if (hasFirebase && firebaseAuth) {
    const { signOut } = await import("firebase/auth");
    await signOut(firebaseAuth);
  }
  devClearToken();
  notify(null);
}

export async function getCurrentToken(): Promise<string | null> {
  if (hasFirebase && firebaseAuth) {
    const { getIdToken } = await import("firebase/auth");
    const user = (firebaseAuth as any).currentUser;
    if (user) return getIdToken(user, true);
  }
  return devGetToken();
}

export function isDevMode(): boolean {
  return !hasFirebase;
}

// ─── Startup ─────────────────────────────────────────────────────────────

// In dev mode, immediately restore session from localStorage
if (!hasFirebase) {
  const saved = devGetToken();
  notify(saved ? { uid: saved, email: "dev@atendemente.local" } : null);
} else {
  // Start Firebase init in background
  ensureInit();
}
