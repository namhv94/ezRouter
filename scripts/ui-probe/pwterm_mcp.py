#!/usr/bin/env python3
"""pwterm MCP stdio driver — shared helper for browser automation against a local UI.

Speaks newline-delimited JSON-RPC 2.0 with the pwterm MCP server (Playwright-style
browser_* tools on system Chromium, headless). Used by the probe scripts in this
directory and reusable for any other UI checks.

Usage:
    from pwterm_mcp import PWTerm
    with PWTerm() as p:
        p.login_admin("http://127.0.0.1:20229/", "123456")
        print(p.snapshot())
"""
import json
import os
import subprocess

PWTERM_MCP = os.environ.get(
    "PWTERM_MCP", "/data/data/com.termux/files/usr/bin/pwterm-mcp"
)


class PWTerm:
    def __init__(self, command=PWTERM_MCP):
        self.proc = subprocess.Popen(
            [command],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            env={**os.environ, "HOME": os.environ.get("HOME", "/")},
        )
        self._id = 0
        self._init()

    # --- JSON-RPC plumbing -------------------------------------------------
    def _read_msg(self):
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "id" in msg:  # skip notifications/events
                return msg
        return None

    def _call(self, method, params=None):
        self._id += 1
        req = {"jsonrpc": "2.0", "id": self._id, "method": method}
        if params is not None:
            req["params"] = params
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        resp = self._read_msg()
        if resp is None:
            raise RuntimeError(f"no response from MCP server for {method}")
        if "error" in resp:
            raise RuntimeError(f"{method} failed: {resp['error']}")
        return resp["result"]

    def _notify(self, method, params=None):
        req = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            req["params"] = params
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()

    def _init(self):
        self._call(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "ezrouter-ui-probe", "version": "1.0"},
            },
        )
        self._notify("notifications/initialized")

    # --- browser tools -----------------------------------------------------
    def tool(self, name, arguments):
        """Call a browser_* tool; returns its text output."""
        res = self._call("tools/call", {"name": name, "arguments": arguments})
        return "\n".join(
            c.get("text", "") for c in res.get("content", []) if isinstance(c, dict)
        )

    def navigate(self, url):
        return self.tool("browser_navigate", {"url": url})

    def snapshot(self):
        return self.tool("browser_snapshot", {})

    def wait(self, ms):
        return self.tool("browser_wait", {"ms": ms})

    def screenshot(self, path):
        return self.tool("browser_screenshot", {"path": path})

    def js(self, script):
        """Evaluate JS in the page; returns a truncated string result."""
        return self.tool("browser_evaluate", {"script": script})[:2500]

    # --- ezRouter helpers ----------------------------------------------------
    def login_admin(self, base_url, admin_key):
        """Open the dashboard and authenticate with the admin bearer key."""
        self.navigate(base_url)
        self.wait(1500)
        self.js(
            "const s = Object.getOwnPropertyDescriptor("
            "window.HTMLInputElement.prototype,'value').set;"
            "const inp = document.querySelector('input');"
            f"s.call(inp, {json.dumps(admin_key)});"
            "inp.dispatchEvent(new Event('input', {bubbles:true})); 'ok'"
        )
        self.js(
            "[...document.querySelectorAll('button')]"
            ".find(b => /authenticate/i.test(b.textContent))?.click(); 'ok'"
        )
        self.wait(2500)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.terminate()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
