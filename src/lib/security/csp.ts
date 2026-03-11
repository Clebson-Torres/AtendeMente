import { headers } from "next/headers";

const isProduction = process.env.NODE_ENV === "production";

const authenticatedNoncePaths = [
  "/dashboard",
  "/agenda",
  "/patients",
  "/appointments",
  "/financeiro",
] as const;

export function isAuthenticatedNoncePath(pathname: string) {
  return authenticatedNoncePaths.some((path) => pathname === path || pathname.startsWith(`${path}/`));
}

export function createCspNonce() {
  return crypto.randomUUID().replaceAll("-", "");
}

export function buildAuthenticatedReportOnlyCsp(nonce: string) {
  return [
    "default-src 'self'",
    `script-src 'self' 'nonce-${nonce}'`,
    "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
    "img-src 'self' data: blob: https:",
    "font-src 'self' https://fonts.gstatic.com",
    "connect-src 'self' https: wss:",
    "object-src 'none'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "report-uri /api/security/csp-report",
  ].join("; ");
}

export function buildEnforcedCsp() {
  return [
    "default-src 'self'",
    `script-src 'self' 'unsafe-inline'${isProduction ? "" : " 'unsafe-eval'"}`,
    "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
    "img-src 'self' data: blob: https:",
    "font-src 'self' https://fonts.gstatic.com",
    "connect-src 'self' https: wss:",
    "object-src 'none'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join("; ");
}

export async function getRequestCspNonce() {
  const headerStore = await headers();
  return headerStore.get("x-csp-nonce");
}
