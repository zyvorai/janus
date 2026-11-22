// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

/**
 * Shared Framer Motion variants — import from here instead of redefining transitions
 * per component. `prefers-reduced-motion` is handled globally by the `MotionConfig
 * reducedMotion="user"` wrapper in app/layout.tsx, which strips transform-based motion
 * (the `y` in fadeInUp, stagger timing) for users who request it — these variants don't
 * need their own reduced-motion branching.
 */
import type { Transition, Variants } from "framer-motion";

export const easeOut: Transition["ease"] = [0.16, 1, 0.3, 1];

export const fadeInUp: Variants = {
  hidden: { opacity: 0, y: 14 },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.42, ease: easeOut },
  },
};

export const staggerContainer: Variants = {
  hidden: {},
  visible: {
    transition: { staggerChildren: 0.06, delayChildren: 0.02 },
  },
};
