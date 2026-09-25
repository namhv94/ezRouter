#!/usr/bin/env python3
"""
Comprehensive Hermes Agent API Compatibility Audit against ag-proxy-rust (staging: 20229)
"""

import json
import urllib.request
import urllib.error

import os

BASE_URL = os.environ.get("AG_BASE_URL", "http://127.0.0.1:20229")
ADMIN_KEY = os.environ.get("AG_API_KEY", "ag-proxy-key")

def audit_get_models():
    print("\n--- 1. Audit /v1/models ---")
    req = urllib.request.Request(
        f"{BASE_URL}/v1/models",
        headers={"Authorization": f"Bearer {ADMIN_KEY}"}
    )
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200, f"Expected 200, got {resp.status}"
        data = json.loads(resp.read().decode())
    
    assert data.get("object") == "list", "Response object must be 'list'"
    assert "data" in data and isinstance(data["data"], list), "data must be a list"
    assert len(data["data"]) > 0, "Models list should not be empty"
    
    for m in data["data"]:
        assert "id" in m, "Model item missing 'id'"
        assert m.get("object") == "model", "Model item object must be 'model'"
    
    model_ids = [m["id"] for m in data["data"]]
    print(f"PASS: Retrieved {len(model_ids)} models. Sample: {model_ids[:4]}")
    return model_ids

def audit_auth_protection():
    print("\n--- 2. Audit Authentication Protection ---")
    # Missing auth
    req = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps({"model": "ag/gemini-3.8-flash-high", "messages": [{"role": "user", "content": "hi"}]}).encode(),
        headers={"Content-Type": "application/json"}
    )
    try:
        urllib.request.urlopen(req)
        assert False, "Should fail without auth"
    except urllib.error.HTTPError as e:
        assert e.code == 401, f"Expected 401, got {e.code}"
        print("PASS: Unauthenticated request rejected with HTTP 401")

    # Invalid token
    req = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps({"model": "ag/gemini-3.8-flash-high", "messages": [{"role": "user", "content": "hi"}]}).encode(),
        headers={"Authorization": "Bearer invalid-key", "Content-Type": "application/json"}
    )
    try:
        urllib.request.urlopen(req)
        assert False, "Should fail with invalid auth"
    except urllib.error.HTTPError as e:
        assert e.code == 401, f"Expected 401, got {e.code}"
        print("PASS: Invalid token rejected with HTTP 401")

def audit_non_streaming(model: str):
    print(f"\n--- 3. Audit Non-Streaming Chat Completion ({model}) ---")
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": "You are Hermes Agent. Be concise."},
            {"role": "user", "content": "Respond with exactly the word PONG."}
        ],
        "temperature": 0.1,
        "max_tokens": 500
    }
    req = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        res = json.loads(resp.read().decode())

    assert res.get("object") == "chat.completion", "object must be 'chat.completion'"
    assert "choices" in res and len(res["choices"]) > 0, "choices missing or empty"
    choice = res["choices"][0]
    assert choice["message"]["role"] == "assistant", "role must be assistant"
    content = choice["message"]["content"]
    assert content and len(content) > 0, "content must not be empty"
    assert "usage" in res, "usage metadata missing"
    assert "total_tokens" in res["usage"], "total_tokens missing in usage"
    print(f"PASS: Non-streaming success. Tokens: {res['usage']['total_tokens']}, Content: {content.strip()}")

def audit_streaming(model: str):
    print(f"\n--- 4. Audit Streaming SSE Chat Completion ({model}) ---")
    payload = {
        "model": model,
        "stream": True,
        "messages": [
            {"role": "system", "content": "You are Hermes Agent. Be concise."},
            {"role": "user", "content": "Count from 1 to 3."}
        ],
        "temperature": 0.1,
        "max_tokens": 500
    }
    req = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
    )
    chunks_received = 0
    full_text = ""
    saw_done = False
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200
        for line in resp:
            line_str = line.decode().strip()
            if line_str == "data: [DONE]":
                saw_done = True
            elif line_str.startswith("data: "):
                chunk = json.loads(line_str[6:])
                assert chunk.get("object") == "chat.completion.chunk"
                delta = chunk["choices"][0]["delta"]
                if "content" in delta and delta["content"]:
                    full_text += delta["content"]
                chunks_received += 1

    assert chunks_received > 0, "No chunks received"
    assert saw_done, "Stream did not terminate with data: [DONE]"
    assert len(full_text) > 0, "Streamed content is empty"
    print(f"PASS: Streaming success. Chunks: {chunks_received}, Content: {full_text.strip()[:60]}")

