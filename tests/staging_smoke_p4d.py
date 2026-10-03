#!/usr/bin/env python3
"""
Staging Smoke Test for Phase 4D (Token Saver) against live ag-proxy-rust (port 20229).
"""

import json
import urllib.request
import urllib.error
import os
import sys

BASE_URL = os.environ.get("AG_BASE_URL", "http://127.0.0.1:20229")
ADMIN_KEY = os.environ.get("AG_API_KEY", "namhv94")

def main():
    print("=== Starting Phase 4D Staging Smoke Tests ===")
    
    # 1. Health check
    print("\n1. Verifying /health endpoint...")
    req = urllib.request.Request(f"{BASE_URL}/health")
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        health = json.loads(resp.read().decode())
        assert health["status"] == "ok"
        assert health["port"] == 20229
        print(f"   -> Health OK: service={health.get('service')}, port={health.get('port')}")

    # 2. Unauthenticated GET /admin/settings
    print("\n2. Verifying auth enforcement on /admin/settings...")
    try:
        req = urllib.request.Request(f"{BASE_URL}/admin/settings")
        urllib.request.urlopen(req)
        assert False, "Unauthenticated request should fail"
    except urllib.error.HTTPError as e:
        assert e.code == 401
        print("   -> Correctly rejected with HTTP 401 Unauthorized")

    # 3. Authenticated GET /admin/settings
    print("\n3. Verifying GET /admin/settings defaults...")
    req = urllib.request.Request(
        f"{BASE_URL}/admin/settings",
        headers={"Authorization": f"Bearer {ADMIN_KEY}"}
    )
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        settings = json.loads(resp.read().decode())
        assert "token_saver_enabled" in settings
        assert "rtk_enabled" in settings
        assert "caveman_level" in settings
        assert "ponytail_level" in settings
        print(f"   -> Settings defaults OK: {settings}")

    # 4. Authenticated POST /admin/settings
    print("\n4. Verifying POST /admin/settings updates...")
    payload = json.dumps({"caveman_level": "ultra", "ponytail_level": "lite"}).encode()
    req = urllib.request.Request(
        f"{BASE_URL}/admin/settings",
        data=payload,
        headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        updated = json.loads(resp.read().decode())
        assert updated["ok"] is True
        assert updated["caveman_level"] == "ultra"
        assert updated["ponytail_level"] == "lite"
        print(f"   -> Update OK: {updated}")

    # Restore defaults
    payload = json.dumps({"caveman_level": "lite", "ponytail_level": "full"}).encode()
    req = urllib.request.Request(
        f"{BASE_URL}/admin/settings",
        data=payload,
        headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        restored = json.loads(resp.read().decode())
        assert restored["caveman_level"] == "lite"
        assert restored["ponytail_level"] == "full"
        print(f"   -> Restored defaults OK: {restored}")

    # 5. Invalid settings payload returns 400
    print("\n5. Verifying validation on POST /admin/settings...")
    try:
        payload = json.dumps({"invalid_key": True}).encode()
        req = urllib.request.Request(
            f"{BASE_URL}/admin/settings",
            data=payload,
            headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
        )
        urllib.request.urlopen(req)
        assert False, "Invalid payload should fail"
    except urllib.error.HTTPError as e:
        assert e.code == 400
        print("   -> Correctly rejected invalid settings with HTTP 400 Bad Request")

    # 6. Chat completion with invalid x-caveman header returns 400
    print("\n6. Verifying header override validation...")
    try:
        chat_payload = json.dumps({
            "model": "ag/gemini-3.8-flash-high",
            "messages": [{"role": "user", "content": "hi"}]
        }).encode()
        req = urllib.request.Request(
            f"{BASE_URL}/v1/chat/completions",
            data=chat_payload,
            headers={
                "Authorization": f"Bearer {ADMIN_KEY}",
                "Content-Type": "application/json",
                "x-caveman": "invalid-level"
            }
        )
        urllib.request.urlopen(req)
        assert False, "Invalid header should fail"
    except urllib.error.HTTPError as e:
        assert e.code == 400
        print("   -> Correctly rejected invalid x-caveman with HTTP 400 Bad Request")

    # 7. Chat completion with x-token-saver: off
    print("\n7. Verifying x-token-saver: off override...")
    chat_payload = json.dumps({
        "model": "ag/gemini-3.8-flash-high",
        "messages": [{"role": "user", "content": "Ping test"}]
    }).encode()
    req = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=chat_payload,
        headers={
            "Authorization": f"Bearer {ADMIN_KEY}",
            "Content-Type": "application/json",
            "x-token-saver": "off"
        }
    )
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        chat_resp = json.loads(resp.read().decode())
        assert "choices" in chat_resp
        print("   -> Completed chat request with x-token-saver: off successfully")

    print("\n=== ALL PHASE 4D STAGING SMOKE CHECKS PASSED SUCCESSFULLY ===")

if __name__ == "__main__":
    main()
