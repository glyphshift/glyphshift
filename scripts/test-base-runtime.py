"""Synthetic installer boundary tests; never load or execute fixture artifacts."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location("base_runtime", Path(__file__).with_name("verify-base-runtime.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


class BaseRuntimeTests(unittest.TestCase):
    def setUp(self):
        root = Path(__file__).resolve().parents[1] / "local-test/evidence/base-runtime-contract"
        root.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.temp.cleanup)
        self.bundle = Path(self.temp.name)
        groups = []
        for architecture in ["x86_64", "x86"]:
            def artifact(stem, extension="dll"):
                name = f"synthetic-{stem}-{architecture}.{extension}"
                data = name.encode()
                (self.bundle / name).write_bytes(data)
                return {"file": name, "sha256": hashlib.sha256(data).hexdigest()}
            adapters = []
            for index, adapter_id in enumerate(sorted(base.CORE_IDS)):
                adapters.append({**artifact(str(index)), "native_metadata": {"adapter_id": adapter_id}})
            groups.append({"architecture": architecture, "adapters": adapters,
                           "controller": artifact("controller", "exe"), "runtime": artifact("runtime")})
        self.manifest = {**groups[0], "schema": "glyphshift.runtime-bundle/4", "additional_architectures": [groups[1]]}

    def verify(self):
        (self.bundle / "runtime-bundle.json").write_text(json.dumps(self.manifest), encoding="utf-8")
        return base.verify(self.bundle)

    def test_complete_base_has_only_fifteen_files(self):
        self.assertEqual(self.verify(), 15)

    def test_engine_cannot_replace_or_extend_core(self):
        for adapter_id in ["windows.qt.painter-draw-text", "windows.sidefx.cv-paint-buffer-text", "synthetic.unreal.text"]:
            with self.subTest(adapter=adapter_id):
                self.manifest["adapters"][0]["native_metadata"]["adapter_id"] = adapter_id
                with self.assertRaisesRegex(ValueError, "five Windows"):
                    self.verify()
        self.manifest["adapters"][0]["native_metadata"]["adapter_id"] = sorted(base.CORE_IDS)[0]
        self.manifest["adapters"].append(self.manifest["adapters"][0].copy())
        with self.assertRaisesRegex(ValueError, "five Windows"):
            self.verify()

    def test_unsupported_manifest_schema_is_rejected(self):
        self.manifest["schema"] = "glyphshift.runtime-bundle/3"
        with self.assertRaisesRegex(ValueError, "schema v4"):
            self.verify()

    def test_extra_payload_or_directory_is_rejected(self):
        for name in ["synthetic-engine.dll", "synthetic.gsp", "test-target-x86.exe", "plugins"]:
            with self.subTest(name=name):
                path = self.bundle / name
                path.mkdir() if name == "plugins" else path.write_bytes(b"extra")
                with self.assertRaisesRegex(ValueError, "undeclared"):
                    self.verify()
                path.rmdir() if path.is_dir() else path.unlink()

    def test_missing_or_duplicate_architecture_is_rejected(self):
        self.manifest["additional_architectures"][0]["architecture"] = "x86_64"
        with self.assertRaisesRegex(ValueError, "exactly x86"):
            self.verify()
        self.manifest["additional_architectures"] = []
        with self.assertRaisesRegex(ValueError, "exactly x86"):
            self.verify()

    def test_secondary_architecture_cannot_hide_engine_or_worker(self):
        secondary = self.manifest["additional_architectures"][0]
        secondary["isolated_workers"] = [{"file": "synthetic.exe"}]
        with self.assertRaisesRegex(ValueError, "workers"):
            self.verify()
        secondary["isolated_workers"] = []
        secondary["adapters"].pop()
        with self.assertRaisesRegex(ValueError, "five Windows"):
            self.verify()

    def test_changed_or_missing_binary_is_rejected(self):
        path = self.bundle / self.manifest["runtime"]["file"]
        path.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum"):
            self.verify()
        path.unlink()
        with self.assertRaises(OSError):
            self.verify()

    def test_unsafe_and_reused_paths_are_rejected(self):
        self.manifest["runtime"]["file"] = "../synthetic.dll"
        with self.assertRaisesRegex(ValueError, "Unsafe"):
            self.verify()
        self.manifest["runtime"] = self.manifest["controller"].copy()
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            self.verify()


if __name__ == "__main__":
    unittest.main()
