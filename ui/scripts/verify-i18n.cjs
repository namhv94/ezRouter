/**
 * Automated Verification for ezRouter Multi-Language (i18n) Support
 *
 * Verifies:
 * 1. Recursive key parity between Vietnamese (vi.ts) and English (en.ts).
 *    Reports exact missing/extra keys with full hierarchical paths.
 * 2. Parameter interpolation parity (e.g. {{name}}, {{count}}, {{port}}).
 * 3. Absence of legacy visible brand strings ("Router Rust", "AG-Proxy")
 *    in UI source files and catalogues, allowing documented comments/fixtures.
 * 4. LanguageSwitcher component existence, touch targets (>=44px), and accessibility.
 * 5. Locale persistence key and fallback configuration invariants.
 */

const fs = require('fs');
const path = require('path');
const ts = require('typescript');
const vm = require('vm');

const UI_ROOT = path.resolve(__dirname, '..');
const SRC_DIR = path.resolve(UI_ROOT, 'src');
const VI_PATH = path.resolve(SRC_DIR, 'i18n/vi.ts');
const EN_PATH = path.resolve(SRC_DIR, 'i18n/en.ts');
const I18N_INDEX_PATH = path.resolve(SRC_DIR, 'i18n/index.ts');
const SWITCHER_PATH = path.resolve(SRC_DIR, 'components/LanguageSwitcher.tsx');
const NAVBAR_PATH = path.resolve(SRC_DIR, 'components/Navbar.tsx');
const STYLES_PATH = path.resolve(SRC_DIR, 'styles.css');
const HTML_PATH = path.resolve(UI_ROOT, 'index.html');

console.log('======================================================');
console.log('       ezRouter i18n & Brand Verification Check       ');
console.log('======================================================\n');

let totalChecks = 0;
let passedChecks = 0;
let failedChecks = 0;

function pass(name, detail = '') {
  totalChecks++;
  passedChecks++;
  console.log(`[PASS] ${name}${detail ? ` - ${detail}` : ''}`);
}

function fail(name, detail = '') {
  totalChecks++;
  failedChecks++;
  console.error(`[FAIL] ${name}${detail ? ` - ${detail}` : ''}`);
}

// Helper: Transpile and execute a TypeScript file in an isolated context
function loadTsModule(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`File not found: ${filePath}`);
  }
  const source = fs.readFileSync(filePath, 'utf8');
  const transpiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;

  const sandbox = {
    exports: {},
    module: { exports: {} },
    require: () => ({}),
    console,
  };
  vm.createContext(sandbox);
  vm.runInContext(transpiled, sandbox);
  return sandbox.exports;
}

// Helper: Flatten nested translation object into dot-notated leaf keys
function getLeafEntries(obj, prefix = '') {
  let entries = [];
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      entries = entries.concat(getLeafEntries(value, fullKey));
    } else {
      entries.push([fullKey, value]);
    }
  }
  return entries;
}

// Helper: Extract {{param}} placeholders from string
function extractPlaceholders(str) {
  if (typeof str !== 'string') return [];
  const matches = str.match(/\{\{([a-zA-Z0-9_]+)\}\}/g) || [];
  return matches.map((m) => m.replace(/[\{\}]/g, '')).sort();
}

