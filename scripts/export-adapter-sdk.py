"""Export a reproducible, source-only SDK from a committed main-repository revision."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import zipfile

MEMBERS = [
    "crates/core/domain",
    "crates/adapters/platform/sdk",
    "crates/adapters/platform/native-abi",
    "crates/adapters/platform/native-host",
    "crates/adapters/platform/package",
    "crates/adapters/tools/devkit",
]


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(root), *args])


def strip_dev_dependencies(text: str) -> str:
    """The host's product-wide tests are not part of the developer SDK graph."""
    chunks = re.split(r"(?m)(?=^\[)", text)
    return "".join(part for part in chunks if "dev-dependencies" not in part.partition("\n")[0])


def export(root: Path, revision: str, output: Path) -> dict:
    revision = git(root, "rev-parse", "--verify", revision + "^{commit}").decode().strip()
    tree = git(root, "ls-tree", "-r", "--name-only", revision).decode().splitlines()
    payload: dict[str, bytes] = {}
    for name in tree:
        selected = any(name == f"{m}/Cargo.toml" or name.startswith(f"{m}/src/") or name.startswith(f"{m}/include/") for m in MEMBERS)
        selected |= name.startswith("vendor/retour/") and not name.endswith(".orig")
        selected |= name in ("LICENSE", "rust-toolchain.toml", "Cargo.lock", "docs/plugins.md")
        if selected:
            content = git(root, "show", f"{revision}:{name}")
            if name.endswith("Cargo.toml") and not name.startswith("vendor/"):
                content = strip_dev_dependencies(content.decode()).encode()
            payload[name] = content
    for member in MEMBERS:
        if f"{member}/Cargo.toml" not in payload:
            raise ValueError(f"missing committed SDK member: {member}")
    original = tomllib.loads(git(root, "show", f"{revision}:Cargo.toml").decode())
    version = original["workspace"]["package"]["version"]
    workspace = "[workspace]\nmembers = " + json.dumps(MEMBERS) + '\nexclude = ["vendor/retour"]\nresolver = "2"\n\n'
    workspace += '[workspace.package]\n' + "\n".join(f"{k} = {json.dumps(v)}" for k,v in original["workspace"]["package"].items()) + "\n\n"
    workspace += '[workspace.lints.rust]\nunsafe_code = "forbid"\n\n[patch.crates-io]\nretour = { path = "vendor/retour" }\n'
    payload["Cargo.toml"] = workspace.encode()
    payload["README.md"] = ("# Glyphshift Adapter SDK\n\nSource-only developer kit; not an end-user Runtime or GSP.\n"
        "Contains Domain, SDK, Native ABI headers, native inspection, GSP packaging, and the reviewed retour patch.\n"
        "Build glyphshift-adapter-tool for each target architecture to inspect locally built DLLs.\n"
        "Use Cargo.lock and --locked. Native metadata inspection requires explicit digest approval.\n"
        "No desktop/controller/target Runtime or official paid Adapter implementation is included.\n"
        "Third-party retour source retains its BSD license; Glyphshift source uses the included LICENSE.\n").encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    # Resolve only this allowlisted graph, seeded with the committed lockfile.
    with tempfile.TemporaryDirectory(dir=output.parent, prefix="sdk-export-") as temp:
        stage = Path(temp)
        for name, data in payload.items():
            destination = stage / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        subprocess.run(["cargo", "metadata", "--manifest-path", str(stage / "Cargo.toml"), "--offline", "--format-version", "1"], check=True, stdout=subprocess.DEVNULL)
        payload["Cargo.lock"] = (stage / "Cargo.lock").read_bytes()
    release = {
        "schema": "glyphshift.adapter-sdk/1", "version": version,
        "source_repository": "https://github.com/glyphshift/glyphshift", "source_commit": revision,
        "native_abi": [1, 0], "gsp_schema": "glyphshift.plugin/1",
        "files": {name: hashlib.sha256(data).hexdigest() for name,data in sorted(payload.items())},
    }
    payload["sdk-release.json"] = (json.dumps(release, indent=2) + "\n").encode()
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data in sorted(payload.items()):
            entry = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            entry.external_attr = 0o100644 << 16
            archive.writestr(entry, data, compresslevel=9)
    data = stream.getvalue()
    with output.open("xb") as file:
        file.write(data)
    return {"version": version, "source_commit": revision, "sha256": hashlib.sha256(data).hexdigest(), "bytes":len(data), "files":len(payload)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ref", default="HEAD")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(export(Path(__file__).resolve().parents[1], args.ref, args.output.resolve()), indent=2))
