/**
 * Verification of the remaining quota / balance formatting surfaces in the UI.
 *
 * The Google-Antigravity and Codex account-pool quota panels (gemini_5h,
 * gemini_weekly, claude_5h, claude_weekly, per-account cooldowns) were removed
 * together with the account tables, so this verifier now pins what survives:
 *
 * 1. OpenRouter credit balance rendering in ProvidersTab (net balance, warn tone).
 * 2. Model request-share quota bars in OverviewTab (percent math + tone classes).
 * 3. The shared .quota-track / .quota-fill CSS contract.
 */

const fs = require('fs');
const path = require('path');

const providersPath = path.resolve(__dirname, '../src/components/ProvidersTab.tsx');
const overviewPath = path.resolve(__dirname, '../src/components/OverviewTab.tsx');
const stylesPath = path.resolve(__dirname, '../src/styles.css');

const providersContent = fs.readFileSync(providersPath, 'utf8');
const overviewContent = fs.readFileSync(overviewPath, 'utf8');
const stylesContent = fs.readFileSync(stylesPath, 'utf8');

const sourceChecks = [
  {
    name: 'ProvidersTab derives net OpenRouter balance from credits minus usage',
    test: () =>
      /openrouterCredits\.total_credits\s*-\s*\(?\s*openrouterCredits\.total_usage\s*\|\|\s*0\s*\)?/.test(
        providersContent
      ),
    detail: 'Balance must be total_credits - total_usage',
  },
  {
    name: 'ProvidersTab clamps the balance at zero and formats 2 decimals',
    test: () => /Math\.max\(\s*0,\s*openrouterCredits\.total_credits\s*-/.test(providersContent) &&
      /\.toFixed\(2\)/.test(providersContent),
    detail: 'Balance must never render negative and must use 2 decimal places',
  },
  {
    name: 'ProvidersTab applies the balance-warn tone when the balance is nearly exhausted',
    test: () => /balance-warn/.test(providersContent) && /balance-good/.test(providersContent),
    detail: 'Balance display must switch between balance-good and balance-warn tones',
  },
  {
    name: 'ProvidersTab surfaces a low-balance warning badge',
    test: () => providersContent.includes('openrouter-low-balance-badge'),
    detail: 'A low-balance advisory badge must be rendered when credits are exhausted',
  },
  {
    name: 'ProvidersTab renders the OpenRouter balance card',
    test: () => providersContent.includes('openrouter-balance-card'),
    detail: 'The OpenRouter balance card must be present',
  },
  {
    name: 'OverviewTab renders a model-share quota bar per model',
    test: () =>
      overviewContent.includes('quota-track') &&
      overviewContent.includes('quota-fill') &&
      /width:\s*`\$\{pct\}%`/.test(overviewContent),
    detail: 'Each model row must render a quota-track/quota-fill bar sized by percentage',
  },
  {
    name: 'OverviewTab no longer renders per-account Google/Codex quota rows',
    test: () =>
      !/gemini_5h|gemini_weekly|claude_5h|claude_weekly|remaining_percent/.test(overviewContent),
    detail: 'Account-pool quota fields must be fully removed from the Overview',
  },
];

// Pure logic: net balance formatting, mirrored from ProvidersTab.
const formatBalance = (credits) => {
  if (!credits || typeof credits.total_credits !== 'number') return null;
  const net = credits.total_credits - (credits.total_usage || 0);
  return `$${Math.max(0, net).toFixed(2)}`;
};

const isLowBalance = (credits) => {
  if (!credits || typeof credits.total_credits !== 'number') return false;
  return credits.total_credits - (credits.total_usage || 0) <= 0.05;
};

const logicChecks = [
  {
    name: 'formatBalance returns null when credits are unknown',
    test: () => formatBalance(null) === null && formatBalance({}) === null,
  },
  {
    name: 'formatBalance subtracts usage from credits',
    test: () =>
      formatBalance({ total_credits: 20, total_usage: 7.5 }) === '$12.50' &&
      formatBalance({ total_credits: 20 }) === '$20.00',
  },
  {
    name: 'formatBalance never renders a negative amount',
    test: () =>
      formatBalance({ total_credits: 5, total_usage: 12 }) === '$0.00' &&
      !formatBalance({ total_credits: 5, total_usage: 12 }).includes('-'),
  },
  {
    name: 'isLowBalance triggers at or below the 0.05 threshold',
    test: () =>
      isLowBalance({ total_credits: 10, total_usage: 9.9 }) === false &&
      isLowBalance({ total_credits: 10, total_usage: 9.97 }) === true &&
      isLowBalance({ total_credits: 10, total_usage: 10 }) === true &&
      isLowBalance({ total_credits: 10, total_usage: 15 }) === true &&
      isLowBalance(null) === false,
  },
  {
    name: 'Model share percentage never exceeds 100 or goes negative',
    test: () => {
      const pct = (requests, total) => (total > 0 ? Math.round((requests / total) * 100) : 0);
      return (
        pct(50, 100) === 50 &&
        pct(0, 0) === 0 &&
        pct(100, 100) === 100 &&
        pct(1, 3) === 33 &&
        pct(0, 100) === 0
      );
    },
  },
];

const styleChecks = [
  {
    name: 'CSS defines .quota-track as the bar container',
    test: () => /\.quota-track\s*\{[^}]*overflow:\s*hidden/s.test(stylesContent),
  },
  {
    name: 'CSS defines .quota-fill as the bar body',
    test: () => /\.quota-fill\s*\{[^}]*height:\s*100%/s.test(stylesContent),
  },
  {
    name: 'CSS keeps the quota tone variants',
    test: () =>
      /\.quota-fill\.quota-good\s*\{/.test(stylesContent) &&
      /\.quota-fill\.quota-warn\s*\{/.test(stylesContent) &&
      /\.quota-fill\.quota-bad\s*\{/.test(stylesContent),
  },
  {
    name: 'CSS no longer carries Google account quota panel rules',
    test: () => !/\.overview-quota-card\s*\{/.test(stylesContent) && !/\.auto-quota-compact-strip\s*\{/.test(stylesContent),
  },
];

let failed = 0;
console.log('--- ezRouter UI Quota & Balance Formatting Verification ---');
for (const check of [...sourceChecks, ...logicChecks, ...styleChecks]) {
  try {
    if (check.test()) {
      console.log(`PASS: ${check.name}`);
    } else {
      console.error(`FAIL: ${check.name}${check.detail ? ` (${check.detail})` : ''}`);
      failed++;
    }
  } catch (err) {
    console.error(`ERROR: ${check.name} (${err.message})`);
    failed++;
  }
}

if (failed > 0) {
  process.exit(1);
}
console.log('\nAll quota & balance formatting invariants passed.');
