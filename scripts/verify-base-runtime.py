"""Check installer Runtime contents; native compatibility uses the production loader."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import stat


# Product boundary, deliberately independent of the configurable package catalog.
CORE_IDS = frozenset({
    "windows.gdi.ext-text-out", "windows.gdi.text-out", "windows.user32.draw-text",
    "windows.gdiplus.draw-string", "windows.directwrite.text-layout",
})


def regular_file(path):
    metadata = path.lstat()
    if (not stat.S_ISREG(metadata.st_mode) or path.is_symlink()
            or getattr(metadata, "st_file_attributes", 0) & 0x400):
        raise ValueError("Base Runtime must contain only regular files")


def verify(bundle):
    bundle = Path(bundle).resolve(strict=True)
    manifest_path = bundle / "runtime-bundle.json"
    regular_file(manifest_path)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if manifest["schema"] != "glyphshift.runtime-bundle/4":
        raise ValueError("Base Runtime requires schema v4")
    groups = [manifest, *manifest.get("additional_architectures", [])]
    if len(groups) != 2 or {g["architecture"] for g in groups} != {"x86", "x86_64"}:
        raise ValueError("Base Runtime requires exactly x86 and x86_64")
    files = {"runtime-bundle.json"}
    for group in groups:
        if group.get("isolated_workers") or group.get("acquisition_workers"):
            raise ValueError("Base Runtime cannot include workers")
        ids = [a["native_metadata"]["adapter_id"] for a in group["adapters"]]
        if len(ids) != len(CORE_IDS) or set(ids) != CORE_IDS:
            raise ValueError("Base Runtime must contain exactly the five Windows text adapters")
        for artifact in [group["controller"], group["runtime"], *group["adapters"]]:
            name = artifact["file"]
            if not isinstance(name, str) or not re.fullmatch(r"[a-z0-9_-]+\.(dll|exe)", name):
                raise ValueError("Unsafe base Runtime artifact name")
            if name in files:
                raise ValueError("Duplicate base Runtime artifact")
            files.add(name)
            path = bundle / name
            regular_file(path)
            with path.open("rb") as stream:
                actual_hash = hashlib.file_digest(stream, "sha256").hexdigest()
            if actual_hash != artifact["sha256"]:
                raise ValueError("Base Runtime artifact checksum mismatch")
    # Tauri copies the whole directory: unreferenced DLLs/packages also violate the boundary.
    if {p.name for p in bundle.iterdir()} != files:
        raise ValueError("Base Runtime contains undeclared files or directories")
    return len(files)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    args = parser.parse_args()
    try:
        count = verify(args.bundle)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"Base Runtime distribution rejected: {error}\n")
    print(f"Base Runtime verified: 5 Windows adapters per architecture, {count} files, no engine payloads")


if __name__ == "__main__":
    main()
