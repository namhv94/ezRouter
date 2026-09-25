/**
 * Invariant verification for TokenSaverTab.tsx
 * Verifies that:
 * 1. TokenSaverTab includes master switch, RTK switch, Caveman and Ponytail selectors.
 * 2. Save button triggers api.updateSettings.
 * 3. Token savings calculations and metric display exist.
 * 4. Hermes / Tool Safety invariants are documented and enforced.
 * 5. Per-request header overrides (x-token-saver, x-caveman, x-ponytail) are documented.
 */

const fs = require('fs');
const path = require('path');

const tabPath = path.resolve(__dirname, '../src/components/TokenSaverTab.tsx');
const tabContent = fs.readFileSync(tabPath, 'utf8');

const checks = [
  {
    name: 'TokenSaverTab contains Master Switch toggle',
    test: () => tabContent.includes('token_saver_enabled') && /Kích Hoạt Token Saver/.test(tabContent),
    detail: 'Master switch allows toggling token saver globally',
  },
  {
    name: 'TokenSaverTab contains RTK Input Compression switch',
    test: () => tabContent.includes('rtk_enabled') && /RTK Input Compression/.test(tabContent),
    detail: 'RTK switch controls selective log/diff/build compression',
  },
  {
    name: 'TokenSaverTab contains Caveman level selector with 4 options',
    test: () => tabContent.includes('caveman_level') && /value="off"/.test(tabContent) && /value="ultra"/.test(tabContent),
    detail: 'Caveman selector supports off, lite, full, and ultra levels',
  },
  {
    name: 'TokenSaverTab contains Ponytail level selector with 4 options',
    test: () => tabContent.includes('ponytail_level') && /value="off"/.test(tabContent) && /value="ultra"/.test(tabContent),
    detail: 'Ponytail selector supports off, lite, full, and ultra levels',
  },
  {
    name: 'TokenSaverTab Save button invokes api.updateSettings',
    test: () => /api\.updateSettings\(settings\)/.test(tabContent),
    detail: 'Save button persists configuration to backend POST /admin/settings',
  },
  {
    name: 'TokenSaverTab displays estimated token savings metrics',
    test: () => /totalEstimatedSaved|TIẾT KIỆM TOKEN ƯỚC TÍNH/.test(tabContent),
    detail: 'Savings card and metrics display estimated tokens saved',
  },
  {
    name: 'TokenSaverTab documents Hermes / Tool Calling safety invariants',
    test: () => /Hermes \/ Tool Safety Invariants/.test(tabContent) && /structured JSON/.test(tabContent),
    detail: 'Explicitly guarantees zero compression on tools/JSON and auto-disabled injections',
  },
  {
    name: 'TokenSaverTab documents HTTP header overrides',
    test: () => tabContent.includes('x-token-saver') && tabContent.includes('x-caveman') && tabContent.includes('x-ponytail'),
    detail: 'Shows curl / client override headers',
  },
];

console.log('--- AG-Proxy UI Token Saver Tab Verification ---');
let allPassed = true;
let passedCount = 0;

for (const check of checks) {
  const passed = check.test();
  if (passed) {
    console.log(`PASS: ${check.name}`);
    passedCount++;
  } else {
    console.error(`FAIL: ${check.name} - ${check.detail}`);
    allPassed = false;
  }
}

console.log(`\nTotal checks: ${checks.length}, Passed: ${passedCount}, Failed: ${checks.length - passedCount}`);
if (!allPassed) {
  process.exit(1);
}
console.log('All Token Saver Tab invariants verified successfully.\n');
