#!/usr/bin/env node
// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

/**
 * Zyvor Janus — client “wow” reel: dual-node (or dual-GPU) preempt / migrate.
 *
 * Default config is two machines × one H100 (placement migrate via preempt).
 * Not live CUDA. Pattern mirrors IronWolf continuous reel.
 *
 * Requires the web dashboard:
 *   ./scripts/run_web_dashboard.sh
 *
 * Usage:
 *   node scripts/demo-videos/record-zyvor-janus-2gpu-migrate-wow-reel.mjs
 *   ZYVOR_JANUS_DEMO_CONFIG=dual_gpu_preempt.yaml node …   # one-node 2×GPU variant
 *
 * Desktop outputs (dual-node default):
 *   zyvor-janus-client-dual-node-migrate-wow-reel.mp4
 *   zyvor-janus-client-dual-node-migrate-wow-reel-linkedin-1080p.mp4
 *   (+ legacy zyvor-janus-client-wow-reel.mp4 copies for older links)
 */
import { createRequire } from 'node:module';
import {
  mkdirSync,
  writeFileSync,
  readdirSync,
  unlinkSync,
  copyFileSync,
  existsSync,
} from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';

const DESKTOP = join(homedir(), 'Desktop');
const FS_ROOT = join(homedir(), 'tt/zyvor-janus');
const PLAYWRIGHT_CANDIDATES = [
  join(homedir(), 'tt/forge/web-ui/node_modules/playwright/package.json'),
  join(homedir(), 'tt/IronWolf/scripts/demo-videos/node_modules/playwright/package.json'),
];
const pwPkg = PLAYWRIGHT_CANDIDATES.find((p) => existsSync(p));
if (!pwPkg) {
  console.error('Playwright not found — install in forge/web-ui or IronWolf demo-videos');
  process.exit(1);
}
const require = createRequire(pwPkg);
const { chromium } = require('playwright');

const BASE = (process.argv.find((a) => a.startsWith('http')) || process.env.ZYVOR_JANUS_URL || 'http://localhost:3000').replace(
  /\/$/,
  '',
);
const USER = process.env.ZYVOR_JANUS_USER || process.env.ZYVOR_JANUS_DASHBOARD_USER || 'Admin';
const PASS = process.env.ZYVOR_JANUS_PASS || process.env.ZYVOR_JANUS_DASHBOARD_PASSWORD || 'Admin@321';
const CONFIG = process.env.ZYVOR_JANUS_DEMO_CONFIG || 'dual_node_preempt.yaml';
const DUAL_NODE = CONFIG.includes('dual_node');

const WORK = join(DESKTOP, 'zyvor-janus-wow-work');
const RAW = join(WORK, 'raw');
const SHOTS = join(DESKTOP, DUAL_NODE ? 'zyvor-janus-dual-node-wow-shots' : 'zyvor-janus-wow-shots');
const FINAL_WEB = join(
  DESKTOP,
  DUAL_NODE ? 'zyvor-janus-client-dual-node-migrate-wow-reel.mp4' : 'zyvor-janus-client-wow-reel.mp4',
);
const FINAL_LI = join(
  DESKTOP,
  DUAL_NODE
    ? 'zyvor-janus-client-dual-node-migrate-wow-reel-linkedin-1080p.mp4'
    : 'zyvor-janus-client-wow-reel-linkedin-1080p.mp4',
);

mkdirSync(RAW, { recursive: true });
mkdirSync(SHOTS, { recursive: true });

async function ensureCaptionCss(page) {
  await page
    .addStyleTag({
      content: `
      #fs-demo-caption {
        position: fixed !important; left: 50% !important; bottom: 36px !important;
        transform: translateX(-50%) !important; z-index: 2147483647 !important;
        max-width: min(1040px, 94vw) !important; padding: 18px 28px !important;
        border-radius: 16px !important; background: rgba(6, 10, 18, 0.94) !important;
        color: #f8fafc !important; font-family: "IBM Plex Sans", "Segoe UI", system-ui, sans-serif !important;
        font-size: 26px !important; font-weight: 650 !important; letter-spacing: 0.01em !important;
        line-height: 1.35 !important; text-align: center !important;
        text-shadow: 0 1px 2px rgba(0,0,0,0.55) !important;
        box-shadow: 0 16px 48px rgba(0,0,0,0.55) !important;
        border: 1px solid rgba(255,255,255,0.18) !important;
        pointer-events: none !important; backdrop-filter: blur(12px) !important;
      }
      #fs-demo-caption-brand {
        font-size: 12px !important; letter-spacing: 0.16em !important;
        text-transform: uppercase !important; color: #7dd3fc !important;
        margin-bottom: 8px !important; font-weight: 700 !important;
      }
      #fs-load-mask {
        position: fixed !important; inset: 0 !important; z-index: 2147483646 !important;
        background: #070b12 !important; pointer-events: none !important;
      }
    `,
    })
    .catch(() => {});
}

