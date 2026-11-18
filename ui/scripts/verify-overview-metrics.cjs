/**
 * Regression check for OverviewTab metrics formatting.
 * Verifies backend API contract alignment:
 *   - Backend (/admin/stats) returns error_rate as a 0..100 percentage.
 *   - OverviewTab derives the success rate as 100 - error_rate (clamped to 0..100).
 *   - Both are formatted to exactly 1 decimal place with a percent sign.
 *   - No account-pool "active rate" metric exists any more.
 */

const fs = require('fs');
const path = require('path');

const overviewPath = path.resolve(__dirname, '../src/components/OverviewTab.tsx');
const overviewContent = fs.readFileSync(overviewPath, 'utf8');

const checks = [
  {
    name: 'Error rate is read from stats.error_rate with a zero fallback',
    test: () => /const\s+errorRate\s*=\s*stats\?\.error_rate\s*\?\?\s*0/.test(overviewContent),
    detail: 'OverviewTab must read stats.error_rate with a nullish fallback of 0',
  },
  {
    name: 'Success rate is derived as 100 - error_rate, clamped to 0..100',
    test: () =>
      /successRateNum\s*=\s*Math\.max\(\s*0\s*,\s*Math\.min\(\s*100\s*,\s*100\s*-\s*errorRate\s*\)\s*\)/.test(overviewContent),
    detail: 'Success rate must be Math.max(0, Math.min(100, 100 - errorRate)) so it stays in 0..100',
  },
  {
    name: 'Success rate formats to 1 decimal place',
    test: () => /successRateStr\s*=\s*successRateNum\.toFixed\(1\)/.test(overviewContent),
    detail: 'Success rate must be formatted with .toFixed(1)',
  },
  {
    name: 'Error rate formats to 1 decimal place with percentage sign',
    test: () => /errorRate\.toFixed\(1\)/.test(overviewContent),
    detail: 'OverviewTab must format error_rate using .toFixed(1) and append %',
  },
  {
    name: 'Error rate is not multiplied by 100',
    test: () => !/error_rate\s*\*\s*100/.test(overviewContent) && !/errorRate\s*\*\s*100/.test(overviewContent),
    detail: 'OverviewTab must not multiply error_rate by 100 since the backend already returns a percentage',
  },
  {
    name: 'No legacy account-pool active_rate metric remains',
    test: () => !/active_rate/.test(overviewContent),
    detail: 'The account-pool active_rate widget was removed; it must not be reintroduced',
  },
  {
    name: 'Simulated contract formatting matches expectations',
    test: () => {
      const format = (stats) => {
        const errorRate = stats?.error_rate ?? 0;
        const successRateNum = Math.max(0, Math.min(100, 100 - errorRate));
        return { error: `${errorRate.toFixed(1)}%`, success: `${successRateNum.toFixed(1)}%` };
      };

      const t1 = format({ error_rate: 0 }).error === '0.0%' && format({ error_rate: 0 }).success === '100.0%';
      const t2 = format({ error_rate: 100 }).error === '100.0%' && format({ error_rate: 100 }).success === '0.0%';
      const t3 = format({ error_rate: 14.3 }).error === '14.3%' && format({ error_rate: 14.3 }).success === '85.7%';
      const t4 = format(null).error === '0.0%' && format(null).success === '100.0%';
      const t5 = format(undefined).error === '0.0%' && format(undefined).success === '100.0%';

      return t1 && t2 && t3 && t4 && t5;
    },
    detail: 'Formatting must produce expected strings for 0%, 100%, 14.3%, and null/undefined fallbacks',
  },
];

console.log('--- ezRouter UI Overview Metrics Regression Verification ---');
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
