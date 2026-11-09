#!/usr/bin/env python3
"""Tap every visible control in every Rune scene headlessly and report failures.

Uses Rune's own --tree/--script tooling. Each tap runs in a fresh process with
the scene as the base state. Reports panics/exit failures and taps that leave
the element tree identical (for manual review).
"""
import argparse
import concurrent.futures
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = os.path.join(ROOT, "target", "debug", "aster-rune")
SIZE = "1440x900"
ROLES = {"button", "menuitem", "switch", "checkbox", "radio", "tab", "link"}

SCENES = [
    "demo", "compose", "command", "menu", "filters", "snooze", "delete",
    "preview", "outbox", "settings", "settings-mail", "settings-rules",
    "settings-aliases", "settings-contacts", "settings-security",
    "settings-mcp", "settings-diagnostics",
]


def run(args, timeout=45):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True, timeout=timeout)


def tree(scene, path):
    r = run([BIN, "--scene", scene, "--tree", path, "--size", SIZE], timeout=60)
    if r.returncode != 0:
        return None, r
    return json.load(open(path)), r


def candidates(doc):
    out, seen = [], set()
    for e in doc.get("elements", []):
        if e.get("role") not in ROLES:
            continue
        x, y, w, h = e.get("rect") or [0, 0, 0, 0]
        if w <= 0 or h <= 0:
            continue
        cx, cy = x + w / 2.0, y + h / 2.0
        if not (0 <= cx < 1440 and 0 <= cy < 900):
            continue
        key = (round(cx), round(cy))
        if key in seen:
            continue
        seen.add(key)
        out.append((e.get("role"), e.get("key") or e.get("label") or "?", round(cx, 1), round(cy, 1)))
    return out


def fingerprint(doc):
    parts = []
    for e in doc.get("elements", []):
        if e.get("role") in ("text", "button", "listitem", "link", "menuitem"):
            parts.append((e.get("role"), e.get("key"), e.get("text") or e.get("label") or "", tuple(e.get("rect") or [])))
    return json.dumps(parts, sort_keys=True)


def tap(scene, role, name, cx, cy, out):
    script = {"events": [{"t": 0.55, "type": "tap", "x": cx, "y": cy, "hold": 0.04}]}
    tag = f"{scene}-{abs(hash((scene, cx, cy)))}"
    sp = os.path.join(out, f"{tag}.script.json")
    after_path = os.path.join(out, f"{tag}.after.json")
    json.dump(script, open(sp, "w"))
    try:
        r = run([BIN, "--scene", scene, "--script", sp, "--frames", "34", "--tree", after_path, "--size", SIZE], timeout=45)
    except subprocess.TimeoutExpired:
        return ("timeout", scene, f"{role}:{name}", "")
    if r.returncode != 0 or "panicked" in (r.stdout + r.stderr).lower():
        return ("fail", scene, f"{role}:{name} @{cx},{cy}", r.stdout[-600:] + r.stderr[-600:])
    try:
        after = json.load(open(after_path))
    except Exception as exc:
        return ("fail", scene, f"{role}:{name}", f"tree: {exc}")
    return ("ok", scene, f"{role}:{name} @{cx},{cy}", after)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scene", action="append")
    ap.add_argument("--limit", type=int, default=80)
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--out", default="/tmp/rune-qa")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    scenes = args.scene or SCENES
    bases = {}
    for scene in scenes:
        doc, r = tree(scene, os.path.join(args.out, f"{scene}.base.json"))
        if doc is None:
            print(f"BASE FAIL {scene}: {(r.stdout + r.stderr)[-400:]}")
            continue
        bases[scene] = (doc, fingerprint(doc))
        print(f"{scene}: {min(len(candidates(doc)), args.limit)} controls", flush=True)

    tasks = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for scene, (doc, _) in bases.items():
            for role, name, cx, cy in candidates(doc)[: args.limit]:
                tasks.append(pool.submit(tap, scene, role, name, cx, cy, args.out))
        total = len(tasks)
        failures, nochange = [], []
        for i, fut in enumerate(concurrent.futures.as_completed(tasks), 1):
            status, scene, name, detail = fut.result()
            if status != "ok":
                failures.append((scene, name, detail))
            elif fingerprint(detail) == bases[scene][1]:
                nochange.append((scene, name))
            if i % 50 == 0:
                print(f"  {i}/{total} tapped, {len(failures)} failures", flush=True)

    print(f"\ncontrols tapped: {total}")
    print(f"failures: {len(failures)}")
    for scene, name, detail in failures[:60]:
        print(f"  FAIL {scene} {name}: {detail.strip()[:400]}")
    print(f"no visible tree change: {len(nochange)}")
    for scene, name in nochange[:120]:
        print(f"  no-change {scene} {name}")
    open(os.path.join(args.out, "report.json"), "w").write(json.dumps({
        "total": total,
        "failures": failures,
        "nochange": nochange,
    }, indent=1))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