async function showMask(page) {
  await ensureCaptionCss(page);
  await page.evaluate(() => {
    if (document.getElementById('fs-load-mask')) return;
    const o = document.createElement('div');
    o.id = 'fs-load-mask';
    document.documentElement.appendChild(o);
  });
}

async function hideMask(page) {
  await page.evaluate(() => document.getElementById('fs-load-mask')?.remove());
}

async function caption(page, text, holdMs = 2400) {
  await ensureCaptionCss(page);
  await hideMask(page);
  await page.evaluate((msg) => {
    let bar = document.getElementById('fs-demo-caption');
    if (!bar) {
      bar = document.createElement('div');
      bar.id = 'fs-demo-caption';
      bar.innerHTML =
        '<div id="fs-demo-caption-brand">Zyvor Janus · Client Demo</div><div id="fs-demo-caption-text"></div>';
      document.documentElement.appendChild(bar);
    }
    const body = document.getElementById('fs-demo-caption-text');
    if (body) body.textContent = msg;
  }, text);
  console.log(`[caption] ${text}`);
  await page.waitForTimeout(holdMs);
}

async function gotoReady(page, path, { readyText, timeout = 45000 } = {}) {
  await showMask(page);
  await page.goto(`${BASE}${path}`, { waitUntil: 'domcontentloaded', timeout: 90000 });
  await page
    .waitForFunction(
      ({ needle }) => {
        const t = (document.body?.innerText || '').replace(/\s+/g, ' ');
        if (/Something went wrong/i.test(t)) return false;
        if (needle) return t.toLowerCase().includes(String(needle).toLowerCase());
        return t.trim().length > 80;
      },
      { needle: readyText || '' },
      { timeout },
    )
    .catch(() => {});
  await page.waitForTimeout(600);
  await ensureCaptionCss(page);
  await hideMask(page);
  await page.waitForTimeout(200);
}

async function login(page) {
  await showMask(page);
  await page.goto(`${BASE}/login`, { waitUntil: 'domcontentloaded', timeout: 90000 });
  await page.waitForSelector('input[name="username"], #password', { timeout: 30000 });
  await hideMask(page);
  await ensureCaptionCss(page);
  await caption(page, 'Zyvor Janus — GPU scheduler digital twin for Forge.', 2600);
  await page.locator('input[name="username"]').fill(USER);
  await page.locator('input[name="password"]').fill(PASS);
  await caption(page, 'Mission Control — run, replay, and compare without physical GPUs.', 2200);
  await showMask(page);
  await page.getByRole('button', { name: /Sign in/i }).click();
  await page
    .waitForFunction(() => /STANDBY|Launch|TOTAL RUNS|CONFIGURATION/i.test(document.body.innerText), {
      timeout: 30000,
    })
    .catch(() => {});
  await page.waitForTimeout(700);
  await hideMask(page);
}

async function selectConfig(page, configId) {
  const combo = page.getByRole('combobox', { name: /CONFIGURATION/i }).first();
  if (await combo.isVisible().catch(() => false)) {
    await combo.selectOption(configId).catch(async () => {
      await combo.selectOption({ label: configId }).catch(() => {});
    });
  } else {
    const select = page.locator('select').first();
    await select.selectOption(configId).catch(() => {});
  }
  await page.waitForTimeout(400);
}

