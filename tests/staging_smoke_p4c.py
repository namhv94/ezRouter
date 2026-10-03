import os
import sys
import time
import json
import base64
import socket
import urllib.request
import subprocess
from http.server import HTTPServer, BaseHTTPRequestHandler
import threading

def get_free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(('127.0.0.1', 0))
    port = s.getsockname()[1]
    s.close()
    return port

mock_port = get_free_port()
staging_port = 20229
admin_key = "staging-p4c-secret"
staging_dir = "/home/namhv/.ag-proxy-rust-staging"

# Mock Codex upstream server
class MockCodexHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        content_len = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_len).decode('utf-8')
        
        if self.path == '/oauth/token':
            # OAuth token exchange or refresh
            jwt_payload = {
                "email": "smoke-test@example.com",
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "acc-smoke-12345"
                },
                "exp": 1900000000
            }
            enc = base64.urlsafe_b64encode(json.dumps(jwt_payload).encode()).decode().rstrip('=')
            mock_jwt = f"header.{enc}.sig"
            
            resp_data = {
                "access_token": "smoke-access-token-999",
                "refresh_token": "smoke-refresh-token-999",
                "id_token": mock_jwt,
                "account_id": "acc-smoke-12345",
                "expires_in": 3600
            }
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(resp_data).encode('utf-8'))
            return
            
        if '/codex/responses' in self.path:
            payload = json.loads(body) if body else {}
            has_tools = "tools" in payload and payload["tools"]
            
            if has_tools:
                sse = (
                    "event: response.output_item.added\n"
                    "data: {\"type\": \"response.output_item.added\", \"item\": {\"type\": \"function_call\", \"id\": \"call_smoke_1\", \"name\": \"smoke_calc\"}}\n\n"
                    "event: response.function_call_arguments.delta\n"
                    "data: {\"type\": \"response.function_call_arguments.delta\", \"delta\": \"{\\\"res\\\": 100}\"}\n\n"
                    "event: response.output_item.done\n"
                    "data: {\"type\": \"response.output_item.done\", \"item\": {\"type\": \"function_call\", \"arguments\": \"{\\\"res\\\": 100}\"}}\n\n"
                    "event: response.completed\n"
                    "data: {\"type\": \"response.completed\", \"response\": {\"usage\": {\"input_tokens\": 10, \"output_tokens\": 5}}}\n\n"
                )
            else:
                sse = (
                    "event: response.output_text.delta\n"
                    "data: {\"type\": \"response.output_text.delta\", \"delta\": \"Smoke test Codex success!\"}\n\n"
                    "event: response.completed\n"
                    "data: {\"type\": \"response.completed\", \"response\": {\"usage\": {\"input_tokens\": 12, \"output_tokens\": 6}}}\n\n"
                )
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.end_headers()
            self.wfile.write(sse.encode('utf-8'))
            return
            
        self.send_response(404)
        self.end_headers()

    def do_GET(self):
        if '/wham/usage' in self.path:
            resp_data = {
                "primary_window": {
                    "used_percent": 15.0,
                    "reset_after_seconds": 3600
                },
                "secondary_window": {
                    "used_percent": 10.0,
                    "reset_after_seconds": 86400
                }
            }
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(resp_data).encode('utf-8'))
            return
        self.send_response(404)
        self.end_headers()

    def log_message(self, format, *args):
        pass

mock_server = HTTPServer(('127.0.0.1', mock_port), MockCodexHandler)
mock_thread = threading.Thread(target=mock_server.serve_forever, daemon=True)
mock_thread.start()
print(f"Mock upstream server started on port {mock_port}")

# Start ag-proxy-rust staging binary
env = os.environ.copy()
env["AG_PORT"] = str(staging_port)
env["AG_DATA_DIR"] = staging_dir
env["AG_API_KEY"] = admin_key
env["CODEX_BASE_URL"] = f"http://127.0.0.1:{mock_port}"
env["CODEX_TOKEN_URL"] = f"http://127.0.0.1:{mock_port}/oauth/token"
env["CODEX_USAGE_URL"] = f"http://127.0.0.1:{mock_port}/wham/usage"

