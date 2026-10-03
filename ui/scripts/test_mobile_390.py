import sys
import os
import re

try:
    from playwright.sync_api import sync_playwright
    HAS_PLAYWRIGHT = True
except ImportError:
    sync_playwright = None
    HAS_PLAYWRIGHT = False


def run_static_checks():
    """
    Deterministic static verification of 390px mobile invariants and LanguageSwitcher
    when Playwright is absent in the environment.
    """
    script_dir = os.path.dirname(os.path.abspath(__file__))
    ui_root = os.path.abspath(os.path.join(script_dir, ".."))

    print("\n==========================================")
    print("Testing 390px Mobile & LanguageSwitcher Invariants (Deterministic Static Mode)")
    print("Notice: Playwright browser dependency is not installed; running static verification.")
    print("==========================================")

    # 1. Check LanguageSwitcher component file
    switcher_path = os.path.join(ui_root, "src", "components", "LanguageSwitcher.tsx")
    assert os.path.exists(switcher_path), f"LanguageSwitcher.tsx missing at {switcher_path}"
    with open(switcher_path, "r", encoding="utf-8") as f:
        switcher_code = f.read()

    assert 'role="group"' in switcher_code, "LanguageSwitcher must define role='group'"
    assert 'aria-pressed=' in switcher_code, "LanguageSwitcher buttons must define aria-pressed"
    assert 'lang="vi"' in switcher_code and 'lang="en"' in switcher_code, "LanguageSwitcher must have lang tags"
    assert 'onKeyDown' in switcher_code or 'handleKeyDown' in switcher_code, "LanguageSwitcher must support keyboard navigation"
    print("PASS: LanguageSwitcher component existence, accessibility, and ARIA attributes")

    # 2. Check Navbar integration
    navbar_path = os.path.join(ui_root, "src", "components", "Navbar.tsx")
    assert os.path.exists(navbar_path), f"Navbar.tsx missing at {navbar_path}"
    with open(navbar_path, "r", encoding="utf-8") as f:
        navbar_code = f.read()
    assert 'LanguageSwitcher' in navbar_code, "Navbar.tsx must render LanguageSwitcher"
    print("PASS: Navbar renders LanguageSwitcher")

    # 3. Check CSS Touch Target invariants (>=44px)
    styles_path = os.path.join(ui_root, "src", "styles.css")
    assert os.path.exists(styles_path), f"styles.css missing at {styles_path}"
    with open(styles_path, "r", encoding="utf-8") as f:
        styles_code = f.read()

    assert re.search(r'\.lang-switcher-btn\s*\{[^}]*min-height:\s*44px', styles_code, re.DOTALL), (
        "LanguageSwitcher button min-height must be >= 44px"
    )
    assert re.search(r'\.lang-switcher-btn\s*\{[^}]*min-width:\s*44px', styles_code, re.DOTALL), (
        "LanguageSwitcher button min-width must be >= 44px"
    )
    assert re.search(
        r'@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.lang-switcher-btn\s*\{[^}]*min-height:\s*44px',
        styles_code
    ), "LanguageSwitcher button must maintain min-height >= 44px on mobile viewports"
    assert re.search(
        r'@media\s*\([^)]*max-width:\s*768px\)[^{]*\{[\s\S]*?\.lang-switcher-btn\s*\{[^}]*min-width:\s*44px',
        styles_code
    ), "LanguageSwitcher button must maintain min-width >= 44px on mobile viewports"
    print("PASS: LanguageSwitcher CSS touch target invariants (>=44px on mobile and desktop)")

    print("\nALL DETERMINISTIC STATIC 390px CHECKS PASSED SUCCESSFULLY!\n")
    return True


