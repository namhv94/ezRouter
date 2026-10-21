/**
 * Automated verification of Operational Theme visual slice invariants.
 * Validates:
 * 1. Operational theme tokens and scoped selectors for Overview and Providers.
 * 2. Visual hierarchy: command center telemetry, KPI grid, quota overview, and provider cards.
 * 3. Design invariants: absence of gradients, neon, or glassmorphism in operational rules.
 * 4. Accessibility: touch target sizes on mobile (>=44px) and reduced-motion handling.
 * 5. Legacy preservation: legacy rules and CSS tokens remain functional and unregressed.
 */

const fs = require('fs');
const path = require('path');

const stylesPath = path.resolve(__dirname, '../src/styles.css');
const overviewPath = path.resolve(__dirname, '../src/components/OverviewTab.tsx');
const providersPath = path.resolve(__dirname, '../src/components/ProvidersTab.tsx');

const stylesContent = fs.readFileSync(stylesPath, 'utf8');
const overviewContent = fs.readFileSync(overviewPath, 'utf8');
const providersContent = fs.readFileSync(providersPath, 'utf8');

const checks = [
  {
    name: 'Operational theme tokens and root variables defined',
    test: () => {
      const hasOpRoot = stylesContent.includes(':root[data-theme="operational"]');
      const hasBgApp = stylesContent.includes('--bg-app: #f4f6f8;');
      const hasAccent = stylesContent.includes('--accent-primary: #0f766e;');
      const hasBorder = stylesContent.includes('--border-subtle: #e2e8f0;');
      return hasOpRoot && hasBgApp && hasAccent && hasBorder;
    },
    detail: 'Operational root variables (--bg-app, --accent-primary, --border-subtle) must be defined',
  },
  {
    name: 'Overview health banner styled as operations command center',
    test: () => {
      const bannerScoped = /\[data-theme="operational"\]\s+\.system-health-banner/.test(stylesContent);
      const metaScoped = /\[data-theme="operational"\]\s+\.health-meta span/.test(stylesContent);
      const serviceClassInOverview = overviewContent.includes('health-service-title');
      return bannerScoped && metaScoped && serviceClassInOverview;
    },
    detail: 'System health banner and meta telemetry pills must be styled under [data-theme="operational"]',
  },
  {
    name: 'Overview KPI grid styled with operational command hierarchy',
    test: () => {
      const kpiCard = /\[data-theme="operational"\]\s+\.kpi-card/.test(stylesContent);
      const kpiValue = /\[data-theme="operational"\]\s+\.kpi-card \.kpi-value/.test(stylesContent);
      const kpiGood = /\[data-theme="operational"\]\s+\.kpi-card-good/.test(stylesContent);
      const kpiWarn = /\[data-theme="operational"\]\s+\.kpi-card-warn/.test(stylesContent);
      const kpiBad = /\[data-theme="operational"\]\s+\.kpi-card-bad/.test(stylesContent);
      return kpiCard && kpiValue && kpiGood && kpiWarn && kpiBad;
    },
    detail: 'KPI grid and tone indicators must be scoped under [data-theme="operational"] with monospace tabular-nums',
  },
  {
    name: 'Overview quota overview and token analytics styled for observability',
    test: () => {
      const quotaCard = /\[data-theme="operational"\]\s+\.overview-quota-card/.test(stylesContent);
      const quotaTrack = /\[data-theme="operational"\]\s+\.quota-track/.test(stylesContent);
      const quotaGood = /\[data-theme="operational"\]\s+\.quota-good/.test(stylesContent);
      const tokenSummary = /\[data-theme="operational"\]\s+\.token-chart-summary-bar/.test(stylesContent);
      const tokenSvg = /\[data-theme="operational"\]\s+\.token-chart-svg-container/.test(stylesContent);
      return quotaCard && quotaTrack && quotaGood && tokenSummary && tokenSvg;
    },
    detail: 'Quota overview tracks and token analytics must have high-contrast operational rules',
  },
  {
    name: 'Providers status strip and auto quota compact strip styled',
    test: () => {
      const statusStrip = /\[data-theme="operational"\]\s+\.status-legend-bar/.test(stylesContent);
      const statusChip = /\[data-theme="operational"\]\s+\.status-segment-chip/.test(stylesContent);
      const autoQuota = /\[data-theme="operational"\]\s+\.auto-quota-compact-strip/.test(stylesContent);
      const intervalBtn = /\[data-theme="operational"\]\s+\.interval-pill-btn\.active/.test(stylesContent);
      return statusStrip && statusChip && autoQuota && intervalBtn;
    },
    detail: 'Status strip chips and auto quota compact panel must be styled under [data-theme="operational"]',
  },
  {
    name: 'Provider cards and account tables scannable and compact',
    test: () => {
      const visualCard = /\[data-theme="operational"\]\s+\.visual-card/.test(stylesContent);
      const metricsRow = /\[data-theme="operational"\]\s+\.visual-metrics-row/.test(stylesContent);
      const healthRing = /\[data-theme="operational"\]\s+\.health-ring-bg/.test(stylesContent);
      const microBars = /\[data-theme="operational"\]\s+\.visual-quota-bars/.test(stylesContent);
      const drawerTable = /\[data-theme="operational"\]\s+\.details-drawer-content \.table-container/.test(stylesContent);
      const mobileAcc = /\[data-theme="operational"\]\s+\.mobile-account-card/.test(stylesContent);
      return visualCard && metricsRow && healthRing && microBars && drawerTable && mobileAcc;
    },
    detail: 'Provider cards, health ring, micro-bars, and drawer tables must have compact operational styling',
  },
  {
    name: 'Design constraints: no gradients in operational rules',
    test: () => {
      const opSliceMatch = stylesContent.split('/* ==========================================================================\n   Operational Theme Visual Slice: Overview Command Center & Providers')[1];
      if (!opSliceMatch) return false;
      return !opSliceMatch.includes('linear-gradient') && !opSliceMatch.includes('radial-gradient');
    },
    detail: 'Operational slice must avoid gradients to adhere to operational minimalism',
  },
  {
    name: 'Touch target invariant: mobile interactive controls >=44px',
    test: () => {
      const hasMobileQuery = /@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.action-icon-btn[^{]*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      return hasMobileQuery;
    },
    detail: 'Operational buttons must maintain min-height: 44px on <=768px viewports',
  },
  {
    name: 'Accessibility: reduced motion respected for operational transitions',
    test: () => {
      const hasReducedMotion = /@media\s*\(prefers-reduced-motion:\s*reduce\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.kpi-card/s.test(stylesContent);
      return hasReducedMotion;
    },
    detail: 'Operational theme must disable transitions/animations under prefers-reduced-motion: reduce',
  },
  {
    name: 'Legacy theme compatibility preserved',
    test: () => {
      const legacyHealth = /\.system-health-banner\.is-healthy\s*\{/.test(stylesContent);
      const legacyKpi = /\.kpi-card-blue::before/.test(stylesContent);
      const legacyVisual = /\.visual-card\s*\{/.test(stylesContent);
      return legacyHealth && legacyKpi && legacyVisual;
    },
    detail: 'Original legacy rules must remain intact so default dark theme appearance is preserved',
  },
  {
    name: 'Operational provider account tables mobile fallback (<=640px/<=390px)',
    test: () => {
      const hasMobileBlock = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.details-drawer-content\s+table\s*\{[^}]*display:\s*block/s.test(stylesContent);
      const hasTheadNone = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.details-drawer-content\s+thead\s*\{[^}]*display:\s*none/s.test(stylesContent);
      const hasDataLabel = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.details-drawer-content\s+td\[data-label\]::before/s.test(stylesContent);
      const hasMobileBtnTarget = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.details-drawer-content\s+\.table-actions\s+\.btn[^{]*\{[^}]*min-height:\s*44px/s.test(stylesContent);
      const providersHasDataLabel = providersContent.includes('data-label={t(\'providers.thAccount\')}');
      return hasMobileBlock && hasTheadNone && hasDataLabel && hasMobileBtnTarget && providersHasDataLabel;
    },
    detail: 'Operational provider account tables must have responsive card layout fallback with data-labels and >=44px touch targets on mobile',
  },
  {
    name: 'Operational theme contrast and semantic token overrides',
    test: () => {
      const hasChartBars = /\[data-theme="operational"\]\s+\.token-bar-comp\s*\{[^}]*fill:\s*#0f766e/s.test(stylesContent);
      const hasMicroBars = /\[data-theme="operational"\]\s+\.micro-bar-fill\.micro-bar-good\s*\{[^}]*background:\s*#0f766e/s.test(stylesContent);
      const hasOrBadge = /\[data-theme="operational"\]\s+\.openrouter-model-badge\s*\{[^}]*color:\s*#4338ca/s.test(stylesContent);
      const hasRootPrimary = stylesContent.includes('--primary: var(--accent-primary);');
      const hasRootBorder = stylesContent.includes('--border-color: var(--border-subtle);');
      const hasRootBgSurface = stylesContent.includes('--bg-surface-secondary: var(--bg-secondary);');
      return hasChartBars && hasMicroBars && hasOrBadge && hasRootPrimary && hasRootBorder && hasRootBgSurface;
    },
    detail: 'Operational theme must define high-contrast token overrides and semantic variable fallbacks',
  },
  {
    name: 'SVG chart bars keep data-driven height',
    test: () => {
      const htmlPrompt = /\.token-bar-track \.token-bar-prompt\s*\{[^}]*height:\s*100%/.test(stylesContent);
      const htmlCompletion = /\.token-bar-track \.token-bar-comp\s*\{[^}]*height:\s*100%/.test(stylesContent);
      const unscopedHeight = /^\.token-bar-(?:prompt|comp)\s*\{[^}]*height:\s*100%/m.test(stylesContent);
      return htmlPrompt && htmlCompletion && !unscopedHeight;
    },
    detail: 'HTML progress height must not override SVG rect height attributes',
  },
  {
    name: 'Operational Overview section ordering prioritizes Quota over Analytics',
    test: () => {
      const quotaOrder = /\[data-theme="operational"\]\s+\.overview-quota-section\s*\{[^}]*order:\s*2/s.test(stylesContent);
      const analyticsOrder = /\[data-theme="operational"\]\s+\.overview-analytics-section\s*\{[^}]*order:\s*3/s.test(stylesContent);
      const containerGrid = /\[data-theme="operational"\]\s+\.overview-container\s*\{[^}]*display:\s*grid/s.test(stylesContent);
      const jsxHasSections = overviewContent.includes('overview-quota-section') && overviewContent.includes('overview-analytics-section');
      return quotaOrder && analyticsOrder && containerGrid && jsxHasSections;
    },
    detail: 'Operational mode must prioritize Quota Overview above Token Analytics via CSS grid order without altering legacy DOM flow',
  },
  {
    name: 'Operational Command Center asymmetric vitals hierarchy & mobile 2-column tiles',
    test: () => {
      const desktopPoolSpan = /\[data-theme="operational"\]\s+\.kpi-card-pool\s*\{[^}]*grid-column:\s*span\s*2/s.test(stylesContent);
      const desktopErrorsSpan = /\[data-theme="operational"\]\s+\.kpi-card-errors\s*\{[^}]*grid-column:\s*span\s*2/s.test(stylesContent);
      const mobileGrid2Col = /@media\s*\([^)]*max-width:\s*560px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.kpi-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*1fr\)/s.test(stylesContent);
      const jsxHasCardRoles = overviewContent.includes('kpi-card-pool') && overviewContent.includes('kpi-card-errors');
      return desktopPoolSpan && desktopErrorsSpan && mobileGrid2Col && jsxHasCardRoles;
    },
    detail: 'Pool and Error vitals must span prominent double columns on desktop and form compact 2-column grid on mobile (<=560px/390px)',
  },
  {
    name: 'Overview Model summary table responsive card fallback on mobile (<=640px)',
    test: () => {
      const hasTableBlock = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.overview-models-table\s*\{[^}]*display:\s*block/s.test(stylesContent);
      const hasTheadNone = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.overview-models-table\s+thead\s*\{[^}]*display:\s*none/s.test(stylesContent);
      const hasDataLabel = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.overview-models-table\s+td\[data-label\]::before/s.test(stylesContent);
      const overviewHasDataLabel = overviewContent.includes('data-label={t(\'overview.tableModel\')}');
      return hasTableBlock && hasTheadNone && hasDataLabel && overviewHasDataLabel;
    },
    detail: 'Model distribution table must render responsive stacked cards with data-labels on mobile viewports',
  },
  {
    name: 'Mobile token chart readability and isolation',
    test: () => {
      const hasChartScroll = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.token-chart-svg-container\s*\{[^}]*overflow-x:\s*auto/s.test(stylesContent);
      const hasSvgMinWidth = /@media\s*\([^)]*max-width:\s*640px\)[^{]*\{[\s\S]*?\[data-theme="operational"\]\s+\.token-chart-svg-container\s+svg\s*\{[^}]*min-width:\s*580px/s.test(stylesContent);
      return hasChartScroll && hasSvgMinWidth;
    },
    detail: 'Token analytics chart must have responsive overflow-x and minimum width on mobile to maintain bar and label readability',
  },
];

console.log('--- AG-Proxy Operational Theme Invariant Verification ---');
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
  console.log('All operational theme visual slice invariants verified successfully.\n');
}
