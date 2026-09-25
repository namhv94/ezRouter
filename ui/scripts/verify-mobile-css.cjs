/**
 * Lightweight automated verification of mobile UI & CSS invariants.
 * Verifies responsive layout rules, touch targets, overflow containment,
 * viewport fit for modals, and iOS zoom prevention.
 */

const fs = require('fs');
const path = require('path');

const stylesPath = path.resolve(__dirname, '../src/styles.css');
const stylesContent = fs.readFileSync(stylesPath, 'utf8');

const checks = [
  {
    name: 'Root / App horizontal overflow containment',
    test: () => {
      return (
        /html,\s*body\s*\{[^}]*overflow-x:\s*hidden/s.test(stylesContent) &&
        /\.app-container\s*\{[^}]*overflow-x:\s*hidden/s.test(stylesContent) &&
        /\.main-wrapper\s*\{[^}]*overflow-x:\s*hidden/s.test(stylesContent) &&
        /\.main-wrapper\s*\{[^}]*min-width:\s*0/s.test(stylesContent)
      );
    },
    detail: 'html, body, .app-container, .main-wrapper must contain overflow-x: hidden and min-width: 0',
  },
  {
    name: 'Touch target invariants (>=44px)',
    test: () => {
      const btnMinHeight = /\.btn\s*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const btnSmMobile = /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.btn-sm\s*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const btnIconOnly = /\.btn-icon-only\s*\{[^}]*min-width:\s*44px/s.test(stylesContent) &&
                          /\.btn-icon-only\s*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const menuToggle = /\.menu-toggle-btn\s*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const sidebarClose = /\.sidebar-close-btn\s*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const navItem = /\.nav-item\s*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const inputs = /input\[type="text"\][\s\S]*?\{[^}]*min-height:\s*44px/s.test(stylesContent);

      return (
        btnMinHeight &&
        btnSmMobile &&
        btnIconOnly &&
        menuToggle &&
        sidebarClose &&
        navItem &&
        inputs
      );
    },
    detail: 'Buttons, toggle buttons, close buttons, nav items, and form inputs must meet >=44px touch targets',
  },
  {
    name: 'iOS Safari auto-zoom prevention (font-size >= 16px)',
    test: () => {
      return /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?font-size:\s*16px/s.test(stylesContent);
    },
    detail: 'Inputs on mobile viewports (<=768px) must use font-size: 16px to prevent automatic zooming',
  },
  {
    name: 'Table internal scroll containment',
    test: () => {
      return (
        /\.table-container\s*\{[^}]*overflow-x:\s*auto/s.test(stylesContent) &&
        /table\s*\{[^}]*min-width:\s*640px/s.test(stylesContent)
      );
    },
    detail: '.table-container must scroll horizontally with min-width >= 640px on table',
  },
  {
    name: 'Modal & login viewport fit',
    test: () => {
      return (
        /\.modal-backdrop\s*\{[^}]*overflow-y:\s*auto/s.test(stylesContent) &&
        /\.modal-card\s*\{[^}]*max-height:\s*calc\(100(?:vh|dvh)\s*-\s*\d+px\)/s.test(stylesContent) &&
        /\.modal-card\s*\{[^}]*overflow-y:\s*auto/s.test(stylesContent)
      );
    },
    detail: '.modal-card must be scrollable internally with max-height bounded to viewport',
  },
  {
    name: 'Chart width containment',
    test: () => {
      return (
        /\.chart-bar-container\s*\{[^}]*overflow-x:\s*auto/s.test(stylesContent) &&
        /\.chart-bar-container\s*\{[^}]*width:\s*100%/s.test(stylesContent)
      );
    },
    detail: '.chart-bar-container must fit width with overflow-x: auto and no layout bursting',
  },
  {
    name: 'Top navbar mobile collapse and compaction',
    test: () => {
      return (
        /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.nav-key-badge\s*\{[^}]*display:\s*none/s.test(stylesContent) &&
        /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.nav-action-btn\s*\.btn-label\s*\{[^}]*display:\s*none/s.test(stylesContent)
      );
    },
    detail: 'Navbar must compact admin key badge and text labels on <=768px viewports',
  },
  {
    name: 'Sidebar drawer mobile responsiveness',
    test: () => {
      return (
        /@media\s*\([^)]*max-width:\s*900px\)[^{]*\{[\s\S]*?\.sidebar\.open\s*\{[^}]*transform:\s*translateX\(0\)/s.test(stylesContent) &&
        /@media\s*\([^)]*max-width:\s*900px\)[^{]*\{[\s\S]*?\.sidebar-close-btn\s*\{[^}]*display:\s*inline-flex/s.test(stylesContent)
      );
    },
    detail: 'Sidebar drawer must slide in/out and provide mobile close button',
  },
];

console.log('--- AG-Proxy UI Mobile CSS Invariant Verification ---');
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
  console.log('All mobile CSS invariants verified successfully.\n');
}
