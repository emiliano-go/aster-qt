#!/usr/bin/env python3
"""Block personal settings and private key material from entering the repo."""
import os
import re
import subprocess
import sys
from pathlib import Path

PRIVATE_KEY = re.compile(
    rb"-----BEGIN (?:PGP PRIVATE KEY(?: BLOCK)?|RSA PRIVATE KEY|OPENSSH PRIVATE KEY|EC PRIVATE KEY|DSA PRIVATE KEY)-----"
    rb"[\r\n]+[A-Za-z0-9+/=\r\n]{40,}"
)
def personal_markers() -> list[bytes]:
    markers: list[bytes] = []
    env = os.environ.get("ASTER_PERSONAL_MARKERS", "")
    markers.extend(part.strip().encode() for part in env.split(",") if part.strip())
    local = Path.home() / ".config" / "aster-qt" / "personal-markers"
    if local.exists():
        markers.extend(
            line.strip().encode()
            for line in local.read_text().splitlines()
            if line.strip() and not line.strip().startswith("#")
        )
    return markers


BLOCKED_NAMES = re.compile(
    r"(^|/)(settings\.json|mcp\.json|.*\.asc|.*\.gpg|.*\.key|.*\.pem|.*\.p12|.*\.pfx)$",
    re.IGNORECASE,
)
BLOCKED_DIRS = (".gnupg/", "private-keys-v1.d/")


def tracked_files() -> list[str]:
    return subprocess.check_output(["git", "ls-files"], text=True).splitlines()


def staged_files() -> list[str]:
    output = subprocess.check_output(
        ["git", "diff", "--cached", "--name-only", "--diff-filter=ACM"],
        text=True,
    )
    return [line for line in output.splitlines() if line]


def content(path: str, staged: bool) -> bytes:
    if not staged:
        try:
            return Path(path).read_bytes()
        except OSError:
            return b""
    try:
        return subprocess.check_output(["git", "show", f":{path}"])
    except subprocess.CalledProcessError:
        return b""


def check(files: list[str], staged: bool) -> int:
    failures = 0
    markers = personal_markers()
    for path in files:
        if BLOCKED_NAMES.search(path) or any(part in path for part in BLOCKED_DIRS):
            print(f"BLOCKED file name: {path}")
            failures += 1
            continue
        data = content(path, staged)
        if PRIVATE_KEY.search(data):
            print(f"BLOCKED private key material: {path}")
            failures += 1
        for marker in markers:
            if marker in data:
                print(f"BLOCKED personal marker {marker.decode()}: {path}")
                failures += 1
    return failures


def main() -> int:
    all_mode = sys.argv[1:] == ["--all"]
    files = tracked_files() if all_mode else staged_files()
    failures = check(files, staged=not all_mode)
    if failures:
        print(f"\nSecret scan failed with {failures} issue(s). Nothing was changed.")
        return 1
    print(f"Secret scan passed for {len(files)} file(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