binary_path = "/home/namhv/ag-proxy-rust/target/debug/ag-proxy-rust"
proc = subprocess.Popen([binary_path], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

def cleanup():
    proc.terminate()
    try:
        proc.wait(timeout=3)
    except Exception:
        proc.kill()
    mock_server.shutdown()

time.sleep(1.0)
if proc.poll() is not None:
    stdout, stderr = proc.communicate()
    print("Failed to start ag-proxy-rust:", stderr.decode())
    sys.exit(1)

def http_req(path, method="GET", data=None, headers=None):
    url = f"http://127.0.0.1:{staging_port}{path}"
    h = headers or {}
    req_body = json.dumps(data).encode('utf-8') if data is not None else None
    req = urllib.request.Request(url, data=req_body, headers=h, method=method)
    with urllib.request.urlopen(req) as resp:
        return resp.status, resp.read().decode('utf-8')

try:
    print("1. Testing GET /health...")
    status, body = http_req("/health")
    assert status == 200, f"Expected 200, got {status}"
    health_json = json.loads(body)
    assert health_json["status"] == "ok"
    assert health_json["port"] == staging_port
    print("   -> /health OK:", health_json)

    print("2. Testing POST /admin/codex/oauth/start...")
    status, body = http_req(
        "/admin/codex/oauth/start",
        method="POST",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    start_data = json.loads(body)
    assert start_data["ok"] is True
    state_str = start_data["state"]
    print(f"   -> OAuth start OK, state: {state_str[:8]}...")

    print("3. Testing GET /admin/codex/oauth/status...")
    status, body = http_req(
        f"/admin/codex/oauth/status?state={state_str}",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    status_data = json.loads(body)
    assert status_data["status"] == "pending"
    print("   -> OAuth status OK (pending)")

    print("4. Testing POST /admin/codex/oauth/exchange...")
    status, body = http_req(
        "/admin/codex/oauth/exchange",
        method="POST",
        data={"code": "auth-code-smoke", "state": state_str},
        headers={
            "Authorization": f"Bearer {admin_key}",
            "Content-Type": "application/json"
        }
    )
    assert status == 200
    exchange_data = json.loads(body)
    assert exchange_data["ok"] is True
    assert exchange_data["email"] == "smoke-test@example.com"
    print("   -> OAuth exchange OK, email:", exchange_data["email"])

    print("5. Testing GET /admin/codex/accounts (verifying secret safety)...")
    status, body = http_req(
        "/admin/codex/accounts",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    assert "smoke-access-token-999" not in body
    assert "smoke-refresh-token-999" not in body
    accs_data = json.loads(body)
    created_acc = next(a for a in accs_data["accounts"] if a["email"] == "smoke-test@example.com")
    acc_id = created_acc["id"]
    print(f"   -> Accounts list OK, found account ID: {acc_id}, secrets NOT exposed")

    print(f"6. Testing POST /admin/codex/accounts/{acc_id}/toggle...")
    status, body = http_req(
        f"/admin/codex/accounts/{acc_id}/toggle",
        method="POST",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    toggle_data = json.loads(body)
    assert toggle_data["is_active"] is False
    # Toggle back to active
    status, body = http_req(
        f"/admin/codex/accounts/{acc_id}/toggle",
        method="POST",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    print("   -> Account toggle OK (deactivated and reactivated)")

    print(f"7. Testing POST /admin/codex/accounts/{acc_id}/reset...")
    status, body = http_req(
        f"/admin/codex/accounts/{acc_id}/reset",
        method="POST",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    print("   -> Account reset OK")

    print("8. Testing POST /v1/chat/completions (sync cx/gpt-5.6-sol)...")
    chat_payload = {
        "model": "cx/gpt-5.6-sol",
        "messages": [{"role": "user", "content": "Smoke ping"}],
        "stream": False
    }
    status, body = http_req(
        "/v1/chat/completions",
        method="POST",
        data=chat_payload,
        headers={
            "Authorization": f"Bearer {admin_key}",
            "Content-Type": "application/json"
        }
    )
    assert status == 200
    chat_resp = json.loads(body)
    assert chat_resp["model"] == "cx/gpt-5.6-sol"
    assert "Smoke test Codex success!" in chat_resp["choices"][0]["message"]["content"]
    print("   -> Sync chat completion OK:", chat_resp["choices"][0]["message"]["content"])

    print("9. Testing POST /v1/chat/completions (tool calls)...")
    tool_payload = {
        "model": "cx/gpt-5.6-sol",
        "messages": [{"role": "user", "content": "Calculate something"}],
        "stream": False,
        "tools": [{
            "type": "function",
            "function": {
                "name": "smoke_calc",
                "description": "Calculate",
                "parameters": {"type": "object", "properties": {"res": {"type": "integer"}}}
            }
        }]
    }
    status, body = http_req(
        "/v1/chat/completions",
        method="POST",
        data=tool_payload,
        headers={
            "Authorization": f"Bearer {admin_key}",
            "Content-Type": "application/json"
        }
    )
    assert status == 200
    tool_resp = json.loads(body)
    assert tool_resp["choices"][0]["finish_reason"] == "tool_calls"
    tc = tool_resp["choices"][0]["message"]["tool_calls"]
    assert tc[0]["function"]["name"] == "smoke_calc"
    print("   -> Tool calls chat completion OK, finish_reason: tool_calls, tool:", tc[0]["function"]["name"])

    print(f"10. Testing DELETE /admin/codex/accounts/{acc_id}...")
    status, body = http_req(
        f"/admin/codex/accounts/{acc_id}",
        method="DELETE",
        headers={"Authorization": f"Bearer {admin_key}"}
    )
    assert status == 200
    del_data = json.loads(body)
    assert del_data["deleted_id"] == acc_id
    print("   -> Account delete OK")

    print("\nALL STAGING SMOKE CHECKS PASSED SUCCESSFULLY!")
finally:
    cleanup()
