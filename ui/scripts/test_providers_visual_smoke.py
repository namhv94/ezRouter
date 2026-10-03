import sys
import os
from playwright.sync_api import sync_playwright

def run_provider_checks(url: str, admin_key: str):
    print("\n========================================================")
    print("Running Visual Providers Smoke & Mobile 390px Tests")
    print(f"Target URL: {url}")
    print("========================================================")

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)

        # ----------------------------------------------------
        # TEST 1: Mobile 390x844 Viewport
        # ----------------------------------------------------
        print("\n--- 1. Testing Mobile 390x844 Viewport ---")
        m_context = browser.new_context(
            viewport={"width": 390, "height": 844},
            user_agent="Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15"
        )
        page = m_context.new_page()

        # Login
        page.goto(url, wait_until="networkidle")
        page.fill('input[type="password"]', admin_key)
        page.click('button[type="submit"]')
        page.wait_for_selector('.top-navbar', timeout=5000)

        # Navigate to Providers Tab
        page.click('.menu-toggle-btn')
        page.wait_for_timeout(200)
        page.click('.nav-item:has-text("Nhà Cung Cấp")')
        page.wait_for_timeout(500)

        # Verify Compact Header
        header_title = page.inner_text('.providers-header-title')
        print(f"✓ Compact Header Title: {header_title.strip()}")
        assert "Nhà Cung Cấp" in header_title, "Header title mismatch"

        # Verify Status Strip & Pulse Dot
        legend = page.query_selector('.status-legend-bar')
        assert legend is not None, "Status strip bar not found"
        pulse_dot = page.query_selector('.route-pulse-dot')
        assert pulse_dot is not None, "Route pulse dot not found"

        # Verify Status Strip Quick Chips
        chips = page.query_selector_all('.status-segment-chip')
        assert len(chips) == 3, f"Expected 3 status chips (Google, Codex, Upstream), got {len(chips)}"
        print("✓ One-line status strip, route pulse dot, and 3 provider status chips verified")

        # Verify Segmented Tab Control (No monolithic scroll)
        seg_tabs = page.query_selector_all('.segment-tab-btn')
        assert len(seg_tabs) == 3, f"Expected 3 segment tabs, got {len(seg_tabs)}"
        active_tab = page.query_selector('.segment-tab-btn.active')
        assert active_tab is not None and "Google" in active_tab.inner_text(), "Default segment should be Google"
        print("✓ Segmented Google/Codex/Upstream tabs verified (default: Google Antigravity)")

        # Verify Single Focused Visual Card in Google segment
        cards = page.query_selector_all('.visual-card')
        assert len(cards) == 1, f"Expected 1 active visual card in segmented view, got {len(cards)}"
        rings = page.query_selector_all('.health-ring-svg')
        assert len(rings) == 1, f"Expected 1 SVG health ring, got {len(rings)}"
        stat_numbers = page.query_selector_all('.stat-num-val')
        assert len(stat_numbers) >= 3, f"Expected at least 3 stat numbers, got {len(stat_numbers)}"
        print("✓ Focused Google visual card with SVG health ring and stat numbers verified")

        # Verify Consolidated Action Bar
        action_bar = page.query_selector('.segment-action-bar')
        assert action_bar is not None, "Consolidated segment action bar not found"

        # Verify Touch Targets (>=44px) on action buttons and details toggle
        action_btns = page.query_selector_all('.action-icon-btn, .details-toggle-btn, .segment-tab-btn')
        for i, btn in enumerate(action_btns):
            box = btn.bounding_box()
            if box:
                assert box['height'] >= 43.5, f"Touch target height < 44px on button {i}: {box['height']}px"
        print(f"✓ Verified {len(action_btns)} action/tab buttons have touch targets >= 44px")

        # Check for Horizontal Overflow at 390px
        doc_scroll = page.evaluate("() => ({ docW: document.documentElement.scrollWidth, bodyW: document.body.scrollWidth, innerW: window.innerWidth })")
        print(f"✓ Mobile dimensions: {doc_scroll}")
        assert doc_scroll['docW'] <= 391, f"Overflow detected: {doc_scroll['docW']} > 390"

        # Test Accordion Expand / Collapse
        print("✓ Testing Google details accordion toggle...")
        page.click('button:has-text("Chi tiết tài khoản")')
        page.wait_for_timeout(300)
        assert page.is_visible('.details-drawer-content'), "Details drawer did not expand"
        page.click('button:has-text("Thu gọn danh sách tài khoản")')
        page.wait_for_timeout(300)

        # Test Switching to Codex Segment
        print("✓ Testing switch to Codex segment tab...")
        page.click('button[data-segment="codex"]')
        page.wait_for_timeout(300)
        codex_card = page.query_selector('.visual-card:has-text("OpenAI Codex")')
        assert codex_card is not None, "Codex visual card not rendered upon tab switch"

        # Test Switching to Upstream Segment
        print("✓ Testing switch to Upstream segment tab...")
        page.click('button[data-segment="upstream"]')
        page.wait_for_timeout(300)
        upstream_card = page.query_selector('.visual-card:has-text("Upstream Tùy Biến")')
        assert upstream_card is not None, "Upstream visual card not rendered upon tab switch"

        # Switch back to Google for screenshot
        page.click('button[data-segment="google"]')
        page.wait_for_timeout(200)

        # Capture mobile screenshot
        os.makedirs("docs/screenshots", exist_ok=True)
        mobile_ss_path = "docs/screenshots/providers_mobile_390.png"
        page.screenshot(path=mobile_ss_path)
        print(f"✓ Saved mobile screenshot to {mobile_ss_path}")

        m_context.close()

        # ----------------------------------------------------
        # TEST 2: Desktop 1280x800 Viewport
        # ----------------------------------------------------
        print("\n--- 2. Testing Desktop 1280x800 Viewport ---")
        d_context = browser.new_context(viewport={"width": 1280, "height": 800})
        page = d_context.new_page()

        page.goto(url, wait_until="networkidle")
        page.fill('input[type="password"]', admin_key)
        page.click('button[type="submit"]')
        page.wait_for_selector('.top-navbar', timeout=5000)

        # Navigate to Providers
        page.click('.nav-item:has-text("Nhà Cung Cấp")')
        page.wait_for_timeout(500)

        # Verify Segmented Tabs on Desktop
        desktop_tabs = page.query_selector_all('.segment-tab-btn')
        assert len(desktop_tabs) == 3, f"Desktop expected 3 segment tabs, got {len(desktop_tabs)}"

        # Test Modal Open/Close: Google Add Token
        print("✓ Testing Google Add Token Modal...")
        page.click('button[title="Thêm tài khoản thủ công qua Refresh Token"]')
        page.wait_for_selector('.modal-card')
        assert page.is_visible('.modal-card:has-text("Thêm Tài Khoản Google Antigravity")')
        page.click('.modal-header .btn-icon-only')
        page.wait_for_timeout(200)

        # Test Modal Open/Close: Google OAuth
        print("✓ Testing Google OAuth Modal...")
        page.click('button[title="Đăng nhập Google qua OAuth 1-Click"]')
        page.wait_for_selector('.modal-card')
        assert page.is_visible('.modal-card:has-text("Đăng Nhập Google OAuth")')
        page.click('.modal-header .btn-icon-only')
        page.wait_for_timeout(200)

        # Switch to Codex tab & test Codex Add Auth Modal
        print("✓ Switching to Codex segment tab on desktop...")
        page.click('button[data-segment="codex"]')
        page.wait_for_timeout(300)
        print("✓ Testing Codex Add Auth Modal...")
        page.click('button[title="Thêm tệp auth.json"]')
        page.wait_for_selector('.modal-card')
        assert page.is_visible('.modal-card:has-text("Thêm Tài Khoản OpenAI Codex")')
        page.click('.modal-header .btn-icon-only')
        page.wait_for_timeout(200)

        # Switch to Upstream tab & test Add Upstream Modal
        print("✓ Switching to Upstream segment tab on desktop...")
        page.click('button[data-segment="upstream"]')
        page.wait_for_timeout(300)
        print("✓ Testing Add Upstream Modal...")
        page.click('button:has-text("Thêm Upstream")')
        page.wait_for_selector('.modal-card')
        assert page.is_visible('.modal-card:has-text("Thêm Nhà Cung Cấp Upstream")')
        page.click('.modal-header .btn-icon-only')
        page.wait_for_timeout(200)

        # Switch back to Google segment for desktop screenshot
        page.click('button[data-segment="google"]')
        page.wait_for_timeout(300)

        # Capture desktop screenshot
        desktop_ss_path = "docs/screenshots/providers_desktop_1280.png"
        page.screenshot(path=desktop_ss_path)
        print(f"✓ Saved desktop screenshot to {desktop_ss_path}")

        d_context.close()
        browser.close()

        print("\n========================================================")
        print("ALL PROVIDERS VISUAL TESTS & SMOKE SUCCEEDED!")
        print("========================================================")

if __name__ == "__main__":
    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:20229"
    key = sys.argv[2] if len(sys.argv) > 2 else "ag-proxy-key"
    run_provider_checks(url, key)
