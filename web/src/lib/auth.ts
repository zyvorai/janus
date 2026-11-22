// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

/** Session cookie auth for the Zyvor Janus dashboard. */

export const COOKIE_NAME = "zyvor_janus_session";
export const SESSION_MAX_AGE = 7 * 24 * 60 * 60; // seconds
export const DEFAULT_USERNAME = "Admin";
export const DEFAULT_PASSWORD = "Admin@321";

export function getDashboardUsername(): string {
  return process.env.ZYVOR_JANUS_DASHBOARD_USER?.trim() || DEFAULT_USERNAME;
}

export function getDashboardPassword(): string {
  return process.env.ZYVOR_JANUS_DASHBOARD_PASSWORD?.trim() || DEFAULT_PASSWORD;
}

export function isAuthEnabled(): boolean {
  return true;
}

export function getAuthSecret(): string {
  return (
    process.env.ZYVOR_JANUS_AUTH_SECRET?.trim() ||
    getDashboardPassword() ||
    DEFAULT_PASSWORD
  );
}

function bufferToHex(buffer: ArrayBuffer): string {
  return [...new Uint8Array(buffer)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function hmacSign(message: string, secret: string): Promise<string> {
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    enc.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const sig = await crypto.subtle.sign("HMAC", key, enc.encode(message));
  return bufferToHex(sig);
}

function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

function timingSafeEqualInsensitive(a: string, b: string): boolean {
  return timingSafeEqual(a.toLowerCase(), b.toLowerCase());
}

function matchesUsername(input: string, expected: string): boolean {
  return timingSafeEqualInsensitive(input.trim(), expected.trim());
}

function matchesPassword(input: string, expected: string): boolean {
  return timingSafeEqualInsensitive(input.trim(), expected.trim());
}

export async function createSessionToken(): Promise<string> {
  const exp = Date.now() + SESSION_MAX_AGE * 1000;
  const payload = `zyvor-janus:${exp}`;
  const sig = await hmacSign(payload, getAuthSecret());
  return `${payload}.${sig}`;
}

export async function verifySessionToken(token: string | undefined | null): Promise<boolean> {
  if (!token || !getAuthSecret()) return false;
  const dot = token.lastIndexOf(".");
  if (dot < 0) return false;
  const payload = token.slice(0, dot);
  const sig = token.slice(dot + 1);
  const expected = await hmacSign(payload, getAuthSecret());
  if (!timingSafeEqual(sig, expected)) return false;
  const exp = Number(payload.split(":")[1]);
  return Number.isFinite(exp) && Date.now() < exp;
}

export function verifyCredentials(username: string, password: string): boolean {
  const envUser = getDashboardUsername();
  const envPass = getDashboardPassword();

  if (matchesUsername(username, envUser) && matchesPassword(password, envPass)) {
    return true;
  }

  // Dev fallback: documented defaults still work if a stale shell env var is set.
  if (process.env.NODE_ENV !== "production") {
    return (
      matchesUsername(username, DEFAULT_USERNAME) && matchesPassword(password, DEFAULT_PASSWORD)
    );
  }

  return false;
}

export const PUBLIC_PATHS = ["/login", "/api/auth/login", "/api/auth/logout", "/api/auth/me"];

export function isPublicPath(pathname: string): boolean {
  return PUBLIC_PATHS.some((path) => pathname === path || pathname.startsWith(`${path}/`));
}