async function launchRun(page) {
  await selectConfig(page, CONFIG);
  await caption(
    page,
    DUAL_NODE
      ? 'Two machines × one H100 — preemptive scheduler.'
      : 'Two H100s on one node — preemptive scheduler.',
    2600,
  );
  await page.screenshot({ path: join(SHOTS, '02-config-dual-gpu.png') });

  await page.getByRole('button', { name: /Run/i }).click();
  await showMask(page);
  await page.waitForURL(/\/runs\/[0-9a-f-]+/i, { timeout: 60000 });
  await page
    .waitForFunction(() => /DONE|Completed|gpu-0|PREEMPTIONS|node-0/i.test(document.body.innerText), {
      timeout: 30000,
    })
    .catch(() => {});
  await page.waitForTimeout(600);
  await hideMask(page);

  const runPath = new URL(page.url()).pathname;
  console.log(`[wow] run=${runPath}`);
  await caption(
    page,
    DUAL_NODE
      ? 'Run complete — machine→machine placement migrate (1 preemption).'
      : 'Run complete — 2 GPUs, 1 preemption, mover migrates.',
    2800,
  );
  await page.screenshot({ path: join(SHOTS, '02b-run-detail.png') });
  return runPath;
}

async function scrubSimulate(page, runPath) {
  const id = runPath.replace(/^\/runs\//, '').split(/[?#]/)[0];
  await gotoReady(page, `/simulate?run=${id}`, { readyText: 'GPU' });
  // Ensure run is selected / snapshots loaded
  await page.waitForFunction(() => /gpu-0|Free GPUs|Scheduler Replay/i.test(document.body.innerText), {
    timeout: 30000,
  }).catch(() => {});
  await caption(page, 'Simulate stage — requests → queue → GPUs.', 2400);
  await page.screenshot({ path: join(SHOTS, '03-simulate-stage.png') });

  await caption(
    page,
    DUAL_NODE
      ? 'High priority arrives — preempt mover off node-0.'
      : 'High priority arrives — preempt mover off gpu-0.',
    2600,
  );
  const playBtn = page.getByRole('button', { name: /Play/i }).first();
  if (await playBtn.isVisible().catch(() => false)) {
    await playBtn.click();
    // Replay steps at ~500ms; ~12 decisions → ~6s; hold through migrate
    await page.waitForTimeout(9000);
  } else {
    const nextBtn = page.getByRole('button', { name: /Next/i }).first();
    for (let i = 0; i < 12; i++) {
      await nextBtn.click().catch(() => {});
      await page.waitForTimeout(400);
    }
  }

  await caption(
    page,
    DUAL_NODE
      ? 'Workload resumes on the other machine — node-0 → node-1.'
      : 'Workload resumes on the free GPU — gpu-0 → gpu-1.',
    3200,
  );
  await page.screenshot({ path: join(SHOTS, '04-migrate-resume.png') });
}

async function showGantt(page, runPath) {
  await gotoReady(page, runPath, { readyText: 'gpu-0' });
  await caption(
    page,
    DUAL_NODE
      ? 'Same job, two segments: node-0/gpu-0 then node-1/gpu-1.'
      : 'Same job, two segments: gpu-0 then gpu-1.',
    3000,
  );
  // Scroll gantt into view
  await page.evaluate(() => {
    const el = [...document.querySelectorAll('*')].find((n) =>
      /JOB TIMELINE|gpu-0/i.test(n.textContent || ''),
    );
    el?.scrollIntoView({ block: 'center' });
  });
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(SHOTS, '05-gantt-segments.png') });
  await caption(
    page,
    DUAL_NODE
      ? 'Preempt/resume across machines — not live CUDA (Forge Path B is experimental).'
      : 'Preemption = digital-twin migrate without live CUDA move.',
    2800,
  );
}

function encode(src, dest, w, h) {
  const ff = spawnSync(
    'ffmpeg',
    [
      '-y',
      '-i',
      src,
      '-vf',
      `scale=${w}:${h}:force_original_aspect_ratio=decrease,pad=${w}:${h}:(ow-iw)/2:(oh-ih)/2:color=0x070b12,fps=30`,
      '-c:v',
      'libx264',
      '-pix_fmt',
      'yuv420p',
      '-movflags',
      '+faststart',
      '-an',
      dest,
    ],
    { encoding: 'utf8' },
  );
  if (ff.status !== 0) {
    console.error(ff.stderr?.slice(-1500));
    throw new Error(`ffmpeg failed → ${dest}`);
  }
  return dest;
}

