// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

/** Allow only same-origin relative paths (blocks protocol-relative //evil.com redirects). */
export function safeRelativePath(raw: string | null | undefined, fallback = "/"): string {
  if (!raw) return fallback;
  if (!raw.startsWith("/") || raw.startsWith("//")) return fallback;
  if (raw.includes("\\") || raw.includes("\0")) return fallback;
  return raw;
}
