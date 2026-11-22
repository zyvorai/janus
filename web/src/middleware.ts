// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import { NextRequest, NextResponse } from "next/server";
import { COOKIE_NAME, isAuthEnabled, isPublicPath, verifySessionToken } from "@/lib/auth";

/**
 * Paths that next.config.js's rewrites() proxy through to zyvor-janus-api
 * (excluding /api/auth/* -- those are Next's own route handlers and never
 * reach the Rust backend, per next.config.js's comment on rewrite ordering).
 */
function isBackendPath(pathname: string): boolean {
  return pathname.startsWith("/api/") || pathname.startsWith("/ws/");
}

/**
 * zyvor-janus-api requires this shared secret as a bearer token on every
 * route except /api/health (see crates/zyvor-janus-api/src/auth.rs). Reused
 * from the OpenAI-shim's existing env var. Injected here, server-side only,
 * so the browser never sees it -- it only ever holds the session cookie.
 */
function withBackendAuth(request: NextRequest): NextResponse {
  const headers = new Headers(request.headers);
  headers.set("Authorization", `Bearer ${process.env.ZYVOR_JANUS_API_KEY ?? "dev-zyvor-janus-key"}`);
  return NextResponse.next({ request: { headers } });
}

function proceed(request: NextRequest): NextResponse {
  return isBackendPath(request.nextUrl.pathname) ? withBackendAuth(request) : NextResponse.next();
}

export async function middleware(request: NextRequest) {
  if (!isAuthEnabled()) {
    return proceed(request);
  }

  const { pathname } = request.nextUrl;
  if (
    isPublicPath(pathname) ||
    pathname.startsWith("/_next") ||
    pathname === "/favicon.ico" ||
    pathname === "/zyvor-logo.png"
  ) {
    return NextResponse.next();
  }

  const token = request.cookies.get(COOKIE_NAME)?.value;
  if (await verifySessionToken(token)) {
    return proceed(request);
  }

  if (pathname.startsWith("/api/")) {
    return NextResponse.json({ detail: "Unauthorized" }, { status: 401 });
  }

  const loginUrl = request.nextUrl.clone();
  loginUrl.pathname = "/login";
  loginUrl.searchParams.set("next", pathname);
  return NextResponse.redirect(loginUrl);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