// ---------------------------------------------------------------------------
// 1. Catalogue Loading & Recursive Key Parity
// ---------------------------------------------------------------------------
console.log('--- 1. Catalogue Key Parity Verification ---');
try {
  const viModule = loadTsModule(VI_PATH);
  const enModule = loadTsModule(EN_PATH);

  const viCatalogue = viModule.vi;
  const enCatalogue = enModule.en;

  if (!viCatalogue || typeof viCatalogue !== 'object') {
    fail('Load vi.ts', 'Failed to export `vi` translation object');
  } else {
    pass('Load vi.ts', 'Successfully parsed Vietnamese translation catalogue');
  }

  if (!enCatalogue || typeof enCatalogue !== 'object') {
    fail('Load en.ts', 'Failed to export `en` translation object');
  } else {
    pass('Load en.ts', 'Successfully parsed English translation catalogue');
  }

  if (viCatalogue && enCatalogue) {
    const viEntries = getLeafEntries(viCatalogue);
    const enEntries = getLeafEntries(enCatalogue);

    const viMap = new Map(viEntries);
    const enMap = new Map(enEntries);

    const viKeys = new Set(viMap.keys());
    const enKeys = new Set(enMap.keys());

    const missingInEn = [...viKeys].filter((k) => !enKeys.has(k)).sort();
    const missingInVi = [...enKeys].filter((k) => !viKeys.has(k)).sort();

    console.log(`\nCatalogue summary: ${viKeys.size} Vietnamese keys, ${enKeys.size} English keys`);

    if (missingInEn.length === 0 && missingInVi.length === 0) {
      pass(
        'Catalogue recursive key parity',
        `Exact 1:1 match across all ${viKeys.size} translation keys`
      );
    } else {
      if (missingInEn.length > 0) {
        console.error(`\nMissing keys in en.ts (${missingInEn.length}):`);
        missingInEn.forEach((k) => console.error(`  - ${k}`));
      }
      if (missingInVi.length > 0) {
        console.error(`\nExtra/missing keys in vi.ts (${missingInVi.length}):`);
        missingInVi.forEach((k) => console.error(`  + ${k}`));
      }
      fail(
        'Catalogue recursive key parity',
        `Parity mismatch: ${missingInEn.length} missing in EN, ${missingInVi.length} missing in VI`
      );
    }

    // Check interpolation parameter consistency
    const paramMismatches = [];
    const emptyValueKeys = [];

    for (const [key, viVal] of viMap.entries()) {
      const enVal = enMap.get(key);

      if (typeof viVal !== 'string' || viVal.trim() === '') {
        emptyValueKeys.push(`vi: ${key}`);
      }
      if (typeof enVal !== 'string' || enVal.trim() === '') {
        emptyValueKeys.push(`en: ${key}`);
      }

      if (typeof viVal === 'string' && typeof enVal === 'string') {
        const viParams = extractPlaceholders(viVal);
        const enParams = extractPlaceholders(enVal);
        if (JSON.stringify(viParams) !== JSON.stringify(enParams)) {
          paramMismatches.push({
            key,
            viParams,
            enParams,
            viVal,
            enVal,
          });
        }
      }
    }

    if (emptyValueKeys.length === 0) {
      pass('Non-empty translation values', 'All leaf values are non-empty strings');
    } else {
      fail(
        'Non-empty translation values',
        `Found ${emptyValueKeys.length} empty values: ${emptyValueKeys.slice(0, 5).join(', ')}`
      );
    }

    if (paramMismatches.length === 0) {
      pass(
        'Interpolation parameter consistency',
        'All parameter placeholders match across locales'
      );
    } else {
      console.error('\nParameter placeholder mismatches:');
      paramMismatches.forEach((m) => {
        console.error(`  Key "${m.key}": vi=[${m.viParams.join(', ')}] vs en=[${m.enParams.join(', ')}]`);
      });
      fail(
        'Interpolation parameter consistency',
        `Found ${paramMismatches.length} placeholder mismatches`
      );
    }

    // Check all t('key') calls in source files
    const sourceFiles = [];
    function scanSrc(dir) {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          if (entry.name !== 'node_modules' && entry.name !== 'dist' && entry.name !== 'i18n') {
            scanSrc(full);
          }
        } else if (entry.name.endsWith('.tsx') || entry.name.endsWith('.ts')) {
          sourceFiles.push(full);
        }
      }
    }
    scanSrc(SRC_DIR);
    const missingInCode = [];
    for (const file of sourceFiles) {
      const code = fs.readFileSync(file, 'utf8');
      const regex = /\bt\(\s*['"`]([a-zA-Z0-9_.-]+)['"`]/g;
      let match;
      while ((match = regex.exec(code)) !== null) {
        const key = match[1];
        if (!viMap.has(key)) {
          missingInCode.push({ file: path.relative(UI_ROOT, file), key });
        }
      }
    }
    if (missingInCode.length === 0) {
      pass('Source code translation usage', 'All t(...) calls match defined catalogue keys');
    } else {
      console.error('\nMissing translation keys in source code:');
      missingInCode.forEach((m) => {
        console.error(`  ${m.file}: t('${m.key}')`);
      });
      fail(
        'Source code translation usage',
        `Found ${missingInCode.length} missing translation keys used in source code`
      );
    }
  }
} catch (err) {
  fail('Catalogue parsing', err.message);
}

// ---------------------------------------------------------------------------
// 2. Legacy Visible Brand Strings Check
// ---------------------------------------------------------------------------
console.log('\n--- 2. Legacy Visible Brand Strings Check ---');

