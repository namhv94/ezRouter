/**
 * Regression check for UI Model Dropdowns.
 * Verifies that canonical 9router-compatible Gemini Pro models:
 *   - ag/gemini-pro-agent
 *   - ag/gemini-3.1-pro-low
 * are present in UI default lists and dropdowns, while unsupported raw models are not.
 */

const fs = require('fs');
const path = require('path');

const playgroundPath = path.resolve(__dirname, '../src/components/PlaygroundTab.tsx');
const integrationPath = path.resolve(__dirname, '../src/components/IntegrationTab.tsx');

const playgroundContent = fs.readFileSync(playgroundPath, 'utf8');
const integrationContent = fs.readFileSync(integrationPath, 'utf8');

const checks = [
  {
    name: 'Playground DEFAULT_AG_MODELS includes ag/gemini-4-argon and ag/gemini-4-argon-high',
    test: () => playgroundContent.includes("'ag/gemini-4-argon'") && playgroundContent.includes("'ag/gemini-4-argon-high'"),
    detail: 'DEFAULT_AG_MODELS must include canonical ag/gemini-4-argon and ag/gemini-4-argon-high',
  },
  {
    name: 'Playground DEFAULT_AG_MODELS includes ag/gemini-pro-agent',
    test: () => playgroundContent.includes("'ag/gemini-pro-agent'"),
    detail: 'DEFAULT_AG_MODELS must include canonical ag/gemini-pro-agent',
  },
  {
    name: 'Playground DEFAULT_AG_MODELS includes ag/gemini-3.1-pro-low',
    test: () => playgroundContent.includes("'ag/gemini-3.1-pro-low'"),
    detail: 'DEFAULT_AG_MODELS must include canonical ag/gemini-3.1-pro-low',
  },
  {
    name: 'Playground DEFAULT_AG_MODELS does not expose raw unsupported gemini-3.1-pro without ag/ prefix',
    test: () => !/['"]gemini-3\.1-pro['"]/.test(playgroundContent),
    detail: 'Unprefixed gemini-3.1-pro must not be exposed in static default options',
  },
  {
    name: 'IntegrationTab model selection includes ag/gemini-4-argon',
    test: () => integrationContent.includes('value="ag/gemini-4-argon"'),
    detail: 'IntegrationTab select dropdown must include ag/gemini-4-argon',
  },
  {
    name: 'IntegrationTab model selection includes ag/gemini-pro-agent',
    test: () => integrationContent.includes('value="ag/gemini-pro-agent"'),
    detail: 'IntegrationTab select dropdown must include ag/gemini-pro-agent',
  },
  {
    name: 'IntegrationTab model selection includes ag/gemini-3.1-pro-low',
    test: () => integrationContent.includes('value="ag/gemini-3.1-pro-low"'),
    detail: 'IntegrationTab select dropdown must include ag/gemini-3.1-pro-low',
  },
  {
    name: 'Playground DEFAULT_CX_MODELS includes Codex thinking models (cx/gpt-6.1-sol-high, cx/gpt-5.6-sol-high, cx/gpt-5.6-sol-ultra)',
    test: () =>
      playgroundContent.includes("'cx/gpt-6.1-sol-high'") &&
      playgroundContent.includes("'cx/gpt-5.6-sol-high'") &&
      playgroundContent.includes("'cx/gpt-5.6-sol-ultra'"),
    detail: 'DEFAULT_CX_MODELS must include cx/gpt-6.1-sol-high, cx/gpt-5.6-sol-high, and cx/gpt-5.6-sol-ultra',
  },
  {
    name: 'Playground thinking level options include xhigh, max, and ultra',
    test: () =>
      playgroundContent.includes('value="xhigh"') &&
      playgroundContent.includes('value="max"') &&
      playgroundContent.includes('value="ultra"'),
    detail: 'Playground thinking level select must include xhigh, max, and ultra options',
  },
  {
    name: 'IntegrationTab model selection includes Codex thinking models (cx/gpt-6.1-sol-high, cx/gpt-5.6-sol-high, cx/gpt-5.6-sol-ultra)',
    test: () =>
      integrationContent.includes('value="cx/gpt-6.1-sol-high"') &&
      integrationContent.includes('value="cx/gpt-5.6-sol-high"') &&
      integrationContent.includes('value="cx/gpt-5.6-sol-ultra"'),
    detail: 'IntegrationTab select dropdown must include cx/gpt-6.1-sol-high, cx/gpt-5.6-sol-high, and cx/gpt-5.6-sol-ultra',
  },
  {
    name: 'IntegrationTab thinking level options include xhigh, max, and ultra',
    test: () =>
      integrationContent.includes('value="xhigh"') &&
      integrationContent.includes('value="max"') &&
      integrationContent.includes('value="ultra"'),
    detail: 'IntegrationTab thinking level select must include xhigh, max, and ultra options',
  },
];

console.log('--- AG-Proxy UI Model Dropdown Verification ---');
let passed = 0;
let failed = 0;

for (const check of checks) {
  const result = check.test();
  if (result) {
    console.log(`PASS: ${check.name}`);
    passed++;
  } else {
    console.error(`FAIL: ${check.name}`);
    console.error(`  Detail: ${check.detail}`);
    failed++;
  }
}

console.log(`\nTotal checks: ${checks.length}, Passed: ${passed}, Failed: ${failed}`);
if (failed > 0) {
  process.exit(1);
}
console.log('All model dropdown invariants verified successfully.\n');
