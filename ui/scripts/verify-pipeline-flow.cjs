/**
 * Automated design regression check for the Request Pipeline Flow curved SVG paths.
 *
 * The pipeline is now provider-agnostic: Client -> Router -> External Upstream.
 * There is a single provider branch (no ag/*, cx/* or image-gen fan-out), so this
 * verifier pins the surviving S-curves, gradients, glow filters, animated flowing
 * dots, idle invariants, mobile connectors and reduced-motion behaviour.
 */

const fs = require('fs');
const path = require('path');

const requestsTabPath = path.resolve(__dirname, '../src/components/RequestsTab.tsx');
const stylesPath = path.resolve(__dirname, '../src/styles.css');

const requestsTabContent = fs.readFileSync(requestsTabPath, 'utf8');
const stylesContent = fs.readFileSync(stylesPath, 'utf8');

const checks = [
  {
    name: 'RequestsTab has curved SVG connectors for Client->Router and Router->Upstream',
    test: () =>
      requestsTabContent.includes('pf-conn-client-router') &&
      requestsTabContent.includes('pf-conn-router-branch') &&
      requestsTabContent.includes('pf-svg-curve'),
    detail: 'Pipeline must define SVG curved connector containers for Client and the Router branch',
  },
  {
    name: 'Curved Bezier/S-curve paths present for the router fan-out and client link',
    test: () => {
      const hasExtCurve = /d="M 0,50 C 45,50 55,87\.5 100,87\.5"/.test(requestsTabContent);
      const hasClientCurve = /d="M 0,20 C 35,10 65,30 100,20"/.test(requestsTabContent);
      return hasExtCurve && hasClientCurve;
    },
    detail: 'Pipeline paths must use cubic Bezier curves from Router to the upstream node and Client to Router',
  },
  {
    name: 'Subtle gradients & SVG glow filters defined for the pipeline',
    test: () =>
      requestsTabContent.includes('pfGradClientRouter') &&
      requestsTabContent.includes('pfGradExt') &&
      requestsTabContent.includes('pfGlowClient') &&
      requestsTabContent.includes('pfGlowExt'),
    detail: 'SVG defs must include linear gradients and glow filters for both pipeline segments',
  },
  {
    name: 'Animated flowing dots (animateMotion) present for both segments when active',
    test: () =>
      requestsTabContent.includes('animateMotion') &&
      requestsTabContent.includes('pf-flowing-dot pf-dot-client') &&
      requestsTabContent.includes('pf-flowing-dot pf-dot-ext'),
    detail: 'Active paths must animate flowing dots along Bezier curves for live requests',
  },
  {
    name: 'Continuous idle paths and junction/terminal anchor pins present',
    test: () =>
      requestsTabContent.includes('pf-path-base') &&
      requestsTabContent.includes('pf-junction-node') &&
      requestsTabContent.includes('active-ext'),
    detail: 'Idle paths must stay continuous and anchor at the junction pin with an upstream terminal node',
  },
  {
    name: 'CSS styles continuous paths and animated active stroke dash',
    test: () => {
      const hasBase = /\.pf-path-base\s*\{[^}]*stroke:[^}]+;[^}]*stroke-width:/s.test(stylesContent);
      const hasActive = /\.pf-path-active\s*\{[^}]*stroke-dasharray:[^}]+;[^}]*animation:\s*pfPathFlow/s.test(stylesContent);
      const hasKeyframe = /@keyframes\s+pfPathFlow\s*\{/.test(stylesContent);
      return hasBase && hasActive && hasKeyframe;
    },
    detail: '.pf-path-base and .pf-path-active must style the continuous dim idle path and the animated dash flow',
  },
  {
    name: 'External upstream branch visual styling in CSS',
    test: () => {
      const hasDot = /\.pf-dot-ext\s*\{[^}]*color:\s*#34d399/s.test(stylesContent);
      const hasTerminal = /\.pf-terminal-node\.active-ext\s*\{[^}]*fill:\s*#10b981/s.test(stylesContent);
      const hasIcon = /\.pf-icon-ext\s*\{[^}]*color:\s*#fbbf24/s.test(stylesContent);
      return hasDot && hasTerminal && hasIcon;
    },
    detail: 'CSS must define distinct emerald/amber accents for the upstream dot, terminal and icon',
  },
  {
    name: 'Model family classification covers OpenRouter and generic upstream models',
    test: () => {
      const hasOpenRouter = /startsWith\(\s*'openrouter\/'\s*\)/.test(requestsTabContent);
      const hasOrTag = requestsTabContent.includes('family-tag-or');
      const hasCustomTag = requestsTabContent.includes('family-tag-custom');
      return hasOpenRouter && hasOrTag && hasCustomTag;
    },
    detail: 'getModelFamily must map openrouter/* to the OR tag and everything else to the upstream tag',
  },
  {
    name: 'Idle state avoids misleading animations when requests are empty',
    test: () => {
      const clientGuard = /\{activeRequests\.length > 0 && \(/s.test(requestsTabContent);
      const extGuard = /\{extActive && \(/s.test(requestsTabContent);
      return clientGuard && extGuard;
    },
    detail: 'Active paths and flowing dots must be guarded by active state to prevent idle animations',
  },
  {
    name: 'Mobile responsiveness: branch connector and 390px safety',
    test: () => {
      const countRows = (requestsTabContent.match(/className="pf-branch-row"/g) || []).length;
      const countSubSvgs = (requestsTabContent.match(/className="pf-svg-curve pf-svg-mobile-sub"/g) || []).length;
      const hasDesktop = /\.pf-svg-desktop\s*\{[^}]*display:\s*block/s.test(stylesContent);
      const hasMobile = /\.pf-svg-mobile\s*\{[^}]*display:\s*none/s.test(stylesContent);
      const hasMobileMedia = /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.pf-svg-desktop\s*\{[^}]*display:\s*none[\s\S]*?\.pf-svg-mobile\s*\{[^}]*display:\s*block/s.test(stylesContent);
      return countRows === 1 && countSubSvgs === 1 && hasDesktop && hasMobile && hasMobileMedia;
    },
    detail: 'CSS and JSX must provide a mobile vertical connector for the single upstream branch',
  },
  {
    name: 'prefers-reduced-motion disables flowing dots and path animations',
    test: () =>
      /@media\s*\(prefers-reduced-motion:\s*reduce\)[^{]*\{[\s\S]*?\.pf-flowing-dot[\s\S]*?animateMotion[\s\S]*?\.pf-path-active\s*\{[^}]*animation:\s*none/s.test(stylesContent),
    detail: 'Reduced motion query must disable .pf-flowing-dot, animateMotion and .pf-path-active animations',
  },
  {
    name: 'No legacy ag/cx/image-gen pipeline branches remain',
    test: () =>
      !/pfGradAg|pfGradCx|pfGlowAg|pfGlowCx|pf-dot-ag|pf-dot-cx|active-ag|active-cx|isCodexImageModel/.test(requestsTabContent) &&
      !/pfGradAg|pfGradCx|pfGlowAg|pfGlowCx|pf-dot-ag|pf-dot-cx|active-ag|active-cx|pf-badge-image/.test(stylesContent),
    detail: 'Antigravity/Codex/image-gen branch gradients, dots, terminals and badges must be fully removed',
  },
];

console.log('--- ezRouter UI Request Pipeline Flow Design Regression Verification ---');
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
