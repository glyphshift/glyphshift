"""Inspect freshly built engine DLLs and package them with the pinned SDK."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import uuid


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run(tool, *args):
    return subprocess.check_output([str(tool), *map(str, args)],
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)


def package(root, target_root, profile):
    spec = json.loads((root / "plugin.json").read_text(encoding="utf-8"))
    output = root / "local-test/evidence/packages"
    output.mkdir(parents=True, exist_ok=True)
    version = ".".join(map(str, spec["version"]))
    destination = output / f"{spec['package_id']}-{version}-{profile}.gsp"
    tool = target_root / f"x86_64-pc-windows-msvc/{profile}/glyphshift-adapter-tool.exe"
    with tempfile.TemporaryDirectory(dir=output) as directory:
        stage = Path(directory)
        files, variants = [], []

        def add(name, data, role):
            path = stage / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            files.append(dict(path=name, sha256=sha(data), size=len(data), role=role))

        add("license.txt", (root / "LICENSE").read_bytes(), "license")
        add("retour-license.txt", (root / ".sdk/vendor/retour/LICENSE").read_bytes(), "license")
        for architecture, target in [("x86_64", "x86_64-pc-windows-msvc"), ("x86", "i686-pc-windows-msvc")]:
            binary_root = target_root / target / profile
            adapters = []
            for entry in spec["adapters"]:
                if architecture not in entry["architectures"]:
                    continue
                dll = binary_root / entry["dll"]
                data = dll.read_bytes()
                metadata = json.loads(run(binary_root / "glyphshift-adapter-tool.exe", "metadata", dll, "--trusted-sha256", sha(data)))
                if metadata["adapter_id"] != entry["id"]:
                    raise ValueError("Native Adapter ID differs from package specification")
                name = f"windows-{architecture}/{entry['id']}.dll"
                add(name, data, "native_adapter")
                adapter = {k: entry[k] for k in ("name", "summary", "technology", "process_resident_after_deactivate")}
                adapters.append(dict(adapter, file=name, native_metadata=metadata))
            if adapters:
                variants.append(dict(platform="windows", architecture=architecture, adapters=adapters))
        for notice in spec["notices"]:
            add(notice["name"], (target_root / notice["path"].replace("{profile}", profile)).read_bytes(), "license")
        manifest = dict(schema="glyphshift.plugin/1", package_id=spec["package_id"], version=spec["version"],
                        runtime_bundle_schema="glyphshift.runtime-bundle/4", license_file="license.txt", variants=variants, files=files)
        (stage / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
        candidate = output / f"candidate-{uuid.uuid4().hex}.gsp"
        try:
            report = json.loads(run(tool, "pack", stage / "manifest.json", stage, candidate))
            run(tool, "verify", candidate)
            if destination.exists():
                if sha(destination.read_bytes()) != report["sha256"]:
                    raise ValueError("Package version already has different bytes; increment plugin.json version")
            else:
                candidate.rename(destination)
            report.update(package=destination.name, sdk=json.loads((root / "sdk.lock.json").read_text()),
                          adapter_ids=[a["id"] for a in spec["adapters"]], profile=profile)
            (output / f"{destination.name}.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(report, indent=2))
        finally:
            candidate.unlink(missing_ok=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-root", type=Path, required=True)
    parser.add_argument("--profile", choices=["debug", "release"], required=True)
    args = parser.parse_args()
    package(Path(__file__).resolve().parents[1], args.target_root, args.profile)
