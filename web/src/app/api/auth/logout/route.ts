// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import { NextRequest, NextResponse } from "next/server";
import { COOKIE_NAME } from "@/lib/auth";

function cookieSecure(request: NextRequest): boolean {
  if (process.env.ZYVOR_JANUS_COOKIE_SECURE === "0") return false;
  if (process.env.ZYVOR_JANUS_COOKIE_SECURE === "1") return true;
  const forwarded = request.headers.get("x-forwarded-proto")?.split(",")[0]?.trim();
  const proto = forwarded || request.nextUrl.protocol.replace(":", "");
  return proto === "https";
}

export async function POST(request: NextRequest) {
  const response = NextResponse.json({ ok: true });
  response.cookies.set({
    name: COOKIE_NAME,
    value: "",
    httpOnly: true,
    sameSite: "lax",
    secure: cookieSecure(request),
    path: "/",
    maxAge: 0,
  });
  return response;
}
