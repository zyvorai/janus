// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

/** @type {import('next').NextConfig} */
const apiBase = process.env.ZYVOR_JANUS_API_URL || "http://127.0.0.1:8080";

const nextConfig = {
  output: "standalone",
  async rewrites() {
    // Proxy Zyvor Janus backend routes. Next.js handlers under /api/auth/* take
    // precedence over these afterFiles rewrites.
    return [
      { source: "/api/:path*", destination: `${apiBase}/api/:path*` },
      { source: "/ws/:path*", destination: `${apiBase}/ws/:path*` },
    ];
  },
};

module.exports = nextConfig;
