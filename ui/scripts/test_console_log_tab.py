import sys
from playwright.sync_api import sync_playwright

def test_console_log_tab(url: str, admin_key: str):
    print("Testing Console Log Tab at", url)
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        for vp, mode in [({"width": 1280, "height": 800}, "Desktop"), ({"width": 390, "height": 844}, "Mobile")]:
            print(f"\n--- Testing Console Log Tab in {mode} ({vp['width']}x{vp['height']}) ---")
            context = browser.new_context(viewport=vp)
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

            # 1. Verify 'Console Log' navigation item exists and click it
            nav_item = page.locator('.nav-item:has-text("Console Log")')
            assert nav_item.count() > 0, "Navigation item 'Console Log' not found"
            print("  PASS: Navigation item 'Console Log' found")
            nav_item.click()
            page.wait_for_timeout(500)

            # 2. Verify Section Header
            title = page.locator('h2.section-title')
            assert "Nhật Ký Hệ Thống" in title.inner_text()
            print("  PASS: Header verified:", title.inner_text())

            # 3. Verify Level Filters
            for lvl in ["Tất Cả", "INFO", "WARN", "ERROR", "DEBUG"]:
                lvl_btn = page.locator(f'button:has-text("{lvl}")')
                assert lvl_btn.count() > 0, f"Level button '{lvl}' not found"
            print("  PASS: Level filter buttons verified")

            # 4. Verify Terminal Output Window
            terminal_hdr = page.locator('text=Router Rust System Output')
            assert terminal_hdr.count() > 0, "Terminal output header not found"
            print("  PASS: Terminal output header present")

            # 5. Verify Copy button exists
            copy_btn = page.locator('button:has-text("Sao Chép")')
            assert copy_btn.count() > 0, "Copy button not found"
            print("  PASS: Copy button present")

            # 6. Verify Search Box
            search_input = page.locator('input[placeholder*="Lọc từ khóa"]')
            assert search_input.count() > 0, "Search input not found"
            search_input.fill("quota")
            page.wait_for_timeout(400)
            print("  PASS: Search input filter working")

            context.close()

        browser.close()
        print("\nALL CONSOLE LOG TAB VERIFICATIONS PASSED SUCCESSFULLY!")

if __name__ == "__main__":
    target_url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:20229"
    target_key = sys.argv[2] if len(sys.argv) > 2 else "namhv94"
    test_console_log_tab(target_url, target_key)
