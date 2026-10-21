/**
 * Automated design regression check for Request Pipeline Flow curved SVG paths.
 * Verifies S-curves, responsive branch fan-out (4 branches: AG, CX, CX Image, Ext),
 * gradients, glow filters, active animated flowing dots, model alias classification,
 * idle state invariants, and reduced-motion invariants.
 */

const fs = require('fs');
const path = require('path');
const ts = require('typescript');
const vm = require('vm');

const requestsTabPath = path.resolve(__dirname, '../src/components/RequestsTab.tsx');
const stylesPath = path.resolve(__dirname, '../src/styles.css');

const requestsTabContent = fs.readFileSync(requestsTabPath, 'utf8');
const stylesContent = fs.readFileSync(stylesPath, 'utf8');

// Helper to evaluate isCodexImageModel in isolated sandbox
function getIsCodexImageModel() {
  const match = requestsTabContent.match(
    /export const isCodexImageModel = \([^)]*\)(?::\s*[^{]+)?\s*=>\s*\{[\s\S]*?\n\};/
  );
  if (!match) return null;
  const code = match[0].replace('export const', 'const') + '\nexports.isCodexImageModel = isCodexImageModel;\n';
  const transpiled = ts.transpileModule(code, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const sandbox = { exports: {} };
  vm.createContext(sandbox);
  vm.runInContext(transpiled, sandbox);
  return sandbox.exports.isCodexImageModel;
}

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
    name: 'Curved Bezier/S-curve paths present for 4-branch natural fan-out',
    test: () => {
      const hasAgCurve = /d="M 0,50 C 45,50 55,12\.5 100,12\.5"/.test(requestsTabContent);
      const hasCxCurve = /d="M 0,50 C 45,50 55,37\.5 100,37\.5"/.test(requestsTabContent);
      const hasCxImgCurve = /d="M 0,50 C 45,50 55,62\.5 100,62\.5"/.test(requestsTabContent);
      const hasExtCurve = /d="M 0,50 C 45,50 55,87\.5 100,87\.5"/.test(requestsTabContent);
      const hasClientCurve = /d="M 0,20 C 35,10 65,30 100,20"/.test(requestsTabContent);
      return hasAgCurve && hasCxCurve && hasCxImgCurve && hasExtCurve && hasClientCurve;
    },
    detail: 'Pipeline paths must use cubic Bezier curves fanning out naturally from Router to all 4 provider nodes (12.5, 37.5, 62.5, 87.5)',
  },
  {
    name: 'Subtle gradients & SVG glow filters defined for all 4 pipeline branches',
    test: () => {
      return (
        requestsTabContent.includes('pfGradClientRouter') &&
        requestsTabContent.includes('pfGradAg') &&
        requestsTabContent.includes('pfGradCx') &&
        requestsTabContent.includes('pfGradCxImg') &&
        requestsTabContent.includes('pfGradExt') &&
        requestsTabContent.includes('pfGlowAg') &&
        requestsTabContent.includes('pfGlowCx') &&
        requestsTabContent.includes('pfGlowCxImg') &&
        requestsTabContent.includes('pfGlowExt')
      );
    },
    detail: 'SVG defs must include linear gradients and glow filters for all 4 route branches including pfGradCxImg and pfGlowCxImg',
  },
  {
    name: 'Animated flowing dots (animateMotion) present for all branches when active',
    test: () => {
      return (
        requestsTabContent.includes('animateMotion') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-client') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-ag') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-cx') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-cx-img') &&
        requestsTabContent.includes('pf-flowing-dot pf-dot-ext')
      );
    },
    detail: 'Active paths must animate flowing dots along Bezier curves for live requests on all branches',
  },
  {
    name: 'Continuous idle paths and junction/terminal anchor pins present for all 4 branches',
    test: () => {
      const hasBase = requestsTabContent.includes('pf-path-base');
      const hasJunction = requestsTabContent.includes('pf-junction-node');
      const hasTerminalAg = requestsTabContent.includes('active-ag');
      const hasTerminalCx = requestsTabContent.includes('active-cx');
      const hasTerminalCxImg = requestsTabContent.includes('active-cx-img');
      const hasTerminalExt = requestsTabContent.includes('active-ext');
      return hasBase && hasJunction && hasTerminalAg && hasTerminalCx && hasTerminalCxImg && hasTerminalExt;
    },
    detail: 'Idle paths must stay continuous and anchor at junction pins with terminal nodes for all 4 branches',
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
    name: 'Codex Image Gen 4th branch visual styling & badge in CSS',
    test: () => {
      const hasDot = /\.pf-dot-cx-img\s*\{[^}]*color:\s*#fb7185/s.test(stylesContent);
      const hasTerminal = /\.pf-terminal-node\.active-cx-img\s*\{[^}]*fill:\s*#f43f5e/s.test(stylesContent);
      const hasIcon = /\.pf-icon-cx-img\s*\{[^}]*color:\s*#fb7185/s.test(stylesContent);
      const hasBadge = /\.pf-badge-image\s*\{/s.test(stylesContent);
      return hasDot && hasTerminal && hasIcon && hasBadge;
    },
    detail: 'CSS must define distinct rose-accented styles for image-gen dot, terminal, icon, and badge',
  },
  {
    name: 'Model alias classification helper handles image-gen aliases and protects generic Codex',
    test: () => {
      const isCodexImageModel = getIsCodexImageModel();
      if (!isCodexImageModel) return false;

      // Positive cases: all image-gen aliases (backend is_codex_image_model)
      const positiveAliases = [
        ['cx/image-gen', ''],
        ['image-gen', ''],
        ['cx/gpt-image', ''],
        ['cx/gpt-image-2', ''],
        ['gpt-image-2', ''],
        ['codex-image', ''],
        ['cx/image-flux', ''],
        ['cx/gpt-image-hd', ''],
        ['', 'Codex Image'],
        ['', 'cx/image-gen'],
        ['', 'image-gen'],
      ];
      for (const [m, p] of positiveAliases) {
        if (!isCodexImageModel(m, p)) {
          console.error(`  Expected isCodexImageModel('${m}', '${p}') to be true`);
          return false;
        }
      }

      // Negative cases: generic Codex chat/completions and other models must NOT match
      const negativeCases = [
        ['cx/gpt-4o', 'Codex'],
        ['cx/o3-mini', 'Codex'],
        ['codex-davinci', 'Codex'],
        ['ag/gemini-2.5-pro', 'Google Antigravity'],
        ['openai/gpt-4o', 'OpenRouter'],
        ['claude-3-5-sonnet', 'Anthropic'],
      ];
      for (const [m, p] of negativeCases) {
        if (isCodexImageModel(m, p)) {
          console.error(`  Expected isCodexImageModel('${m}', '${p}') to be false`);
          return false;
        }
      }

      return true;
    },
    detail: 'isCodexImageModel must classify image-gen aliases accurately while leaving chat models to cxActive',
  },
  {
    name: 'cx/image-gen priority check ensures it is not swallowed by cxActive',
    test: () => {
      // Must check cxImageActive before agActive, cxActive, extActive
      const idxCxImage = requestsTabContent.indexOf('const cxImageActive =');
      const idxAg = requestsTabContent.indexOf('const agActive =');
      const idxCx = requestsTabContent.indexOf('const cxActive =');
      const idxExt = requestsTabContent.indexOf('const extActive =');

      if (idxCxImage === -1 || idxAg === -1 || idxCx === -1 || idxExt === -1) {
        return false;
      }
      return idxCxImage < idxAg && idxCxImage < idxCx && idxCx < idxExt;
    },
    detail: 'cxImageActive must be resolved before cxActive and excluded from generic cxActive',
  },
  {
    name: 'Idle state avoids misleading animations when requests are empty',
    test: () => {
      // Active paths and flowing dots must only be conditioned on active variables
      const agActiveGuard = /\{agActive\s*&&\s*\(/s.test(requestsTabContent);
      const cxActiveGuard = /\{cxActive\s*&&\s*\(/s.test(requestsTabContent);
      const cxImageActiveGuard = /\{cxImageActive\s*&&\s*\(/s.test(requestsTabContent);
      const extActiveGuard = /\{extActive\s*&&\s*\(/s.test(requestsTabContent);
      return agActiveGuard && cxActiveGuard && cxImageActiveGuard && extActiveGuard;
    },
    detail: 'Active paths and flowing dots must be guarded by active state to prevent idle animations',
  },
  {
    name: 'Mobile responsiveness: all 4 branches have organic mobile connectors and 390px safety',
    test: () => {
      const countRows = (requestsTabContent.match(/className="pf-branch-row"/g) || []).length;
      const countSubSvgs = (requestsTabContent.match(/className="pf-svg-curve pf-svg-mobile-sub"/g) || []).length;
      const hasDesktop = /\.pf-svg-desktop\s*\{[^}]*display:\s*block/s.test(stylesContent);
      const hasMobile = /\.pf-svg-mobile\s*\{[^}]*display:\s*none/s.test(stylesContent);
      const hasMobileMedia = /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.pf-svg-desktop\s*\{[^}]*display:\s*none[\s\S]*?\.pf-svg-mobile\s*\{[^}]*display:\s*block/s.test(stylesContent);
      return countRows === 4 && countSubSvgs === 4 && hasDesktop && hasMobile && hasMobileMedia;
    },
    detail: 'CSS and JSX must provide mobile vertical connectors for all 4 branches',
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
