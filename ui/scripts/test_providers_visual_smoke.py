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
        page.click('.nav-item:has-text("Nhà cung cấp")')
        page.wait_for_timeout(500)

        # Verify Compact Header
        header_title = page.inner_text('.providers-header-title')
        print(f"✓ Compact Header Title: {header_title.strip()}")
        assert "Nhà Cung Cấp" in header_title.upper() or "Nhà cung cấp" in header_title, "Header title mismatch"

        # Verify Status Strip & Pulse Dot
        legend = page.query_selector('.status-legend-bar')
        assert legend is not None, "Status strip bar not found"
        pulse_dot = page.query_selector('.route-pulse-dot')
        assert pulse_dot is not None, "Route pulse dot not found"

        # Verify Status Strip Quick Chips (OpenRouter + Upstream only)
        chips = page.query_selector_all('.status-segment-chip')
        assert len(chips) == 2, f"Expected 2 status chips (OpenRouter, Upstream), got {len(chips)}"
        chips_text = " ".join(c.inner_text() for c in chips)
        assert "OpenRouter" in chips_text and "Upstream" in chips_text, f"Unexpected chip labels: {chips_text}"
        print("✓ One-line status strip, route pulse dot, and 2 provider status chips verified")

        # Verify both provider panels render (legacy mode shows all panels)
        cards = page.query_selector_all('.visual-card')
        assert len(cards) >= 2, f"Expected OpenRouter + Upstream visual cards, got {len(cards)}"
        assert page.query_selector('#provider-openrouter') is not None, "OpenRouter panel missing"
        assert page.query_selector('#provider-upstream') is not None, "Upstream panel missing"
        print(f"✓ {len(cards)} provider visual cards rendered (OpenRouter + Upstream)")

        rings = page.query_selector_all('.health-ring-svg')
        assert len(rings) >= 1, f"Expected at least 1 SVG health ring, got {len(rings)}"
        stat_numbers = page.query_selector_all('.stat-num-val')
        assert len(stat_numbers) >= 4, f"Expected at least 4 stat numbers, got {len(stat_numbers)}"
        print("✓ SVG health ring and stat numbers verified")

        # Verify Consolidated Action Bars
        action_bars = page.query_selector_all('.segment-action-bar')
        assert len(action_bars) >= 2, f"Expected an action bar per provider panel, got {len(action_bars)}"

        # Verify Touch Targets (>=44px) on action buttons and details toggles
        action_btns = page.query_selector_all('.action-icon-btn, .details-toggle-btn')
        checked = 0
        for i, btn in enumerate(action_btns):
            box = btn.bounding_box()
            if box:
                assert box['height'] >= 43.5, f"Touch target height < 44px on button {i}: {box['height']}px"
                checked += 1
        print(f"✓ Verified {checked} action/toggle buttons have touch targets >= 44px")

        # Check for Horizontal Overflow at 390px
        doc_scroll = page.evaluate("() => ({ docW: document.documentElement.scrollWidth, bodyW: document.body.scrollWidth, innerW: window.innerWidth })")
        print(f"✓ Mobile dimensions: {doc_scroll}")
        assert doc_scroll['docW'] <= 391, f"Overflow detected: {doc_scroll['docW']} > 390"

        # Test Accordion Expand / Collapse on the Upstream drawer
        print("✓ Testing Upstream details accordion toggle...")
        upstream_toggle = page.locator('#provider-upstream .details-toggle-btn').first
        assert upstream_toggle.count() > 0, "Upstream details toggle not found"
        upstream_toggle.click()
        page.wait_for_timeout(300)
        assert page.is_visible('.details-drawer-content'), "Details drawer did not expand"

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
        page.click('.nav-item:has-text("Nhà cung cấp")')
        page.wait_for_timeout(500)

        # Legacy mode: both provider panels must be present
        assert page.query_selector('#provider-openrouter') is not None, "OpenRouter panel missing on desktop"
        assert page.query_selector('#provider-upstream') is not None, "Upstream panel missing on desktop"
        print("✓ OpenRouter and Upstream panels verified on desktop")

        # Status strip interplay: clicking a chip scrolls to the matching panel
        print("✓ Testing status chip navigation to Upstream...")
        page.click('.status-segment-chip:has-text("Upstream")')
        page.wait_for_timeout(600)

        # Test Add Upstream Modal
        print("✓ Testing Add Upstream Modal...")
        page.click('#provider-upstream .action-icon-btn.primary')
        page.wait_for_selector('.modal-card')
        assert page.is_visible('.modal-card:has-text("Thêm Nhà Cung Cấp Upstream")')
        page.click('.modal-header .btn-icon-only')
        page.wait_for_timeout(200)

        # Verify no Google/Codex account management surfaces remain
        print("✓ Verifying removed account-pool surfaces are gone...")
        assert page.query_selector('.op-segmented-tab:has-text("Antigravity")') is None, "Antigravity tab still present"
        assert page.query_selector('.op-segmented-tab:has-text("Codex")') is None, "Codex tab still present"
        assert len(page.query_selector_all('.status-segment-chip')) == 2, "Unexpected extra status chip"
        print("✓ No Google Antigravity / OpenAI Codex account tabs or chips remain")

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
    key = sys.argv[2] if len(sys.argv) > 2 else "ezrouter-key"
    run_provider_checks(url, key)