/**
 * Scan source files for legacy brand strings:
 * - "Router Rust" (case-insensitive in visible UI copy)
 * - "AG-Proxy" (case-insensitive in visible UI copy)
 *
 * Allowed exemptions:
 * - Code comments explicitly referencing historical backend contracts or architecture
 * - Internal mock/test fixtures: e.g. 'ag-proxy-key' (default API key fixture)
 * - Internal package names / repo names: e.g. 'ag-proxy-rust'
 */

function collectSourceFiles(dir) {
  let files = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== 'node_modules' && entry.name !== 'dist' && !entry.name.startsWith('.')) {
        files = files.concat(collectSourceFiles(fullPath));
      }
    } else if (/\.(tsx?|html)$/.test(entry.name)) {
      files.push(fullPath);
    }
  }
  return files;
}

const filesToScan = collectSourceFiles(SRC_DIR).concat([HTML_PATH]);
const legacyBrandViolations = [];

for (const filePath of filesToScan) {
  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');

  lines.forEach((line, idx) => {
    const lineNum = idx + 1;
    const trimmed = line.trim();

    // Check if line is purely a comment
    const isComment = /^\s*(\/\/|\/\*|\*)/.test(line);

    // 1. Check for "Router Rust"
    if (/Router\s+Rust/i.test(line)) {
      if (!isComment) {
        legacyBrandViolations.push({
          file: path.relative(UI_ROOT, filePath),
          line: lineNum,
          match: 'Router Rust',
          text: trimmed,
        });
      }
    }

    // 2. Check for "AG-Proxy" (or "AG Proxy") as visible branding
    if (/ag[\s-]proxy\b/i.test(line)) {
      // Exclude allowed identifiers/fixtures:
      // - ag-proxy-key (admin key fixture / placeholder)
      // - ag-proxy-rust (repository/package name)
      // - comments
      const isAllowedFixture = /ag-proxy-key|ag-proxy-rust/i.test(line);
      if (!isComment && !isAllowedFixture) {
        legacyBrandViolations.push({
          file: path.relative(UI_ROOT, filePath),
          line: lineNum,
          match: 'AG-Proxy',
          text: trimmed,
        });
      }
    }
  });
}

if (legacyBrandViolations.length === 0) {
  pass(
    'Visible brand naming consistency',
    'No unexempted "Router Rust" or "AG-Proxy" visible brand strings found (all use ezRouter)'
  );
} else {
  console.error(`\nFound ${legacyBrandViolations.length} legacy brand string violation(s):`);
  legacyBrandViolations.forEach((v) => {
    console.error(`  ${v.file}:${v.line} [${v.match}] -> ${v.text}`);
  });
  fail(
    'Visible brand naming consistency',
    `${legacyBrandViolations.length} legacy brand strings detected in UI source`
  );
}

// ---------------------------------------------------------------------------
// 3. Language Switcher & Accessibility Invariants
// ---------------------------------------------------------------------------
console.log('\n--- 3. Language Switcher & Accessibility Invariants ---');

