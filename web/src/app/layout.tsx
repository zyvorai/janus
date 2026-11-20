// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import { MotionConfig } from "framer-motion";
import { AuthenticatedChrome } from "@/components/AuthenticatedChrome";
import "./globals.css";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
});

const jetbrains = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-jetbrains",
});

export const metadata: Metadata = {
  title: "Zyvor Janus · Mission Control",
  description: "Monitor, replay, and compare GPU scheduler simulations",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${inter.variable} ${jetbrains.variable}`}>
      <body className="app-shell antialiased">
        <MotionConfig reducedMotion="user">
          <AuthenticatedChrome>{children}</AuthenticatedChrome>
        </MotionConfig>
      </body>
    </html>
  );
}
