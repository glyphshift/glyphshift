"""Fetch and verify the pinned public base Adapter GSP packages."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tempfile
import urllib.request
import zipfile


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOCK = ROOT / "scripts/base-adapters.lock.json"
MAX_PACKAGE_BYTES = 16 * 1024 * 1024
MAX_ENTRY_BYTES = 8 * 1024 * 1024
MAX_ARCHIVE_BYTES = 32 * 1024 * 1024
ALLOWED_REPOSITORY_PREFIX = "https://github.com/glyphshift/adapter-"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_package(url: str, cache_path: Path) -> bytes:
    if cache_path.is_file():
        return cache_path.read_bytes()
    if not url.startswith(ALLOWED_REPOSITORY_PREFIX) or "/releases/download/" not in url:
        raise ValueError(f"unsupported base Adapter URL: {url}")
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url, timeout=60) as response:
        length = response.headers.get("Content-Length")
        if length is not None and int(length) > MAX_PACKAGE_BYTES:
            raise ValueError("base Adapter package exceeds download limit")
        data = response.read(MAX_PACKAGE_BYTES + 1)
    if len(data) > MAX_PACKAGE_BYTES:
        raise ValueError("base Adapter package exceeds download limit")
    with tempfile.NamedTemporaryFile(dir=cache_path.parent, delete=False) as handle:
        handle.write(data)
        temporary = Path(handle.name)
    temporary.replace(cache_path)
    return data


def safe_archive(archive: zipfile.ZipFile) -> dict[str, zipfile.ZipInfo]:
    entries: dict[str, zipfile.ZipInfo] = {}
    total = 0
    for info in archive.infolist():
        path = PurePosixPath(info.filename)
        if (
            info.is_dir()
            or path.is_absolute()
            or not info.filename
            or "\\" in info.filename
            or ":" in info.filename
            or any(part in ("", ".", "..") for part in path.parts)
        ):
            raise ValueError(f"unsafe GSP entry: {info.filename!r}")
        if info.file_size > MAX_ENTRY_BYTES:
            raise ValueError(f"GSP entry exceeds size limit: {info.filename}")
        total += info.file_size
        if total > MAX_ARCHIVE_BYTES:
            raise ValueError("GSP archive exceeds expanded size limit")
        key = info.filename.casefold()
        if key in entries:
            raise ValueError(f"duplicate GSP entry: {info.filename}")
        entries[key] = info
    return entries


def package_version(value: object) -> tuple[int, int, int]:
    if not isinstance(value, list) or len(value) != 3 or not all(isinstance(v, int) for v in value):
        raise ValueError("invalid package version")
    return tuple(value)


def extract_package(package: dict[str, object], data: bytes, output: Path) -> dict[str, object]:
    if digest(data) != package["sha256"]:
        raise ValueError(f"base Adapter digest mismatch: {package['package_id']}")
    with tempfile.NamedTemporaryFile(suffix=".gsp", delete=False) as handle:
        handle.write(data)
        temporary = Path(handle.name)
    try:
        with zipfile.ZipFile(temporary) as archive:
            entries = safe_archive(archive)
            manifest_info = entries.get("manifest.json")
            if manifest_info is None:
                raise ValueError("GSP manifest is missing")
            manifest = json.loads(archive.read(manifest_info))
            if manifest.get("schema") != "glyphshift.plugin/1":
                raise ValueError("unsupported GSP manifest schema")
            if manifest.get("package_id") != package["package_id"]:
                raise ValueError("GSP package ID mismatch")
            if package_version(manifest.get("version")) != package_version(package["version"]):
                raise ValueError("GSP package version mismatch")

            declared_files = {}
            for entry in manifest.get("files", []):
                if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
                    raise ValueError("invalid GSP file inventory")
                declared_files[entry["path"]] = entry

            expected = {entry["id"]: entry for entry in package["adapters"]}
            observed: set[str] = set()
            extracted = []
            for variant in manifest.get("variants", []):
                if variant.get("platform") != "windows":
                    continue
                architecture = variant.get("architecture")
                if architecture not in ("x86_64", "x86"):
                    raise ValueError(f"unexpected base Adapter architecture: {architecture!r}")
                for adapter in variant.get("adapters", []):
                    metadata = adapter.get("native_metadata", {})
                    adapter_id = metadata.get("adapter_id")
                    if adapter_id not in expected:
                        raise ValueError(f"unexpected base Adapter ID: {adapter_id!r}")
                    source_name = adapter.get("file")
                    inventory = declared_files.get(source_name)
                    archive_info = entries.get(str(source_name).casefold())
                    if inventory is None or archive_info is None:
                        raise ValueError(f"missing native Adapter payload: {adapter_id}")
                    payload = archive.read(archive_info)
                    if digest(payload) != inventory.get("sha256") or len(payload) != inventory.get("size"):
                        raise ValueError(f"native Adapter payload mismatch: {adapter_id}")
                    destination = output / architecture / expected[adapter_id]["file"]
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(payload)
                    observed.add(adapter_id)
                    extracted.append(
                        {
                            "id": adapter_id,
                            "architecture": architecture,
                            "file": str(destination),
                            "sha256": digest(payload),
                        }
                    )
            if observed != set(expected):
                raise ValueError(
                    f"incomplete base Adapter package {package['package_id']}: "
                    f"expected {sorted(expected)}, got {sorted(observed)}"
                )
            return {
                "package_id": package["package_id"],
                "version": package["version"],
                "sha256": package["sha256"],
                "adapters": extracted,
            }
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, default=DEFAULT_LOCK)
    parser.add_argument("--cache-root", type=Path, default=ROOT / "local-test/cache/base-adapters")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    lock = json.loads(args.lock.read_text(encoding="utf-8"))
    if lock.get("schema") != "glyphshift.base-adapters-lock/1":
        raise ValueError("unsupported base Adapter lock schema")
    packages = lock.get("packages")
    if not isinstance(packages, list) or not packages:
        raise ValueError("base Adapter lock contains no packages")

    args.output.mkdir(parents=True, exist_ok=True)
    reports = []
    for package in packages:
        if not isinstance(package, dict):
            raise ValueError("invalid base Adapter lock entry")
        package_id = package.get("package_id")
        sha256 = package.get("sha256")
        url = package.get("url")
        if not isinstance(package_id, str) or not isinstance(sha256, str) or len(sha256) != 64:
            raise ValueError("invalid base Adapter identity")
        if not isinstance(url, str):
            raise ValueError("invalid base Adapter URL")
        cache_path = args.cache_root / f"{package_id}-{'.'.join(map(str, package['version']))}.gsp"
        data = read_package(url, cache_path)
        if digest(data) != sha256:
            raise ValueError(f"cached base Adapter digest mismatch: {package_id}")
        reports.append(extract_package(package, data, args.output))

    report_path = args.output / "base-adapters.json"
    report_path.write_text(
        json.dumps(
            {"schema": "glyphshift.base-adapters-fetch/1", "packages": reports},
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(report_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
