import { NextResponse, type NextRequest } from "next/server";
import { createServerClient } from "@supabase/ssr";
import type { ResponseCookie } from "next/dist/compiled/@edge-runtime/cookies";
import {
  buildAuthenticatedReportOnlyCsp,
  buildEnforcedCsp,
  createCspNonce,
  isAuthenticatedNoncePath,
} from "@/lib/security/csp";

const PUBLIC_PATHS = ["/login", "/forgot-password", "/reset-password", "/accept-invite"];

function isPublicPath(pathname: string) {
  return PUBLIC_PATHS.some((path) => pathname === path || pathname.startsWith(`${path}/`));
}

function applySecurityHeaders(response: NextResponse, requestId: string, nonce?: string) {
  response.headers.set("x-request-id", requestId);
  response.headers.set("X-Frame-Options", "DENY");
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=(), payment=()");
  response.headers.set("Content-Security-Policy", buildEnforcedCsp());

  if (nonce) {
    response.headers.set("Content-Security-Policy-Report-Only", buildAuthenticatedReportOnlyCsp(nonce));
    response.headers.set("x-csp-nonce", nonce);
  }
}

export async function middleware(request: NextRequest) {
  const requestHeaders = new Headers(request.headers);
  const requestId = request.headers.get("x-request-id") ?? crypto.randomUUID();
  requestHeaders.set("x-request-id", requestId);
  const pendingCookies: Array<{ name: string; value: string; options?: Partial<ResponseCookie> }> = [];

  const supabaseUrl = process.env.NEXT_PUBLIC_SUPABASE_URL;
  const supabaseAnonKey = process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY;

  if (!supabaseUrl || !supabaseAnonKey) {
    const response = NextResponse.next({
      request: {
        headers: requestHeaders,
      },
    });
    applySecurityHeaders(response, requestId);
    return response;
  }

  const supabase = createServerClient(supabaseUrl, supabaseAnonKey, {
    cookies: {
      getAll() {
        return request.cookies.getAll();
      },
      setAll(nextCookies) {
        nextCookies.forEach(({ name, value, options }) => {
          request.cookies.set(name, value);
          pendingCookies.push({ name, value, options });
        });
      },
    },
  });

  const {
    data: { user },
  } = await supabase.auth.getUser();

  const pathname = request.nextUrl.pathname;

  if (!user && pathname.startsWith("/api")) {
    const response = NextResponse.next({
      request: {
        headers: requestHeaders,
      },
    });
    applySecurityHeaders(response, requestId);
    pendingCookies.forEach(({ name, value, options }) => {
      response.cookies.set(name, value, options);
    });
    return response;
  }

  if (!user && !isPublicPath(pathname)) {
    const redirectUrl = request.nextUrl.clone();
    redirectUrl.pathname = "/login";
    redirectUrl.searchParams.set("redirectTo", pathname);
    const redirectResponse = NextResponse.redirect(redirectUrl);
    applySecurityHeaders(redirectResponse, requestId);
    pendingCookies.forEach(({ name, value, options }) => {
      redirectResponse.cookies.set(name, value, options);
    });
    return redirectResponse;
  }

  if (user && pathname === "/login") {
    const redirectUrl = request.nextUrl.clone();
    redirectUrl.pathname = "/dashboard";
    redirectUrl.searchParams.delete("redirectTo");
    const redirectResponse = NextResponse.redirect(redirectUrl);
    applySecurityHeaders(redirectResponse, requestId);
    pendingCookies.forEach(({ name, value, options }) => {
      redirectResponse.cookies.set(name, value, options);
    });
    return redirectResponse;
  }

  if (user && isAuthenticatedNoncePath(pathname)) {
    const nonce = createCspNonce();
    requestHeaders.set("x-csp-nonce", nonce);
  }

  const response = NextResponse.next({
    request: {
      headers: requestHeaders,
    },
  });
  const nonce = user && isAuthenticatedNoncePath(pathname) ? requestHeaders.get("x-csp-nonce")! : undefined;
  applySecurityHeaders(response, requestId, nonce);
  pendingCookies.forEach(({ name, value, options }) => {
    response.cookies.set(name, value, options);
  });

  return response;
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|.*\\.(?:svg|png|jpg|jpeg|gif|webp)$).*)"],
};
