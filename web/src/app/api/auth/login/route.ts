// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { NextRequest, NextResponse } from "next/server";
import {
  COOKIE_NAME,
  SESSION_MAX_AGE,
  createSessionToken,
  verifyCredentials,
} from "@/lib/auth";

function cookieSecure(request: NextRequest): boolean {
  // Lab/NodePort deploys are often plain HTTP. Never force Secure just because
  // NODE_ENV=production — browsers drop Secure cookies on http:// hosts.
  if (process.env.ZYVOR_JANUS_COOKIE_SECURE === "0") return false;
  if (process.env.ZYVOR_JANUS_COOKIE_SECURE === "1") return true;
  const forwarded = request.headers.get("x-forwarded-proto")?.split(",")[0]?.trim();
  const proto = forwarded || request.nextUrl.protocol.replace(":", "");
  return proto === "https";
}

export async function POST(request: NextRequest) {
  let username = "";
  let password = "";
  try {
    const body = (await request.json()) as { username?: string; password?: string };
    username = body.username ?? "";
    password = body.password ?? "";
  } catch {
    return NextResponse.json({ detail: "Invalid request body" }, { status: 400 });
  }

  if (!verifyCredentials(username, password)) {
    return NextResponse.json({ detail: "Invalid username or password" }, { status: 401 });
  }

  const token = await createSessionToken();
  const response = NextResponse.json({ ok: true, username: username.trim() });
  response.cookies.set({
    name: COOKIE_NAME,
    value: token,
    httpOnly: true,
    sameSite: "lax",
    secure: cookieSecure(request),
    path: "/",
    maxAge: SESSION_MAX_AGE,
  });
  return response;
}
