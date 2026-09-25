/**
 * Automated design regression check for Request Pipeline Flow curved SVG paths.
 * Verifies S-curves, responsive branch fan-out, gradients, glow filters,
 * active animated flowing dots, and reduced-motion invariants.
 */

const fs = require('fs');
const path = require('path');

const requestsTabPath = path.resolve(__dirname, '../src/components/RequestsTab.tsx');
const stylesPath = path.resolve(__dirname, '../src/styles.css');

const requestsTabContent = fs.readFileSync(requestsTabPath, 'utf8');
const stylesContent = fs.readFileSync(stylesPath, 'utf8');

const checks = [
  {
    name: 'RequestsTab has curved SVG connectors for Client->Router and Router->Branches',
    test: () => {
      return (
        requestsTabContent.includes('pf-conn-client-router') &&
        requestsTabContent.includes('pf-conn-router-branch') &&
        requestsTabContent.includes('pf-svg-curve')
      );
    },
    detail: 'Pipeline must define SVG curved connector containers for Client and Router branch fan-out',
  },
  {
    name: 'Curved Bezier/S-curve paths present for natural branch fan-out',
    test: () => {
      const hasAgCurve = /d="M 0,50 C 45,50 55,16\.7 100,16\.7"/.test(requestsTabContent);
      const hasCxCurve = /d="M 0,50 C 35,50 65,50 100,50"/.test(requestsTabContent);
      const hasExtCurve = /d="M 0,50 C 45,50 55,83\.3 100,83\.3"/.test(requestsTabContent);
      const hasClientCurve = /d="M 0,20 C 35,10 65,30 100,20"/.test(requestsTabContent);
      return hasAgCurve && hasCxCurve && hasExtCurve && hasClientCurve;
    },
    detail: 'Pipeline paths must use cubic Bezier curves fanning out naturally from Router to all provider nodes',
  },
  {
    name: 'Subtle gradients & SVG glow filters defined for pipeline routes',
    test: () => {
      return (
        requestsTabContent.includes('pfGradClientRouter') &&
        requestsTabContent.includes('pfGradAg') &&
        requestsTabContent.includes('pfGradCx') &&
        requestsTabContent.includes('pfGradExt') &&
        requestsTabContent.includes('pfGlowAg') &&
        requestsTabContent.includes('pfGlowCx') &&
        requestsTabContent.includes('pfGlowExt')
      );
    },
    detail: 'SVG defs must include linear gradients and glow filters for all route branches',
  },
  {
    name: 'Animated flowing dots (animateMotion) present when active',
    test: () => {
      return (
        requestsTabContent.includes('animateMotion') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-client') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-ag') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-cx') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-ext')
      );
    },
    detail: 'Active paths must animate flowing dots along Bezier curves for live requests',
  },
  {
    name: 'Continuous idle paths and junction/terminal anchor pins present',
    test: () => {
      return (
        requestsTabContent.includes('pf-path-base') &&
        requestsTabContent.includes('pf-junction-node') &&
        requestsTabContent.includes('pf-terminal-node')
      );
    },
    detail: 'Idle paths must stay continuous and anchor at junction pins without disconnected stubs',
  },
  {
    name: 'CSS styles continuous paths and animated active stroke dash',
    test: () => {
      const hasBase = /\.pf-path-base\s*\{[^}]*stroke:[^}]+;[^}]*stroke-width:/s.test(stylesContent);
      const hasActive = /\.pf-path-active\s*\{[^}]*stroke-dasharray:[^}]+;[^}]*animation:\s*pfPathFlow/s.test(stylesContent);
      const hasKeyframe = /@keyframes\s+pfPathFlow\s*\{/s.test(stylesContent);
      return hasBase && hasActive && hasKeyframe;
    },
    detail: '.pf-path-base and .pf-path-active must style continuous dim idle path and animated dash flow',
  },
  {
    name: 'Mobile responsiveness overrides (desktop fan-out vs mobile organic tree)',
    test: () => {
      const hasDesktop = /\.pf-svg-desktop\s*\{[^}]*display:\s*block/s.test(stylesContent);
      const hasMobile = /\.pf-svg-mobile\s*\{[^}]*display:\s*none/s.test(stylesContent);
      const hasMobileMedia = /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.pf-svg-desktop\s*\{[^}]*display:\s*none[\s\S]*?\.pf-svg-mobile\s*\{[^}]*display:\s*block/s.test(stylesContent);
      return hasDesktop && hasMobile && hasMobileMedia;
    },
    detail: 'CSS must switch between desktop 3-branch fan-out SVG and mobile vertical curved tree',
  },
  {
    name: 'prefers-reduced-motion disables flowing dots and path animations',
    test: () => {
      return /@media\s*\(prefers-reduced-motion:\s*reduce\)[^{]*\{[\s\S]*?\.pf-flowing-dot[\s\S]*?animateMotion[\s\S]*?\.pf-path-active\s*\{[^}]*animation:\s*none/s.test(stylesContent);
    },
    detail: 'Reduced motion query must disable .pf-flowing-dot, animateMotion, and .pf-path-active animations',
  },
];

console.log('--- AG-Proxy UI Request Pipeline Flow Design Regression Verification ---');
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
}
console.log('All pipeline flow design regression invariants verified successfully.');