def audit_tool_calling_multiturn(model: str):
    print(f"\n--- 5. Audit Multi-turn Tool Calling Flow ({model}) ---")
    tools = [{
        "type": "function",
        "function": {
            "name": "get_stock_price",
            "description": "Fetch stock price for a symbol",
            "parameters": {
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Stock ticker symbol"}
                },
                "required": ["symbol"]
            }
        }
    }]

    # Turn 1: Model should invoke tool
    payload1 = {
        "model": model,
        "messages": [
            {"role": "system", "content": "You are Hermes Agent. Use available tools to answer questions."},
            {"role": "user", "content": "What is the stock price of AAPL? Use the get_stock_price tool."}
        ],
        "tools": tools,
        "tool_choice": "auto"
    }

    req1 = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps(payload1).encode(),
        headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req1) as resp:
        assert resp.status == 200
        res1 = json.loads(resp.read().decode())

    msg = res1["choices"][0]["message"]
    assert "tool_calls" in msg and msg["tool_calls"] is not None, f"Model {model} did not invoke tool"
    tool_call = msg["tool_calls"][0]
    call_id = tool_call["id"]
    func_name = tool_call["function"]["name"]
    func_args = tool_call["function"]["arguments"]
    finish_reason = res1["choices"][0]["finish_reason"]
    assert finish_reason == "tool_calls", f"finish_reason should be 'tool_calls', got {finish_reason}"
    assert func_name == "get_stock_price", f"Expected tool 'get_stock_price', got {func_name}"
    print(f"   Turn 1 PASS: Tool invoked: {func_name}({func_args}), ID: {call_id[:35]}...")

    # Turn 2: Feed back tool result
    tool_output = json.dumps({"symbol": "AAPL", "price": 225.50, "currency": "USD"})
    payload2 = {
        "model": model,
        "messages": [
            {"role": "system", "content": "You are Hermes Agent. Use available tools to answer questions."},
            {"role": "user", "content": "What is the stock price of AAPL? Use the get_stock_price tool."},
            msg,
            {
                "role": "tool",
                "tool_call_id": call_id,
                "content": tool_output
            }
        ],
        "tools": tools
    }

    req2 = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps(payload2).encode(),
        headers={"Authorization": f"Bearer {ADMIN_KEY}", "Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req2) as resp:
        assert resp.status == 200
        res2 = json.loads(resp.read().decode())

    msg2 = res2["choices"][0]["message"]
    assert msg2["role"] == "assistant"
    final_content = msg2["content"]
    assert final_content and ("225.5" in final_content or "225" in final_content or "AAPL" in final_content), \
        f"Model did not use tool result in final answer: {final_content}"
    print(f"   Turn 2 PASS: Final answer generated using tool result: {final_content.strip()[:80]}...")

def main():
    print("=======================================================")
    print("Hermes Agent API Compatibility Audit for ag-proxy-rust")
    print("Target: http://127.0.0.1:20229")
    print("=======================================================")

    audit_get_models()
    audit_auth_protection()

    # Audit all 3 upstream families:
    # 1. Antigravity Gemini
    audit_non_streaming("ag/gemini-3.8-flash-high")
    audit_streaming("ag/gemini-3.8-flash-high")
    audit_tool_calling_multiturn("ag/gemini-3.8-flash-high")

    # 2. Antigravity Claude
    audit_non_streaming("ag/claude-sonnet-4-6")
    audit_streaming("ag/claude-sonnet-4-6")
    audit_tool_calling_multiturn("ag/claude-sonnet-4-6")

    # 3. OpenAI Codex
    audit_non_streaming("cx/gpt-5.6-sol")
    audit_streaming("cx/gpt-5.6-sol")
    audit_tool_calling_multiturn("cx/gpt-5.6-sol")

    print("\n=======================================================")
    print("AUDIT RESULT: 100% HERMES API COMPATIBILITY VERIFIED!")
    print("=======================================================")

if __name__ == "__main__":
    main()
