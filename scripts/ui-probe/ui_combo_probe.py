#!/usr/bin/env python3
"""UI probe: verify the Combos save flow round-trips model names correctly.

Regression test for the "wrong name in route" bug: `/v1/models` used to list
collapsed ids like `nvidia/nemotron-...` for providers whose stored models
already carried their own namespace. Saving such an id into a combo produced a
route that 404'd, because dispatch strips the provider prefix (`X/rest` ->
provider X, then `{prefix}/` is stripped from `rest`).

Checks (read-only unless --save-probe is given):
  1. every model id offered in the combo picker satisfies the invariant
     `id == {provider_prefix}/{stored_upstream_model}` (round-trips through
     routing + dispatch strip),
  2. (with --save-probe) creating and saving a probe combo stores exactly the
     ids that were picked, then deletes it again.

Usage:
    AG_ADMIN_KEY=123456 python3 ui_combo_probe.py \
        [--base-url http://127.0.0.1:20229] [--filter nvidia] [--save-probe]

Exits non-zero if any offered id violates the invariant.
"""
import argparse
import json
import os
import sqlite3
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pwterm_mcp import PWTerm  # noqa: E402

DEFAULT_DB = os.path.expanduser("~/.ezrouter/data/data.sqlite")


def fetch_model_ids(base_url, admin_key):
    req = urllib.request.Request(
        f"{base_url.rstrip('/')}/v1/models",
        headers={"Authorization": f"Bearer {admin_key}"},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        return [m["id"] for m in json.load(r)["data"]]


def provider_prefixes(db_path):
    conn = sqlite3.connect(db_path)
    try:
        return dict(
            conn.execute("SELECT prefix, models FROM providers WHERE is_active = 1")
        )
    finally:
        conn.close()


def roundtrip_ok(model_id, providers):
    """id must equal {prefix}/{stored} for some active provider (dispatch invariant)."""
    if "/" not in model_id:
        return False
    prefix, rest = model_id.split("/", 1)
    stored = json.loads(providers.get(prefix) or "[]")
    return rest in stored


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--base-url", default="http://127.0.0.1:20229")
    ap.add_argument("--admin-key", default=os.environ.get("AG_ADMIN_KEY", "123456"))
    ap.add_argument("--db", default=DEFAULT_DB)
    ap.add_argument("--filter", default="", help="only check ids containing this")
    ap.add_argument(
        "--save-probe",
        action="store_true",
        help="also create+save a probe combo via the UI, then delete it",
    )
    args = ap.parse_args()

    providers = provider_prefixes(args.db)
    ids = [i for i in fetch_model_ids(args.base_url, args.admin_key) if args.filter in i]

    print(f"== invariant check over {len(ids)} ids (filter={args.filter!r}) ==")
    bad = [i for i in ids if not roundtrip_ok(i, providers)]
    for i in ids:
        mark = "OK  " if i not in bad else "BAD "
        print(f"  {mark} {i}")
    if bad:
        print(f"\nFAIL: {len(bad)} offered id(s) would 404 when routed:")
        for i in bad:
            print(f"  {i}")
        sys.exit(1)
    print("all offered ids round-trip correctly")

    if not args.save_probe:
        return

    print("\n== UI save probe (create -> verify DB -> delete) ==")
    probe_name = f"probe-{int(time.time())}"
    picked = ids[:2]
    with PWTerm() as p:
        p.login_admin(args.base_url, args.admin_key)
        p.js(
            "[...document.querySelectorAll('button,a')]"
            ".filter(e => /^combos$/i.test(e.textContent.trim()))[0]?.click(); 'ok'"
        )
        p.wait(1500)
        p.js(
            "[...document.querySelectorAll('button')]"
            ".find(b => /create new combo/i.test(b.textContent))?.click(); 'ok'"
        )
        p.wait(1000)
        # type the name
        p.js(
            "const s = Object.getOwnPropertyDescriptor("
            "window.HTMLInputElement.prototype,'value').set;"
            "const n = [...document.querySelectorAll('input')].find(i => i.type==='text');"
            f"s.call(n, {json.dumps(probe_name)});"
            "n.dispatchEvent(new Event('input', {bubbles:true})); 'ok'"
        )
        # tick checkboxes for the picked ids
        p.js(
            "const want = "
            + json.dumps(picked)
            + ";[...document.querySelectorAll('input[type=checkbox]')]"
            ".filter(b => want.some(w => (b.closest('label,li,div,tr')?.textContent || '').includes(w)))"
            ".forEach(b => { if (!b.checked) b.click(); }); 'ok'"
        )
        p.wait(500)
        p.js("document.querySelector('button[type=submit]')?.click(); 'ok'")
        p.wait(2500)

    conn = sqlite3.connect(args.db)
    try:
        rows = conn.execute(
            "SELECT name, models FROM combos WHERE name = ?", (probe_name,)
        ).fetchall()
    finally:
        conn.close()
    if not rows:
        print(f"FAIL: probe combo '{probe_name}' not found in DB after UI save")
        sys.exit(1)
    saved = json.loads(rows[0][1])
    print(f"  picked: {picked}")
    print(f"  saved : {saved}")
    if saved != picked:
        print("FAIL: saved models differ from picked models (wrong name in route)")
        sys.exit(1)
    print("UI save round-trip OK")

    # cleanup via API
    req = urllib.request.Request(
        f"{args.base_url.rstrip('/')}/admin/combos",
        headers={"Authorization": f"Bearer {args.admin_key}"},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        combo_id = next(c["id"] for c in json.load(r) if c["name"] == probe_name)
    req = urllib.request.Request(
        f"{args.base_url.rstrip('/')}/admin/combos/{combo_id}",
        headers={"Authorization": f"Bearer {args.admin_key}"},
        method="DELETE",
    )
    urllib.request.urlopen(req, timeout=15)
    print(f"probe combo '{probe_name}' deleted")


if __name__ == "__main__":
    main()
