import sys
from playwright.sync_api import sync_playwright

def test_traffic_monitor(url: str, admin_key: str):
    print(f"Testing Traffic Monitor UI on {url}...")
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 900})

        # 1. Login
        print("1. Opening page and authenticating...")
        page.goto(url, wait_until="networkidle")
        page.fill('input[type="password"]', admin_key)
        page.click('button[type="submit"]')
        page.wait_for_selector('.sidebar', timeout=5000)
        print("   -> Authentication successful!")

        # 2. Check sidebar contains the Requests/Traffic tab
        print("2. Checking sidebar tab item...")
        traffic_item = page.locator('.nav-item:has-text("Lưu lượng")')
        assert traffic_item.count() > 0, "Sidebar does not contain the 'Lưu lượng' tab"
        print("   -> Found 'Lưu lượng' in sidebar!")

        # 3. Click Traffic Monitor tab
        print("3. Navigating to Traffic Monitor tab...")
        traffic_item.click()
        page.wait_for_timeout(500)

        # Check title
        title_text = page.locator('.top-navbar h1').inner_text()
        print(f"   Navbar Title: {title_text}")
        assert "Lưu lượng" in title_text, f"Navbar title mismatch: {title_text}"

        # 4. Verify Live Pipeline Header
        print("4. Checking live pipeline header controls...")
        assert page.locator('#pipelineFlow').is_visible(), "pipelineFlow not visible"
        assert page.locator('.pf-live-badge').is_visible(), "pf-live-badge not visible"
        assert page.locator('.pf-live-dot').is_visible(), "pf-live-dot not visible"
        assert page.locator('.pf-count').is_visible(), "active request counter not visible"
        print("   -> Live header controls verified!")

        # 4.5 Verify Request Pipeline Flow (2-node Client -> Router -> Upstream)
        print("4.5 Checking Request Pipeline Flow diagram...")
        assert page.locator('#pipelineFlow').is_visible(), "pipelineFlow diagram not visible"
        assert page.locator('.pf-diagram').is_visible(), "pf-diagram not visible"
        assert page.locator('.pf-node:has-text("Client")').is_visible(), "Client node not visible"
        assert page.locator('.pf-node:has-text("ezRouter")').is_visible(), "ezRouter node not visible"
        # Exactly one upstream-facing provider node survives (External / configured upstream)
        provider_nodes = page.locator('.pf-node.pf-provider')
        assert provider_nodes.count() == 1, f"Expected a single upstream provider node, got {provider_nodes.count()}"
        assert page.locator('.pf-active-bar').is_visible(), "pf-active-bar not visible"
        # Legacy Antigravity / Codex branches must be gone from the pipeline
        assert page.locator('.pf-node:has-text("Google Antigravity")').count() == 0, "Antigravity node still rendered"
        assert page.locator('.pf-node:has-text("OpenAI Codex")').count() == 0, "Codex node still rendered"
        print("   -> Request Pipeline Flow diagram verified (Client -> ezRouter -> Upstream)!")

        # 5. Verify Traffic KPI Cards
        print("5. Checking Traffic KPI Cards...")
        kpi_grid = page.locator('.traffic-kpi-grid')
        assert kpi_grid.is_visible(), "traffic-kpi-grid not visible"
        assert kpi_grid.locator('.card').count() == 5, "Expected 5 KPI cards"
        assert page.locator('text=Tổng Yêu Cầu').first.is_visible()
        assert page.locator('text=Monitoring').first.is_visible()
        assert page.locator('text=Thời Gian Stream TB').first.is_visible()
        assert page.locator('text=Tỷ Lệ Lỗi Hệ Thống').first.is_visible()
        assert page.locator('text=Tổng Tokens').first.is_visible()
        print("   -> 5 KPI Cards displayed properly!")

        # 6. Verify Filter Presets
        print("6. Checking quick filter preset chips...")
        presets = page.locator('.traffic-filter-presets')
        assert presets.is_visible(), "traffic-filter-presets not visible"
        # Legacy provider-specific presets must be gone
        assert page.locator('button:has-text("Google Antigravity")').count() == 0, "Antigravity preset chip still present"
        assert page.locator('button:has-text("OpenAI Codex")').count() == 0, "Codex preset chip still present"

        status_select = page.locator('.filter-item-status select').first
        model_input = page.locator('.filter-item-model input')

        page.click('.traffic-filter-presets button:has-text("Lỗi Hệ Thống")')
        page.wait_for_timeout(400)
        assert status_select.input_value() == 'error', f"Error preset did not set status filter: {status_select.input_value()}"
        print("   -> Quick preset chip 'Lỗi Hệ Thống' applied successfully!")

        page.click('.traffic-filter-presets button:has-text("Hết Quota")')
        page.wait_for_timeout(400)
        assert status_select.input_value() == 'quota_exhausted', f"Quota preset did not set status filter: {status_select.input_value()}"
        print("   -> Quick preset chip 'Hết Quota' applied successfully!")

        # Reset filter
        page.click('.traffic-filter-presets button:has-text("Tất Cả Models")')
        page.wait_for_timeout(400)
        assert model_input.input_value() == '', f"Reset preset did not clear model filter: {model_input.input_value()}"

        # 7. Verify Table Rows
        print("7. Checking traffic table rows...")
        rows = page.locator('tbody tr.clickable-row')
        row_count = rows.count()
        print(f"   Found {row_count} traffic log rows.")
        assert row_count > 0, "Expected at least 1 traffic log row"

        # Check badges on first row
        first_row = rows.first
        # Model family tags are now 'OR' (openrouter/*) or 'UP' (upstream/custom)
        has_family_tag = (
            first_row.locator('.family-tag-or').count() > 0
            or first_row.locator('.family-tag-custom').count() > 0
        )
        has_latency_badge = first_row.locator('.badge-latency').count() > 0
        assert has_family_tag, "Row missing OR/UP model family tag"
        assert has_latency_badge, "Row missing latency badge"
        print("   -> Row badges & layout verified!")

        # 8. Test Request Inspector Modal
        print("8. Testing Request Inspector Modal...")
        first_row.click()
        page.wait_for_selector('.modal-card', timeout=3000)
        assert page.locator('.modal-title').is_visible(), "Modal title not visible"
        modal_title = page.locator('.modal-title').inner_text()
        print(f"   Inspector Title: {modal_title}")
        assert "Yêu Cầu #" in modal_title

        # Check tabs in modal
        assert page.locator('button.inspector-tab-btn:has-text("Tổng Quan")').is_visible()
        assert page.locator('button.inspector-tab-btn:has-text("Dữ Liệu Thô (JSON)")').is_visible()

        # Switch to Raw JSON tab
        page.click('button.inspector-tab-btn:has-text("Dữ Liệu Thô (JSON)")')
        page.wait_for_timeout(200)
        assert page.locator('.code-viewer-box').is_visible()
        print("   -> Raw JSON tab verified!")

        # Close modal
        page.click('button:has-text("Đóng")')
        page.wait_for_timeout(300)
        assert page.locator('.modal-card').count() == 0, "Modal should be closed"
        print("   -> Modal closed cleanly!")

        # 9. Test Mobile Viewport (390x844)
        print("9. Testing Mobile Responsiveness (390x844)...")
        page.set_viewport_size({"width": 390, "height": 844})
        page.wait_for_timeout(400)

        # Check horizontal overflow
        scroll_width = page.evaluate("document.documentElement.scrollWidth")
        inner_width = page.evaluate("window.innerWidth")
        print(f"   Mobile widths: scrollWidth={scroll_width}, innerWidth={inner_width}")
        assert scroll_width <= inner_width + 1, f"Horizontal overflow on mobile! {scroll_width} > {inner_width}"

        # Check table container scrolls horizontally
        table_container = page.locator('.table-container')
        assert table_container.is_visible()
        print("   -> Mobile viewport verified with 0 horizontal overflow!")

        browser.close()
        print("\nAll Traffic Monitor E2E tests PASSED successfully! 🚀")

if __name__ == "__main__":
    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:20229"
    admin_key = sys.argv[2] if len(sys.argv) > 2 else "ezrouter-key"
    test_traffic_monitor(url, admin_key)