try {
  // Check LanguageSwitcher component file
  if (fs.existsSync(SWITCHER_PATH)) {
    const switcherSource = fs.readFileSync(SWITCHER_PATH, 'utf8');
    const hasAriaGroup = /role="group"/.test(switcherSource);
    const hasAriaPressed = /aria-pressed=/.test(switcherSource);
    const hasLangAttrs = /lang="vi"/.test(switcherSource) && /lang="en"/.test(switcherSource);
    const hasKeyboardNav = /handleKeyDown/.test(switcherSource) || /onKeyDown/.test(switcherSource);

    if (hasAriaGroup && hasAriaPressed && hasLangAttrs && hasKeyboardNav) {
      pass(
        'LanguageSwitcher accessibility',
        'Component has role="group", aria-pressed, lang tags, and keyboard navigation'
      );
    } else {
      fail(
        'LanguageSwitcher accessibility',
        `Missing accessibility attributes in LanguageSwitcher.tsx (group: ${hasAriaGroup}, pressed: ${hasAriaPressed}, lang: ${hasLangAttrs}, keyboard: ${hasKeyboardNav})`
      );
    }
  } else {
    fail('LanguageSwitcher existence', 'LanguageSwitcher.tsx does not exist');
  }

  // Check Navbar integration
  if (fs.existsSync(NAVBAR_PATH)) {
    const navbarSource = fs.readFileSync(NAVBAR_PATH, 'utf8');
    if (navbarSource.includes('LanguageSwitcher')) {
      pass('Navbar integration', 'Navbar renders LanguageSwitcher control');
    } else {
      fail('Navbar integration', 'Navbar does not render LanguageSwitcher');
    }
  } else {
    fail('Navbar file check', 'Navbar.tsx not found');
  }

  // Check CSS touch targets (>=44px)
  if (fs.existsSync(STYLES_PATH)) {
    const stylesSource = fs.readFileSync(STYLES_PATH, 'utf8');
    const hasSwitcherStyles = /\.lang-switcher\s*\{/.test(stylesSource);
    const has44pxMinHeight = /\.lang-switcher-btn\s*\{[^}]*min-height:\s*44px/s.test(stylesSource);
    const has44pxMinWidth = /\.lang-switcher-btn\s*\{[^}]*min-width:\s*44px/s.test(stylesSource);
    const hasFocusVisible = /\.lang-switcher-btn:focus-visible/.test(stylesSource);

    if (hasSwitcherStyles && has44pxMinHeight && has44pxMinWidth && hasFocusVisible) {
      pass(
        'Language switcher CSS invariants',
        'Touch targets >= 44px (height & width) with focus-visible styling defined'
      );
    } else {
      fail(
        'Language switcher CSS invariants',
        `CSS touch target or focus state missing (styles: ${hasSwitcherStyles}, min-height 44px: ${has44pxMinHeight}, min-width 44px: ${has44pxMinWidth}, focus-visible: ${hasFocusVisible})`
      );
    }
  } else {
    fail('CSS styles check', 'styles.css not found');
  }
} catch (err) {
  fail('LanguageSwitcher check error', err.message);
}

// ---------------------------------------------------------------------------
// 4. Persistence & Default Locale Invariants
// ---------------------------------------------------------------------------
console.log('\n--- 4. Locale Provider & HTML Invariants ---');

try {
  // Check index.html defaults
  if (fs.existsSync(HTML_PATH)) {
    const htmlSource = fs.readFileSync(HTML_PATH, 'utf8');
    const hasLangVi = /<html\s+lang="vi">/i.test(htmlSource);
    const hasEzRouterTitle = /<title>[^<]*ezRouter[^<]*<\/title>/i.test(htmlSource);

    if (hasLangVi && hasEzRouterTitle) {
      pass(
        'index.html default markup',
        'Contains <html lang="vi"> default and ezRouter title'
      );
    } else {
      fail(
        'index.html default markup',
        `index.html missing lang="vi" (${hasLangVi}) or ezRouter title (${hasEzRouterTitle})`
      );
    }
  } else {
    fail('index.html existence', 'index.html not found');
  }

  // Check i18n/index.ts persistence key and export contracts
  if (fs.existsSync(I18N_INDEX_PATH)) {
    const i18nIndexSource = fs.readFileSync(I18N_INDEX_PATH, 'utf8');
    const hasStorageKey = i18nIndexSource.includes('ezrouter.locale');
    const hasDefaultVi = /DEFAULT_LOCALE\s*:\s*Locale\s*=\s*['"]vi['"]/.test(i18nIndexSource) ||
                         /defaultLocale\s*:\s*Locale\s*=\s*['"]vi['"]/.test(i18nIndexSource) ||
                         /fallback.*['"]vi['"]/i.test(i18nIndexSource);
    const hasProviderExport = /export\s+(?:const|function)\s+I18nProvider/.test(i18nIndexSource);
    const hasHookExport = /export\s+(?:const|function)\s+useI18n/.test(i18nIndexSource);

    if (hasStorageKey && hasProviderExport && hasHookExport && hasDefaultVi) {
      pass(
        'i18n index contracts',
        'Exports I18nProvider, useI18n, defaults to "vi", and uses "ezrouter.locale" storage key'
      );
    } else {
      fail(
        'i18n index contracts',
        `i18n/index.ts contracts check failed (storageKey: ${hasStorageKey}, provider: ${hasProviderExport}, hook: ${hasHookExport}, defaultVi: ${hasDefaultVi})`
      );
    }
  } else {
    fail('i18n index existence', 'ui/src/i18n/index.ts not found');
  }
} catch (err) {
  fail('Provider check error', err.message);
}

// ---------------------------------------------------------------------------
// Summary & Exit Code
// ---------------------------------------------------------------------------
console.log('\n======================================================');
console.log(`Verification completed: ${passedChecks}/${totalChecks} checks passed.`);
if (failedChecks > 0) {
  console.error(`FAILED: ${failedChecks} check(s) failed.`);
  console.log('======================================================\n');
  process.exit(1);
} else {
  console.log('SUCCESS: All i18n parity, brand, and layout checks passed.');
  console.log('======================================================\n');
  process.exit(0);
}
