import sys
from playwright.sync_api import sync_playwright

def test_integration_tab(url: str, admin_key: str):
    print("Testing Integration Tab at", url)
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        # Check both desktop (1280x800) and mobile (390x844)
        for vp, mode in [({"width": 1280, "height": 800}, "Desktop"), ({"width": 390, "height": 844}, "Mobile")]:
            print(f"\n--- Testing in {mode} ({vp['width']}x{vp['height']}) ---")
            context = browser.new_context(viewport=vp)
            # Grant clipboard permissions
            context.grant_permissions(["clipboard-read", "clipboard-write"])
            page = context.new_page()

            page.goto(url, wait_until="networkidle")
            page.fill('input[type="password"]', admin_key)
            page.click('button[type="submit"]')
            page.wait_for_selector('.top-navbar', timeout=5000)

            # Open sidebar if mobile
            if mode == "Mobile":
                page.click('.menu-toggle-btn')
                page.wait_for_timeout(300)

            # Verify 'Tích hợp API' navigation item exists and click it
            nav_item = page.locator('.nav-item:has-text("Tích hợp API")')
            assert nav_item.count() > 0, "Navigation item 'Tích hợp API' not found"
            print("  PASS: Navigation item 'Tích hợp API' found")
            nav_item.click()
            page.wait_for_timeout(400)

            # Check page header
            title = page.locator('h2.section-title')
            assert "Hướng Dẫn Tích Hợp API" in title.inner_text()
            print("  PASS: Header verified:", title.inner_text())

            # Check Security Alert box
            sec_alert = page.locator('.security-alert-box')
            assert sec_alert.count() > 0, "Security alert box not found"
            assert "Bảo Vệ Khóa API" in sec_alert.inner_text()
            print("  PASS: Security alert verified")

            # Check Base URL display
            assert "https://router.namhv.vip/v1" in page.content()
            print("  PASS: Base URL https://router.namhv.vip/v1 verified")

            # Check Model Names
            assert "ag/gemini-3.8-flash-high" in page.content()
            assert "cx/gpt-5.6-sol" in page.content()
            print("  PASS: Model examples ag/gemini-3.8-flash-high and cx/gpt-5.6-sol present")

            # Check Code tabs: Hermes, Codex CLI, OpenAI SDK Python, cURL, Node.js
            tab_labels = ["Hermes Agent", "Codex CLI", "OpenAI SDK Python", "cURL / Bash", "Node.js / TS"]
            for label in tab_labels:
                tab_btn = page.locator(f'.integration-tab-btn:has-text("{label}")')
                assert tab_btn.count() > 0, f"Tab button '{label}' not found"
                tab_btn.click()
                page.wait_for_timeout(200)
                # Verify code snippet pre exists and has content
                pre = page.locator('.integration-tab-pane pre')
                assert pre.count() > 0, f"Code block for '{label}' missing"
                snippet_text = pre.first.inner_text()
                assert len(snippet_text) > 30, f"Snippet for '{label}' too short"
                print(f"  PASS: Tab '{label}' verified (code length: {len(snippet_text)})")

            # Check Streaming Note
            body_text = page.inner_text('body')
            assert "Lưu Ý Về Cơ Chế Streaming" in body_text
            print("  PASS: Streaming note verified")

            # Check Tools Note
            assert "Lưu Ý Về Tools & Gọi Hàm" in body_text
            print("  PASS: Tools note verified")

            # Check Error Troubleshooting
            assert "Chẩn Đoán Lỗi Thường Gặp & Cách Khắc Phục" in body_text
            assert "401" in body_text
            assert "404" in body_text
            assert "429" in body_text
            assert "500" in body_text
            print("  PASS: Troubleshooting section verified")

            # Test Copy button
            copy_btn = page.locator('.integration-tab-pane button:has-text("Sao chép")').first
            if copy_btn.count() > 0:
                copy_btn.click()
                page.wait_for_timeout(200)
                # Should transition to 'Đã sao chép'
                assert page.locator('button:has-text("Đã sao chép")').count() > 0
                print("  PASS: Copy button feedback verified ('Đã sao chép')")

            context.close()
        browser.close()
    print("\nALL INTEGRATION TAB VERIFICATIONS PASSED SUCCESSFULLY!")

if __name__ == "__main__":
    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:20229"
    key = sys.argv[2] if len(sys.argv) > 2 else "ag-proxy-key"
    test_integration_tab(url, key)
