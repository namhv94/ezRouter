import sys
from playwright.sync_api import sync_playwright

def test_playground(url: str, admin_key: str):
    print(f"Testing Playground UI on {url}...")
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 900})

        # 1. Login
        print("1. Opening page and authenticating...")
        page.goto(url, wait_until="networkidle")
        page.fill('input[type="password"]', admin_key)
        page.click('button[type="submit"]')
        page.wait_for_selector('.sidebar', timeout=5000)

        # 2. Check sidebar contains Playground / Chat Test
        print("2. Checking sidebar tab item...")
        playground_item = page.locator('.nav-item:has-text("Playground / Chat Test")')
        assert playground_item.count() > 0, "Sidebar does not contain Playground / Chat Test tab"
        print("   -> Found Playground / Chat Test in sidebar!")

        # 3. Click Playground tab
        print("3. Navigating to Playground tab...")
        playground_item.click()
        page.wait_for_timeout(400)

        # Check title
        title_text = page.locator('.top-navbar h1').inner_text()
        print(f"   Navbar Title: {title_text}")
        assert "Playground" in title_text

        # 4. Verify Empty State
        print("4. Checking initial empty state...")
        assert page.locator('text=Playground sẵn sàng').is_visible()

        # 5. Check model selection
        print("5. Checking model dropdown...")
        model_select = page.locator('#model-select')
        assert model_select.is_visible()
        options = model_select.locator('option').all_inner_texts()
        print(f"   Available model options count: {len(options)}")
        print(f"   Sample options: {options[:4]}")
        has_ag = any(o.startswith("ag/") for o in options)
        has_cx = any(o.startswith("cx/") for o in options)
        assert has_ag, "Models dropdown missing ag/* models"
        assert has_cx, "Models dropdown missing cx/* models"

        # 6. Check Auth Key input
        print("6. Checking Auth Key input...")
        auth_input = page.locator('#auth-key-input')
        assert auth_input.is_visible()
        current_val = auth_input.input_value()
        assert current_val == admin_key, f"Auth key input ({current_val}) should default to admin key ({admin_key})"

        # 7. Check Prompt presets
        print("7. Testing prompt preset buttons...")
        page.click('button:has-text("⚡ Test nhanh")')
        prompt_val = page.locator('#user-prompt').input_value()
        assert "Thủ đô của Việt Nam" in prompt_val
        print("   -> Preset prompt applied successfully")

        # 8. Test sending Non-stream request
        print("8. Testing Non-stream request execution...")
        page.click('button:has-text("Non-stream (JSON)")')
        page.click('button:has-text("Gửi Yêu Cầu")')
        # Wait for completion
        page.wait_for_selector('.playground-output-box', timeout=20000)
        
        # Verify status badge and response output
        assert page.locator('.badge:has-text("200 OK")').is_visible()
        alert_error = page.locator('.alert-error')
        assert not alert_error.is_visible(), f"Unexpected error banner: {alert_error.inner_text() if alert_error.is_visible() else ''}"
        resp_box = page.locator('.playground-output-box')
        assert resp_box.is_visible()
        resp_text = resp_box.inner_text()
        print(f"   -> Non-stream response received ({len(resp_text)} chars): {resp_text[:60]}...")

        # 9. Test sending Stream request
        print("9. Testing Stream request execution...")
        page.click('button:has-text("Stream (SSE)")')
        page.click('button:has-text("Gửi Yêu Cầu")')
        page.wait_for_selector('.badge:has-text("200 OK")', timeout=20000)
        resp_box = page.locator('.playground-output-box')
        assert resp_box.is_visible()
        stream_text = resp_box.inner_text()
        print(f"   -> Stream response received ({len(stream_text)} chars): {stream_text[:60]}...")

        # 10. Test custom model input toggle
        print("10. Testing custom model toggle...")
        page.click('text=+ Nhập model tùy chỉnh')
        custom_input = page.locator('input[placeholder*="ag/gemini"]')
        assert custom_input.is_visible()
        page.click('text=← Chọn từ danh sách')
        assert model_select.is_visible()

        # 11. Test invalid API key error handling
        print("11. Testing invalid API key response...")
        auth_input.fill("invalid-test-key-1234")
        page.click('button:has-text("Gửi Yêu Cầu")')
        page.wait_for_timeout(1000)
        assert page.locator('text=401 Error').is_visible() or page.locator('text=Invalid API key').is_visible()
        print("   -> 401 Invalid API key caught and rendered in Vietnamese error banner!")

        print("\nALL PLAYGROUND E2E TESTS PASSED!")
        browser.close()

if __name__ == "__main__":
    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:20229"
    key = sys.argv[2] if len(sys.argv) > 2 else "ag-proxy-key"
    test_playground(url, key)
