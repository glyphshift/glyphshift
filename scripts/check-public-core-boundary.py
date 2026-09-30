"""Reject concrete Adapter implementation material accidentally tracked by Public Core."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]

FORBIDDEN_PREFIXES = (
    "research/",
    "crates/adapters/implementations/",
    "test-support/catsystem2-native/",
    "test-support/coreclr-late-attach/",
    "test-support/kag-bridge-contract/",
    "test-support/kag-parser-contract/",
    "test-support/kag-runtime-host/",
    "test-support/monogame-text/",
    "test-support/native-framework-abi/",
    "test-support/renpy-text/",
    "test-support/rpgmaker-mv-text/",
    "test-support/tyranoscript-text/",
    "test-support/unity-il2cpp-custom-text-negative/",
    "test-support/vgui-localize-lifecycle/",
    "test-support/vgui-localize-probe/",
    "test-support/webview2-dom-host/",
)

FORBIDDEN_FILES = {
    "adapter-sources.lock.json",
    "apps/glyphshift-desktop/tests/catsystem2.spec.ts",
    "crates/adapters/platform/native-host/tests/coreclr_extension_fixture.rs",
    "crates/runtime/targets/runtime/examples/mv_deployment.rs",
    "scripts/build-adapter-packages.ps1",
    "scripts/extract-adapter-repositories.py",
    "scripts/package-adapters.py",
    "scripts/sync-adapter-snapshots.py",
    "scripts/sync-raylib-snapshot.py",
    "scripts/test-adapter-snapshots.py",
    "scripts/test-webview2-dom-native.ps1",
    "test-support/windows-host/src/windows/ocr.rs",
    "test-support/windows-host/src/windows/uia.rs",
}


def verify_core_presentations() -> list[str]:
    distribution = json.loads(
        (ROOT / "scripts/adapter-distribution.json").read_text(encoding="utf-8")
    )
    presentations = json.loads(
        (ROOT / "scripts/runtime-bundle-adapters.zh-CN.json").read_text(encoding="utf-8")
    )
    expected = distribution["core"]
    actual = [entry["id"] for entry in presentations]
    if actual == expected:
        return []
    return [
        "scripts/runtime-bundle-adapters.zh-CN.json must contain exactly the five "
        f"bundled base Adapter presentations (expected {expected!r}, got {actual!r})"
    ]


def main() -> int:
    tracked = subprocess.check_output(
        ["git", "-C", str(ROOT), "ls-files"], text=True, encoding="utf-8"
    ).splitlines()
    violations = sorted(
        path
        for path in tracked
        if path in FORBIDDEN_FILES or path.startswith(FORBIDDEN_PREFIXES)
    )
    messages = [f"tracked private material: {path}" for path in violations]
    messages.extend(verify_core_presentations())
    if messages:
        print("Public Core boundary violation:", file=sys.stderr)
        for message in messages:
            print(f"  {message}", file=sys.stderr)
        return 1
    print("Public Core source boundary verified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
