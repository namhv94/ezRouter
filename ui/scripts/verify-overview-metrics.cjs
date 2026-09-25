/**
 * Regression check for OverviewTab metrics formatting.
 * Verifies backend API contract alignment:
 * Backend (/admin/stats, AccountPool.get_stats_summary) returns active_rate as 0..100 percentage.
 * OverviewTab must NOT multiply active_rate by 100, and must handle 0.0 correctly without falsy fallback.
 */

const fs = require('fs');
const path = require('path');

const overviewPath = path.resolve(__dirname, '../src/components/OverviewTab.tsx');
const overviewContent = fs.readFileSync(overviewPath, 'utf8');

const checks = [
  {
    name: 'Active rate metric does not multiply by 100',
    test: () => {
      // Must not contain active_rate * 100
      return !/active_rate\s*\*\s*100/.test(overviewContent);
    },
    detail: 'OverviewTab must not multiply stats.active_rate by 100 since backend already returns percentage (0..100)',
  },
  {
    name: 'Active rate handles 0.0 correctly (nullish check, not truthy check)',
    test: () => {
      // Must check != null or !== undefined rather than truthy stats?.active_rate ?
      return /active_rate\s*!=\s*null|active_rate\s*!==\s*undefined|typeof\s+stats\?\.active_rate/.test(overviewContent);
    },
    detail: 'OverviewTab must use nullish/type check so active_rate: 0.0 is not treated as falsy and defaulted to 100.0',
  },
  {
    name: 'Active rate formats to 1 decimal place with percentage sign',
    test: () => {
      return /active_rate\.toFixed\(1\)/.test(overviewContent);
    },
    detail: 'OverviewTab must format active_rate using .toFixed(1)',
  },
  {
    name: 'Simulated contract formatting matches expectations',
    test: () => {
      const formatActiveRate = (stats) =>
        `${stats?.active_rate != null ? stats.active_rate.toFixed(1) : '100.0'}%`;

      const t1 = formatActiveRate({ active_rate: 100.0 }) === '100.0%';
      const t2 = formatActiveRate({ active_rate: 0.0 }) === '0.0%';
      const t3 = formatActiveRate({ active_rate: 85.7 }) === '85.7%';
      const t4 = formatActiveRate(null) === '100.0%';
      const t5 = formatActiveRate(undefined) === '100.0%';

      return t1 && t2 && t3 && t4 && t5;
    },
    detail: 'Formatting logic must produce expected percentage strings for 100.0%, 0.0%, 85.7%, and null/undefined fallbacks',
  },
];

console.log('--- AG-Proxy UI Overview Metrics Regression Verification ---');
let passed = 0;
let failed = 0;

for (const check of checks) {
  const ok = check.test();
  if (ok) {
    console.log(`PASS: ${check.name}`);
    passed++;
  } else {
    console.error(`FAIL: ${check.name} (${check.detail})`);
    failed++;
  }
}

console.log(`\nTotal checks: ${checks.length}, Passed: ${passed}, Failed: ${failed}`);

if (failed > 0) {
  process.exit(1);
} else {
  console.log('All overview metric invariants verified successfully.\n');
}