async function main() {
  console.log(`[wow] base=${BASE} config=${CONFIG}`);
  if (existsSync(RAW)) {
    for (const f of readdirSync(RAW)) {
      try {
        unlinkSync(join(RAW, f));
      } catch {
        /* ignore */
      }
    }
  }
  if (existsSync(SHOTS)) {
    for (const f of readdirSync(SHOTS)) {
      try {
        unlinkSync(join(SHOTS, f));
      } catch {
        /* ignore */
      }
    }
  }

  // Warm off-camera
  {
    const warmBrowser = await chromium.launch({ channel: 'chrome', headless: true });
    const warmCtx = await warmBrowser.newContext({
      viewport: { width: 1440, height: 900 },
      colorScheme: 'dark',
    });
    const warm = await warmCtx.newPage();
    console.log('[wow] warming pages (off-camera)…');
    await warm.goto(`${BASE}/login`, { waitUntil: 'domcontentloaded', timeout: 90000 }).catch(() => {});
    await warm.locator('input[name="username"]').fill(USER).catch(() => {});
    await warm.locator('input[name="password"]').fill(PASS).catch(() => {});
    await warm.getByRole('button', { name: /Sign in/i }).click().catch(() => {});
    await warm.waitForTimeout(1500);
    await warm.goto(BASE, { waitUntil: 'domcontentloaded', timeout: 60000 }).catch(() => {});
    await warmBrowser.close();
  }

  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    colorScheme: 'dark',
    recordVideo: { dir: RAW, size: { width: 1440, height: 900 } },
  });
  const page = await context.newPage();
  const t0 = Date.now();

  await login(page);
  await gotoReady(page, '/', { readyText: 'CONFIGURATION' });
  await caption(page, 'Zyvor Janus — GPU scheduler digital twin.', 2600);
  await page.screenshot({ path: join(SHOTS, '01-dashboard.png') });

  const runPath = await launchRun(page);
  await scrubSimulate(page, runPath);
  await showGantt(page, runPath);

  await gotoReady(page, '/', { readyText: 'CONFIGURATION' });
  await caption(page, 'zyvor.dev  ·  Forge + Zyvor Janus  ·  Schedule without the silicon.', 4000);
  await page.screenshot({ path: join(SHOTS, '06-close.png') });

  await page.close();
  await context.close();
  await browser.close();

  const webms = readdirSync(RAW).filter((f) => f.endsWith('.webm'));
  if (!webms.length) throw new Error('no webm recorded');
  const src = join(RAW, webms[0]);
  console.log(`[wow] encoding ${src}`);
  encode(src, FINAL_WEB, 1440, 900);
  encode(src, FINAL_LI, 1920, 1080);
  copyFileSync(src, join(DESKTOP, DUAL_NODE ? 'zyvor-janus-client-dual-node-migrate-wow-reel.webm' : 'zyvor-janus-client-wow-reel.webm'));
  if (DUAL_NODE) {
    // Keep legacy filenames pointing at the latest dual-node cut.
    copyFileSync(FINAL_WEB, join(DESKTOP, 'zyvor-janus-client-wow-reel.mp4'));
    copyFileSync(FINAL_LI, join(DESKTOP, 'zyvor-janus-client-wow-reel-linkedin-1080p.mp4'));
  }

  const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
  const meta = {
    base: BASE,
    config: CONFIG,
    dualNode: DUAL_NODE,
    elapsedSec: Number(elapsed),
    websiteMp4: FINAL_WEB,
    linkedInMp4: FINAL_LI,
    shots: SHOTS,
    recordedAt: new Date().toISOString(),
    note: DUAL_NODE
      ? 'dual_node_preempt — 2 machines × H100, preempt mover node-0 → resume node-1 (placement migrate)'
      : 'dual_gpu_preempt — 2 H100s, preempt mover gpu-0 → resume gpu-1',
  };
  writeFileSync(
    join(
      DESKTOP,
      DUAL_NODE
        ? 'zyvor-janus-client-dual-node-migrate-wow-reel-manifest.json'
        : 'zyvor-janus-client-wow-reel-manifest.json',
    ),
    JSON.stringify(meta, null, 2),
  );
  console.log(JSON.stringify(meta, null, 2));
  console.log(`[wow] DONE in ${elapsed}s — Desktop updated`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
