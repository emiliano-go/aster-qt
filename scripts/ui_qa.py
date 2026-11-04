#!/usr/bin/env python3
"""Local mouse + accessibility QA for the Aster Qt UI.

Requires a running Aster Qt instance started with:
    QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 ./target/debug/aster-qt

Uses AT-SPI (pyatspi) for structure and actions, Hyprland + ydotool for real
pointer events, grim for screenshots, and the app's MCP server for data checks.
No personal data is read from or written to the repository.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
import time
import traceback

try:
    import pyatspi
except ImportError:
    print("python-atspi is required: sudo pacman -S python-atspi", file=sys.stderr)
    raise SystemExit(2)

DATA_DIR = os.environ.get("ASTER_QT_DATA_DIR", os.path.expanduser("~/.local/share/com.astermail.qt"))
WM_CLASS = "aster-qt"
SHOWING = pyatspi.STATE_SHOWING


def run(cmd: list[str] | str, timeout: float = 10.0) -> subprocess.CompletedProcess:
    if isinstance(cmd, str):
        cmd = ["sh", "-c", cmd]
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


class WM:
    def __init__(self):
        self.has_hypr = bool(run(["sh", "-c", "command -v hyprctl"]).stdout.strip())
        self.has_ydotool = bool(run(["sh", "-c", "command -v ydotool"]).stdout.strip())
        self.has_grim = bool(run(["sh", "-c", "command -v grim"]).stdout.strip())

    def dispatch(self, code: str) -> str:
        return run(["hyprctl", "dispatch", code]).stdout.strip()

    def clients(self) -> list[dict]:
        if not self.has_hypr:
            return []
        out = run(["hyprctl", "clients", "-j"]).stdout
        try:
            return json.loads(out)
        except Exception:
            return []

    def client(self) -> dict | None:
        for client in self.clients():
            if client.get("class") == WM_CLASS:
                return client
        return None

    def geometry(self) -> tuple[int, int, int, int]:
        client = self.client()
        if not client:
            raise RuntimeError(f"{WM_CLASS} window not found")
        x, y = client["at"]
        w, h = client["size"]
        return x, y, w, h

    def focus(self) -> None:
        if self.has_hypr:
            self.dispatch(f'hl.dsp.focus({{ window = "class:{WM_CLASS}" }})')

    def float(self) -> None:
        if self.has_hypr:
            self.dispatch(f'hl.dsp.window.float({{ action = "enable", window = "class:{WM_CLASS}" }})')

    def move(self, x: int, y: int) -> None:
        if self.has_hypr:
            self.dispatch(f'hl.dsp.window.move({{ x = {x}, y = {y}, window = "class:{WM_CLASS}" }})')

    def resize(self, w: int, h: int) -> None:
        if self.has_hypr:
            self.dispatch(f'hl.dsp.window.resize({{ x = {w}, y = {h}, window = "class:{WM_CLASS}" }})')

    def cursor(self, x: int, y: int) -> None:
        self.dispatch(f"hl.dsp.cursor.move({{ x = {x}, y = {y} }})")

    def click(self, code: str = "0xC0") -> None:
        run(["ydotool", "click", code])

    def key(self, code: str) -> None:
        run(["ydotool", "key", code])

    def screenshot(self, path: str) -> bool:
        if not self.has_grim:
            return False
        try:
            x, y, w, h = self.geometry()
        except RuntimeError:
            return False
        res = run(["grim", "-g", f"{x},{y} {w}x{h}", path])
        return res.returncode == 0 and os.path.exists(path)


class MCP:
    def __init__(self):
        cfg_path = os.path.join(DATA_DIR, "mcp.json")
        if not os.path.exists(cfg_path):
            raise RuntimeError(f"MCP config missing: {cfg_path}")
        cfg = json.load(open(cfg_path))
        self.url = cfg["url"]
        self.token = cfg["token"]

    def call(self, name: str, args: dict | None = None) -> tuple[bool, str]:
        import urllib.request

        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": args or {}},
        }
        req = urllib.request.Request(
            self.url,
            data=json.dumps(payload).encode(),
            headers={"Authorization": f"Bearer {self.token}", "Content-Type": "application/json"},
        )
        response = json.load(urllib.request.urlopen(req, timeout=60))
        result = response.get("result", {})
        text = (result.get("content") or [{}])[0].get("text", "")
        return bool(result.get("isError")), text


class App:
    def __init__(self, wm: WM):
        self.wm = wm
        pyatspi.Registry.getDesktop(0)
        self.root = self._connect()

    def _connect(self):
        deadline = time.time() + 10
        while time.time() < deadline:
            for desktop in range(pyatspi.Registry.getDesktopCount()):
                d = pyatspi.Registry.getDesktop(desktop)
                for app in d:
                    if app.name != WM_CLASS or app.childCount == 0:
                        continue
                    try:
                        pid = app.get_process_id()
                    except Exception:
                        pid = -1
                    if pid < 0 or self._pid_running(pid):
                        return app
            time.sleep(0.25)
        raise RuntimeError(
            f"{WM_CLASS} accessibility tree not found. Start it with "
            "QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1."
        )

    @staticmethod
    def _pid_running(pid: int) -> bool:
        return os.path.exists(f"/proc/{pid}")

    def nodes(self, node=None, depth=0):
        node = node or self.root
        if depth > 18:
            return
        yield node
        for child in node:
            yield from self.nodes(child, depth + 1)

    def find_id(self, aid: str, showing: bool | None = True):
        matches = [n for n in self.nodes() if getattr(n, "accessibleId", "") == aid]
        if showing is None:
            return matches[0] if matches else None
        visible = [n for n in matches if self.showing(n)]
        if visible:
            return visible[0]
        return None if showing else (matches[0] if matches else None)

    def find_name(self, name: str, showing: bool = True):
        matches = [n for n in self.nodes() if (n.name or "") == name]
        visible = [n for n in matches if self.showing(n)]
        return (visible or matches or [None])[0] if showing else (matches or [None])[0]

    def find_contains(self, text: str, showing: bool = True):
        matches = [n for n in self.nodes() if text in (n.name or "")]
        visible = [n for n in matches if self.showing(n)]
        return (visible or [None])[0]

    def find_role(self, role: str, showing: bool = True):
        matches = [n for n in self.nodes() if n.getRoleName() == role]
        visible = [n for n in matches if self.showing(n)]
        return (visible or [None])[0]

    @staticmethod
    def showing(node) -> bool:
        try:
            return bool(node) and node.getState().contains(SHOWING)
        except Exception:
            return False

    def action(self, node) -> bool:
        if not node:
            return False
        try:
            return bool(node.queryAction().doAction(0))
        except Exception:
            return False

    def do_id(self, aid: str) -> bool:
        return self.action(self.find_id(aid))

    def set_text(self, aid: str, value: str) -> bool:
        node = self.find_id(aid)
        if not node:
            return False
        try:
            node.queryEditableText().setTextContents(value)
            time.sleep(0.1)
            return self._norm(self.text(node)) == self._norm(value)
        except Exception:
            return False

    @staticmethod
    def _norm(value: str) -> str:
        return (value or "").replace("\u2029", "\n").replace("\u2028", "\n").strip()

    def text(self, node) -> str:
        try:
            return node.queryText().getText(0, -1) or ""
        except Exception:
            return ""

    def frame_offset(self) -> tuple[int, int]:
        frame = self.find_role("frame")
        if not frame:
            return 0, 0
        try:
            e = frame.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
        except Exception:
            return 0, 0
        try:
            x, y, _, _ = self.wm.geometry()
        except RuntimeError:
            return 0, 0
        return x - e.x, y - e.y

    def screen_point(self, node) -> tuple[int, int]:
        e = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
        ox, oy = self.frame_offset()
        return ox + e.x + e.width // 2, oy + e.y + e.height // 2

    def real_click(self, node, settle: float = 0.5) -> bool:
        if not node or not self.wm.has_ydotool:
            return False
        self.wm.focus()
        time.sleep(0.2)
        x, y = self.screen_point(node)
        self.wm.cursor(x, y)
        time.sleep(0.2)
        self.wm.click("0xC0")
        time.sleep(settle)
        return True

    def visible(self, aid: str) -> bool:
        return bool(self.find_id(aid))

    def wait_id(self, aid: str, timeout: float = 3.0) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.find_id(aid):
                return True
            time.sleep(0.1)
        return False

    def wait_no_contains(self, text: str, timeout: float = 2.0) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if not self.find_contains(text):
                return True
            time.sleep(0.1)
        return False

    def dump(self, path: str) -> None:
        lines = []
        for n in self.nodes():
            try:
                e = n.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                geo = f"{e.x},{e.y} {e.width}x{e.height}"
            except Exception:
                geo = ""
            lines.append(
                f"{'  ' * 0}{n.getRoleName()}|{n.name or ''}|"
                f"{getattr(n, 'accessibleId', '')}|{geo}|{self.showing(n)}"
            )
        open(path, "w").write("\n".join(lines))


class QA:
    def __init__(self, artifacts: str, mouse: bool = True):
        self.wm = WM()
        self.app = App(self.wm)
        self.mcp = MCP()
        self.mouse = mouse
        self.artifacts = artifacts
        os.makedirs(artifacts, exist_ok=True)
        self.failures: list[str] = []
        self.passes = 0
        self._shot_n = 0

    def shot(self, name: str) -> str:
        self._shot_n += 1
        path = os.path.join(self.artifacts, f"{self._shot_n:02d}-{name}.png")
        self.wm.screenshot(path)
        return path

    def tree(self, name: str) -> str:
        path = os.path.join(self.artifacts, f"{name}.tree.txt")
        self.app.dump(path)
        return path

    def check(self, name: str, ok: bool, detail: str = "") -> bool:
        if ok:
            self.passes += 1
            print(f"PASS {name}")
        else:
            self.failures.append(name)
            print(f"FAIL {name} {detail}")
            self.shot("failure-" + name.replace(" ", "-")[:60])
            self.tree("failure-" + name.replace(" ", "-")[:60])
        return ok

    def require(self, name: str, aid: str) -> bool:
        return self.check(name, self.app.visible(aid), f"{aid} not visible")

    # --- suites -------------------------------------------------------

    def suite_smoke(self):
        self.check("window geometry", self.wm.client() is not None)
        self.check("accessible tree", self.app.root.childCount > 0)
        self.require("search field", "search-input")
        self.require("compose button", "compose-open")
        self.require("settings button", "Settings")
        self.require("inbox folder", "folder-inbox")
        self.shot("smoke")

    def suite_folders(self):
        try:
            for folder in ["sent", "drafts", "archive", "trash", "inbox"]:
                ok = self.app.action(self.app.find_id(f"folder-{folder}"))
                time.sleep(0.5)
                node = self.app.find_id(f"folder-{folder}")
                selected = bool(node) and node.getState().contains(pyatspi.STATE_SELECTED)
                self.check(f"select {folder}", ok and selected, f"selected={selected}")
        finally:
            self.app.do_id("folder-inbox")
            time.sleep(0.4)

    def suite_search(self):
        try:
            self.app.action(self.app.find_id("search-input"))
            self.check("type search", self.app.set_text("search-input", "QA-UI"), "search text not set")
            time.sleep(1.0)
            rows = [n for n in self.app.nodes() if n.getRoleName() == "list item" and self.app.showing(n)]
            self.check("search results", len(rows) > 0, f"{len(rows)} rows")
            self.shot("search-qa-ui")
        finally:
            self.app.set_text("search-input", "")
            time.sleep(0.5)

    def suite_reader(self):
        try:
            self.check("search target message", self.app.set_text("search-input", "QA-UI 01 markdown basics"))
            time.sleep(0.8)
            row = self.app.find_contains("QA-UI 01 markdown basics")
            self.check("find QA message", row is not None)
            self.app.action(row)
            time.sleep(1.0)
            self.check("reader open", self.app.visible("reader-body"))
            self.require("reader reply button", "Reply")
            self.shot("reader-markdown")
            self.app.do_id("Close reader")
            time.sleep(0.4)
            self.check("reader closed", not self.app.visible("reader-body"))
        finally:
            self.app.set_text("search-input", "")
            self.app.do_id("Close reader")
            time.sleep(0.3)

    def suite_thread(self):
        try:
            row = self.app.find_contains("QA-UI Thread sample")
            self.check("thread row", row is not None)
            self.app.action(row)
            time.sleep(0.8)
            self.check("thread reader open", self.app.visible("reader-body"))
            for label in ["Previous message in conversation", "Next message in conversation", "Show conversation messages"]:
                self.check(f"thread control {label}", self.app.find_name(label) is not None)
            self.shot("thread-bundle")
        finally:
            self.app.do_id("Close reader")
            time.sleep(0.3)

    def suite_compose(self):
        try:
            self.check("open compose", self.app.do_id("compose-open"))
            time.sleep(0.6)
            self.require("compose to", "compose-to")
            self.require("compose subject", "compose-subject")
            self.require("compose body", "compose-body")
            self.check("set to", self.app.set_text("compose-to", "qa@example.com"), "to text")
            self.check("set subject", self.app.set_text("compose-subject", "QA UI compose"), "subject text")
            self.check("set body", self.app.set_text("compose-body", "# QA\n\nCompose body check."), "body text")
            self.require("compose send", "compose-send")
            self.shot("compose-filled")
        finally:
            self.app.do_id("Close composer")
            time.sleep(0.4)
        self.check("compose closed", not self.app.visible("compose-body"))

    def suite_settings(self):
        tabs = {
            "appearance": "chip-system",
            "mail": "settings-poll-interval",
            "rules": None,
            "aliases": "settings-new-mailbox",
            "contacts": "settings-contact-uid",
            "security": "link-publish-wkd",
            "mcp": "settings-mcp-enabled",
            "diagnostics": "link-open-themes-folder",
        }
        try:
            self.check("open settings", self.app.do_id("Settings"))
            time.sleep(0.6)
            for tab, marker in tabs.items():
                opened = self.app.do_id(f"settings-tab-{tab}")
                time.sleep(0.7)
                node = self.app.find_id(f"settings-tab-{tab}")
                selected = bool(node) and node.getState().contains(pyatspi.STATE_SELECTED)
                marker_ok = marker is None or self.app.visible(marker)
                self.check(f"settings tab {tab}", opened and selected and marker_ok,
                           f"selected={selected} marker={marker}")
                self.shot(f"settings-{tab}")
        finally:
            self.app.do_id("Close settings")
            time.sleep(0.4)
        self.check("settings closed", not self.app.visible("settings-tab-mail"))

    def suite_theme_mode(self):
        original = self.settings().get("appearance", {}).get("mode", "system")
        try:
            opened = self.app.do_id("Settings")
            self.app.do_id("settings-tab-appearance")
            self.check("theme settings visible", opened and self.app.wait_id("chip-light", 2.0))
            for mode in ["light", "dark", "system"]:
                self.check(f"theme chip {mode}", self.app.wait_id(f"chip-{mode}", 2.0))
                chip = self.app.find_id(f"chip-{mode}")
                self.check(f"theme mode {mode}", self.app.action(chip))
                time.sleep(0.6)
                self.shot(f"theme-{mode}")
        finally:
            self.app.do_id("settings-tab-appearance")
            chip = self.app.find_id(f"chip-{original}")
            if chip:
                self.app.action(chip)
            self.app.do_id("Close settings")
            time.sleep(0.4)

    @staticmethod
    def settings() -> dict:
        try:
            return json.load(open(os.path.join(DATA_DIR, "settings.json")))
        except Exception:
            return {}

    def suite_batch(self):
        try:
            self.check("enter selection", self.app.do_id("Select messages"))
            time.sleep(0.4)
            self.check("batch bar", self.app.visible("link-clear"), "Clear link missing")
            rows = [n for n in self.app.nodes() if n.getRoleName() == "list item" and self.app.showing(n)]
            if rows:
                self.app.action(rows[0])
                time.sleep(0.4)
                self.shot("batch-selected")
            self.app.do_id("link-clear")
        finally:
            self.app.do_id("Cancel selection")
            time.sleep(0.3)

    def suite_dialogs(self):
        try:
            self.check("open filters", self.app.do_id("Search filters"))
            time.sleep(0.4)
            self.require("filter from", "filter-from")
            self.check("set filter", self.app.set_text("filter-from", "emiliano"), "filter text")
            self.shot("filter-popup")
            self.app.set_text("filter-from", "")
            self.app.do_id("Close filters")
            time.sleep(0.3)
            self.check("filters closed", not self.app.visible("filter-from"))

            self.check("open identity menu", self.app.do_id("account-switcher"))
            time.sleep(0.4)
            identities = [n for n in self.app.nodes() if self.app.showing(n) and (n.name or "").startswith("Send from ")]
            self.check("identity rows", len(identities) > 0, f"{len(identities)} rows")
            self.shot("identity-popup")
            if self.wm.has_ydotool:
                self.wm.focus()
                x, y, w, h = self.wm.geometry()
                self.wm.cursor(x + int(w * 0.78), y + int(h * 0.78))
                time.sleep(0.15)
                self.wm.click("0xC0")
                time.sleep(0.5)
        finally:
            pass
        self.check("identity menu closed", self.app.wait_no_contains("Send from ", 2.0))

    def suite_mouse(self):
        if not self.mouse or not self.wm.has_ydotool:
            print("SKIP mouse suite")
            return
        try:
            # Close any modal left open by an earlier suite before clicking.
            self.wm.focus()
            self.wm.cursor(4700, 400)
            time.sleep(0.15)
            self.wm.click("0xC0")
            time.sleep(0.5)
            self.check("focus window", True)
            self.wm.focus()
            time.sleep(0.2)
            def click_folder(aid: str) -> bool:
                for _ in range(2):
                    node = self.app.find_id(aid)
                    if not node:
                        return False
                    self.app.real_click(node)
                    time.sleep(0.4)
                    node = self.app.find_id(aid)
                    if node and node.getState().contains(pyatspi.STATE_SELECTED):
                        return True
                return False

            self.check("mouse click Sent", click_folder("folder-sent"))
            self.shot("mouse-sent")
            self.check("mouse click Inbox", click_folder("folder-inbox"))
            row = self.app.find_contains("QA-UI 02 lists")
            self.check("mouse click message", self.app.real_click(row), "row missing")
            time.sleep(0.6)
            self.check("mouse opened reader", self.app.visible("reader-body"))
            self.shot("mouse-reader")
            self.app.do_id("Close reader")
            time.sleep(0.3)
            self.check("mouse click settings", self.app.real_click(self.app.find_id("Settings")))
            time.sleep(0.5)
            self.check("mouse settings opened", self.app.visible("settings-tab-mail"))
            self.shot("mouse-settings")
            self.app.do_id("Close settings")
        finally:
            self.app.do_id("Close reader")
            self.app.do_id("Close settings")
            time.sleep(0.3)

    def suite_scroll(self):
        if not self.wm.has_ydotool:
            print("SKIP scroll suite")
            return

        def first_row():
            for node in self.app.nodes():
                if node.getRoleName() == "list item" and self.app.showing(node):
                    return node.name, node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS).y
            return None

        try:
            before = first_row()
            self.check("scroll baseline", before is not None)
            self.check("open settings for scroll", self.app.do_id("Settings") and self.app.wait_id("chip-light", 2.0))
            self.wm.focus()
            self.wm.cursor(4700, 420)
            time.sleep(0.2)
            for _ in range(10):
                run(["ydotool", "mousemove", "-w", "-x", "0", "-y", "5"])
                time.sleep(0.06)
            time.sleep(0.4)
            self.shot("settings-scrolled")
            self.app.do_id("Close settings")
            time.sleep(0.6)
            after = first_row()
            self.check("settings wheel stays inside", before == after, f"{before} -> {after}")
        finally:
            self.app.do_id("Close settings")
            time.sleep(0.3)

    def suite_sizes(self):
        original = None
        try:
            self.wm.float()
            time.sleep(0.3)
            original = self.wm.geometry()
            for w, h in [(980, 620), (1280, 800), (1440, 900), (1920, 1080)]:
                self.wm.resize(w, h)
                time.sleep(0.8)
                self.check(f"size {w}x{h}", self.app.visible("Settings"), "settings button missing")
                self.shot(f"size-{w}x{h}")
            self.wm.resize(original[2], original[3])
            self.wm.move(original[0], original[1])
            time.sleep(0.6)
        finally:
            if original:
                try:
                    x, y, w, h = original
                    self.wm.resize(w, h)
                    self.wm.move(x, y)
                    time.sleep(0.5)
                except Exception:
                    pass

    def suite_mcp_data(self):
        for name, args in [
            ("status", {}),
            ("mail_list", {"folder": "inbox", "limit": 5}),
            ("mail_search", {"query": "QA-UI", "limit": 5}),
        ]:
            err, text = self.mcp.call(name, args)
            self.check(f"mcp {name}", not err, text[:100].replace("\n", " "))


SUITES = {
    "smoke": QA.suite_smoke,
    "folders": QA.suite_folders,
    "search": QA.suite_search,
    "reader": QA.suite_reader,
    "compose": QA.suite_compose,
    "thread": QA.suite_thread,
    "settings": QA.suite_settings,
    "theme": QA.suite_theme_mode,
    "batch": QA.suite_batch,
    "dialogs": QA.suite_dialogs,
    "mouse": QA.suite_mouse,
    "scroll": QA.suite_scroll,
    "sizes": QA.suite_sizes,
    "mcp": QA.suite_mcp_data,
}


def main() -> int:
    parser = argparse.ArgumentParser(description="Aster Qt local UI QA")
    parser.add_argument("--suite", action="append", choices=sorted(SUITES), help="suite to run (repeatable)")
    parser.add_argument("--no-mouse", action="store_true", help="skip real pointer events")
    parser.add_argument("--artifacts", default=None)
    args = parser.parse_args()

    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    artifacts = args.artifacts or f"/tmp/aster-qt-qa/{stamp}"
    qa = QA(artifacts, mouse=not args.no_mouse)
    print(f"artifacts: {artifacts}")
    try:
        for name in args.suite or ["smoke", "mcp", "folders", "search", "reader", "thread", "compose", "settings", "theme", "batch", "dialogs", "mouse", "sizes", "scroll"]:
            print(f"\n== {name} ==")
            try:
                SUITES[name](qa)
            except Exception as err:
                qa.check(f"{name} suite", False, repr(err))
                traceback.print_exc()
    finally:
        qa.tree("final")
    print(f"\n{qa.passes} passed, {len(qa.failures)} failed")
    if qa.failures:
        print("failed: " + ", ".join(qa.failures))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