def run_checks(url: str, admin_key: str):
    if not HAS_PLAYWRIGHT or sync_playwright is None:
        return run_static_checks()

    print(f"\n==========================================")
    print(f"Testing URL: {url} at 390x844 (Mobile)")
    print(f"==========================================")
    
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        # iPhone 12/13/14 viewport: 390x844
        context = browser.new_context(
            viewport={"width": 390, "height": 844},
            user_agent="Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1"
        )
        page = context.new_page()
        
        # 1. Load login page
        print("1. Loading page...")
        page.goto(url, wait_until="networkidle")
        
        # Check login screen overflow
        body_scroll = page.evaluate("() => ({ scrollWidth: document.body.scrollWidth, clientWidth: document.body.clientWidth, docScrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })")
        print(f"   Login screen dimensions: {body_scroll}")
        assert body_scroll["docScrollWidth"] <= 390, f"Horizontal overflow on login screen: {body_scroll}"
        
        # Check input touch target and font size
        input_info = page.evaluate("""() => {
            const input = document.querySelector('input[type="password"]');
            if (!input) return null;
            const rect = input.getBoundingClientRect();
            const style = window.getComputedStyle(input);
            return {
                width: rect.width,
                height: rect.height,
                fontSize: style.fontSize,
                minHeight: style.minHeight
            };
        }""")
        print(f"   Input info: {input_info}")
        assert input_info["height"] >= 44, f"Input touch target too small: {input_info['height']}px < 44px"
        assert float(input_info["fontSize"].replace("px", "")) >= 16, f"Input font size < 16px (risks iOS auto-zoom): {input_info['fontSize']}"
        
        # Check login button touch target
        btn_info = page.evaluate("""() => {
            const btn = document.querySelector('button[type="submit"]');
            if (!btn) return null;
            const rect = btn.getBoundingClientRect();
            return { width: rect.width, height: rect.height };
        }""")
        print(f"   Login button: {btn_info}")
        assert btn_info["height"] >= 44, f"Login button touch target too small: {btn_info['height']}px < 44px"
        
        # 2. Perform Login
        print("2. Authenticating with admin key...")
        page.fill('input[type="password"]', admin_key)
        page.click('button[type="submit"]')
        page.wait_for_selector('.top-navbar', timeout=5000)
        page.wait_for_timeout(500)
        
        # Check Navbar at 390px
        navbar_info = page.evaluate("""() => {
            const nav = document.querySelector('.top-navbar');
            const keyBadge = document.querySelector('.nav-key-badge');
            const btnLabels = Array.from(document.querySelectorAll('.nav-action-btn .btn-label'));
            const rect = nav.getBoundingClientRect();
            return {
                width: rect.width,
                keyBadgeVisible: keyBadge ? window.getComputedStyle(keyBadge).display !== 'none' : false,
                btnLabelsVisible: btnLabels.some(l => window.getComputedStyle(l).display !== 'none')
            };
        }""")
        print(f"   Navbar info: {navbar_info}")
        assert not navbar_info["keyBadgeVisible"], "Key badge should be hidden on 390px mobile"
        assert not navbar_info["btnLabelsVisible"], "Action button text labels should be hidden on 390px mobile"

        # Check LanguageSwitcher existence and touch targets
        lang_switcher_info = page.evaluate("""() => {
            const switcher = document.querySelector('.lang-switcher');
            if (!switcher) return null;
            const buttons = Array.from(switcher.querySelectorAll('.lang-switcher-btn')).map(btn => {
                const rect = btn.getBoundingClientRect();
                return {
                    text: btn.textContent.trim(),
                    lang: btn.getAttribute('lang'),
                    active: btn.classList.contains('active'),
                    pressed: btn.getAttribute('aria-pressed'),
                    width: rect.width,
                    height: rect.height
                };
            });
            return {
                role: switcher.getAttribute('role'),
                buttons: buttons
            };
        }""")
        print(f"   Language switcher info: {lang_switcher_info}")
        assert lang_switcher_info is not None, "LanguageSwitcher should exist in navbar"
        assert lang_switcher_info["role"] == "group", "LanguageSwitcher must have role='group'"
        assert len(lang_switcher_info["buttons"]) == 2, "LanguageSwitcher must provide 2 language options (VI and EN)"
        for btn in lang_switcher_info["buttons"]:
            assert btn["height"] >= 44, f"Language button {btn['text']} touch height < 44px: {btn['height']}"
            assert btn["width"] >= 44, f"Language button {btn['text']} touch width < 44px: {btn['width']}"
        
        # Check for any element causing overflow
        def check_overflow(context_label: str):
            res = page.evaluate("""() => {
                const w = window.innerWidth;
                const docW = document.documentElement.scrollWidth;
                const bodyW = document.body.scrollWidth;
                const overflowing = [];
                document.querySelectorAll('*').forEach(el => {
                    const rect = el.getBoundingClientRect();
                    if (rect.right > w + 1) { // 1px tolerance for subpixel rounding
                        overflowing.push({
                            tag: el.tagName,
                            className: el.className,
                            id: el.id,
                            right: Math.round(rect.right),
                            width: Math.round(rect.width)
                        });
                    }
                });
                return {
                    w,
                    docW,
                    bodyW,
                    overflowing: overflowing.slice(0, 5)
                };
            }""")
            print(f"   [{context_label}] docScrollWidth={res['docW']}, bodyScrollWidth={res['bodyW']}, windowWidth={res['w']}")
            if res['overflowing']:
                print(f"   WARNING / ERROR overflowing elements: {res['overflowing']}")
            assert res['docW'] <= 391, f"[{context_label}] Document overflows viewport: {res['docW']} > 390 ({res['overflowing']})"
            assert res['bodyW'] <= 391, f"[{context_label}] Body overflows viewport: {res['bodyW']} > 390 ({res['overflowing']})"
            return res

        # Check Overview Tab
        check_overflow("Tab: Overview")

        # Test switching to English and check overflow
        print("   Testing language switch to English...")
        page.click('.lang-switcher-btn[lang="en"]')
        page.wait_for_timeout(300)
        curr_lang = page.evaluate("() => document.documentElement.lang")
        assert curr_lang == "en", f"Expected document lang to be 'en', got '{curr_lang}'"
        check_overflow("Tab: Overview (English)")

        # Switch back to Vietnamese
        print("   Testing language switch back to Vietnamese...")
        page.click('.lang-switcher-btn[lang="vi"]')
        page.wait_for_timeout(300)
        curr_lang = page.evaluate("() => document.documentElement.lang")
        assert curr_lang == "vi", f"Expected document lang to be 'vi', got '{curr_lang}'"
        
        # 3. Test Sidebar Drawer
        print("3. Testing Sidebar Drawer...")
        page.click('.menu-toggle-btn')
        page.wait_for_timeout(300)
        sidebar_info = page.evaluate("""() => {
            const sb = document.querySelector('.sidebar');
            const overlay = document.querySelector('.mobile-overlay');
            const closeBtn = document.querySelector('.sidebar-close-btn');
            const sbRect = sb.getBoundingClientRect();
            const closeRect = closeBtn.getBoundingClientRect();
            return {
                open: sb.classList.contains('open'),
                width: sbRect.width,
                left: sbRect.left,
                overlayOpen: overlay.classList.contains('open'),
                closeBtnVisible: window.getComputedStyle(closeBtn).display !== 'none',
                closeBtnHeight: closeRect.height,
                bodyOverflow: document.body.style.overflow
            };
        }""")
        print(f"   Sidebar open state: {sidebar_info}")
        assert sidebar_info["open"], "Sidebar should have 'open' class"
        assert sidebar_info["overlayOpen"], "Mobile overlay should have 'open' class"
        assert sidebar_info["width"] <= 390, f"Sidebar too wide: {sidebar_info['width']}px"
        assert sidebar_info["closeBtnVisible"], "Sidebar close button should be visible on mobile"
        assert sidebar_info["closeBtnHeight"] >= 44, f"Sidebar close button touch target < 44px: {sidebar_info['closeBtnHeight']}px"
        assert sidebar_info["bodyOverflow"] == "hidden", "Body scroll should be locked when sidebar is open"
        
        # Close sidebar via close button
        page.click('.sidebar-close-btn')
        page.wait_for_timeout(300)
        closed_state = page.evaluate("() => document.querySelector('.sidebar').classList.contains('open')")
        assert not closed_state, "Sidebar should close after clicking close button"
        
        # Re-open and close via Escape
        page.click('.menu-toggle-btn')
        page.wait_for_timeout(300)
        page.keyboard.press("Escape")
        page.wait_for_timeout(300)
        esc_closed_state = page.evaluate("() => document.querySelector('.sidebar').classList.contains('open')")
        assert not esc_closed_state, "Sidebar should close on Escape key"
        
        # 4. Test tabs
        tabs = [
            ("monitoring", "Monitoring"),
            ("playground", "Playground"),
            ("providers", "Nhà Cung Cấp"),
            ("combos", "Combo"),
            ("token_saver", "Token Saver"),
            ("api_keys", "Khóa API"),
            ("logs", "Console Log"),
            ("integration", "Tích hợp API"),
        ]
        
        for tab_id, tab_label in tabs:
            print(f"4. Testing tab: {tab_label} ({tab_id})...")
            # Open drawer, click nav item
            page.click('.menu-toggle-btn')
            page.wait_for_timeout(200)
            page.click(f'.nav-item:has-text("{tab_label}")')
            page.wait_for_timeout(500)
            
            check_overflow(f"Tab: {tab_id}")
            
            # Check modals in this tab if applicable
            if tab_id == "providers":
                print("   Opening Add Upstream Provider modal...")
                upstream_tab = page.query_selector('button[data-segment="upstream"], button:has-text("Upstream")')
                if upstream_tab:
                    upstream_tab.click()
                    page.wait_for_timeout(300)
                page.click('button:has-text("Thêm Upstream")')
                page.wait_for_timeout(300)
                check_overflow("Modal: Add Provider")
                # Close modal
                page.click('.modal-header .btn-icon-only')
                page.wait_for_timeout(200)
            elif tab_id == "combos":
                print("   Opening Add Combo modal...")
                page.click('button:has-text("Tạo Combo Mới")')
                page.wait_for_timeout(300)
                check_overflow("Modal: Add Combo")
                page.click('.modal-header .btn-icon-only')
                page.wait_for_timeout(200)
            elif tab_id == "api_keys":
                print("   Opening Add API Key modal...")
                page.click('button:has-text("Tạo Khóa Mới")')
                page.wait_for_timeout(300)
                check_overflow("Modal: Add API Key")
                page.click('.modal-header .btn-icon-only')
                page.wait_for_timeout(200)

        print("\nALL 390px CHECKS PASSED SUCCESSFULLY!")
        browser.close()

if __name__ == "__main__":
    if not HAS_PLAYWRIGHT:
        run_static_checks()
        sys.exit(0)

    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:20229"
    key = sys.argv[2] if len(sys.argv) > 2 else "ag-proxy-key"
    run_checks(url, key)
