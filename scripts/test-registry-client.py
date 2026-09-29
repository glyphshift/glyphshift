"""Real Go Registry -> verified HTTPS -> Rust installation; all keys/artifacts/evidence are local-only."""
import argparse
import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import queue
import ssl
import struct
import subprocess
import tempfile
import threading
import zipfile

ROOT = Path(__file__).resolve().parents[1]
FLAGS = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def synthetic_gsp(path):
    image = bytearray(90)
    image[:2] = b"MZ"
    struct.pack_into("<I", image, 60, 64)
    image[64:68] = b"PE\0\0"
    struct.pack_into("<H", image, 68, 0x8664)
    struct.pack_into("<H", image, 88, 0x20B)
    files = {"adapter.dll": bytes(image), "license.txt": b"synthetic fixture"}
    metadata = {"adapter_id": "synthetic.inline", "version": [1, 0, 0], "abi": [1, 0],
                "apply_model": 1, "placement": 1, "feature_bits": 3, "platform_bits": 1,
                "architecture_bits": 3, "source_policy": 0}
    manifest = {"schema": "glyphshift.plugin/1", "package_id": "glyphshift-adapter-synthetic", "version": [1, 0, 0],
                "runtime_bundle_schema": "glyphshift.runtime-bundle/4", "license_file": "license.txt",
                "files": [{"path": name, "sha256": hashlib.sha256(data).hexdigest(), "size": len(data),
                           "role": "license" if name.endswith(".txt") else "native_adapter"} for name, data in files.items()],
                "variants": [{"platform": "windows", "architecture": "x86_64", "adapters": [{"file": "adapter.dll",
                              "native_metadata": metadata, "name": "Synthetic fixture", "summary": "Not executable code",
                              "technology": "Synthetic", "process_resident_after_deactivate": True}]}]}
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_STORED) as archive:
        archive.writestr("manifest.json", json.dumps(manifest))
        for name, data in files.items():
            archive.writestr(name, data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--client", type=Path, required=True)
    parser.add_argument("--registry", type=Path, required=True)
    parser.add_argument("--inspector", type=Path, required=True)
    parser.add_argument("--adapter", type=Path)
    args = parser.parse_args()
    base = ROOT / "local-test/evidence/registry-client/runs"
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=base))
    environment = os.environ.copy()

    def run(executable, *options, fail=False, env=None):
        result = subprocess.run([str(executable), *map(str, options)], capture_output=True, creationflags=FLAGS,
                                timeout=120, env=env or environment)
        if fail:
            assert result.returncode != 0, "unexpected success"
            assert not result.stdout.strip(), "failure emitted success output"
            return None
        assert result.returncode == 0, result.stderr.decode(errors="replace")
        return json.loads(result.stdout) if result.stdout.strip() else None

    run("go", "run", ROOT / "scripts/registry-client-fixture.go", root)
    environment.update(REGISTRY_SIGNING_KEY_FILE=str(root / "signing.pem"), REGISTRY_TRUST_FILE=str(root / "trust.json"),
                       REGISTRY_SIGNING_KEY_ID="fixture-key", GLYPHSHIFT_REGISTRY_CA_FILE=str(root / "tls-cert.pem"))
    # Both servers are loopback-only and owned by this harness. Avoid unrelated proxy configuration for the fixture.
    environment["NO_PROXY"] = "localhost,127.0.0.1"
    environment["no_proxy"] = environment["NO_PROXY"]
    database = root / "registry.sqlite"
    checker_hash = hashlib.sha256(args.inspector.read_bytes()).hexdigest()

    def operator(command, *options):
        return run(args.registry.resolve(), command, "--db", database, *options)

    def publish(kind, file, key):
        inspected = subprocess.run([str(args.inspector.resolve()), kind], input=file.read_bytes(), capture_output=True,
                                   creationflags=FLAGS, timeout=45, check=True)
        report = json.loads(inspected.stdout)
        operator("claim", "--actor", "Zbcdef23", "--owner", "Abcdef23", "--kind", kind, "--package", report["packageId"])
        receipt = operator("submit", "--actor", "Abcdef23", "--kind", kind, "--input", file, "--key", key,
                           "--inspector", args.inspector.resolve(), "--inspector-sha256", checker_hash)
        operator("review", "--actor", "Zbcdef23", "--submission", receipt["id"], "--decision", "approve", "--reason", "synthetic cross-runtime test")
        return report

    for version in ["1.0.0", "1.0.1", "1.0.2"]:
        dictionary = root / f"dictionary-{version}.json"
        dictionary.write_text(json.dumps({"schema": "glyphshift.dictionary/3", "revision": 1,
            "metadata": {"id": "fixture.dictionary", "releaseVersion": version, "name": "Synthetic dictionary", "sourceLocale": "en-US", "targetLocale": "zh-CN"},
            "entries": [{"source": "Open", "translation": "打开"}]}, ensure_ascii=False), encoding="utf-8")
        publish("dictionary", dictionary, "cross-runtime-dictionary-" + version)
    adapter = args.adapter.resolve() if args.adapter else root / "synthetic.gsp"
    if args.adapter is None:
        synthetic_gsp(adapter)
    report = publish("adapter", adapter, "cross-runtime-adapter-001")

    backend = subprocess.Popen([str(args.registry.resolve()), "serve", "--db", str(database)], stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True, creationflags=FLAGS, env=environment)
    proxy = None
    proxy_thread = None
    try:
        started = queue.Queue()
        threading.Thread(target=lambda: started.put(backend.stdout.readline()), daemon=True).start()
        line = started.get(timeout=20).strip()
        assert line.startswith("Local read-only Registry: 127.0.0.1:"), line
        backend_port = int(line.rsplit(":", 1)[1])

        class Proxy(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                connection = http.client.HTTPConnection("127.0.0.1", backend_port, timeout=15)
                try:
                    connection.request("GET", self.path)
                    response = connection.getresponse()
                    body = response.read()
                    self.send_response(response.status)
                    for name, value in response.getheaders():
                        if name.lower() in ("content-type", "cache-control", "etag"):
                            self.send_header(name, value)
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                finally:
                    connection.close()

        proxy = ThreadingHTTPServer(("127.0.0.1", 0), Proxy)
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.minimum_version = ssl.TLSVersion.TLSv1_2
        tls.load_cert_chain(root / "tls-cert.pem", root / "tls-key.pem")
        proxy.socket = tls.wrap_socket(proxy.socket, server_side=True)
        proxy_thread = threading.Thread(target=proxy.serve_forever, daemon=True)
        proxy_thread.start()
        origin = f"https://127.0.0.1:{proxy.server_port}"
        client = args.client.resolve()
        trust = root / "trust.json"

        data_root = root / "dictionary-store"
        def install_dictionary(version, destination=data_root, fail=False):
            return run(client, "install-dictionary", origin, trust, "fixture.registry", "fixture.dictionary", version,
                       "Abcdef23", destination, "replace-verified", fail=fail)

        first = install_dictionary("1.0.0")
        second = install_dictionary("1.0.1")
        assert first["state"] == second["state"] == "Verified"
        installed = data_root / "dictionaries/fixture.dictionary.json"
        assert installed.read_bytes() == (root / "dictionary-1.0.1.json").read_bytes()
        edited = installed.read_text(encoding="utf-8").replace("打开", "本地编辑")
        installed.write_text(edited, encoding="utf-8")
        install_dictionary("1.0.2", fail=True)
        assert installed.read_text(encoding="utf-8") == edited

        plugin_store = root / "plugin-store"
        def install_adapter(destination, trusted=trust, fail=False, env=None):
            return run(client, "install-adapter", origin, trusted, report["packageId"], report["version"], "Abcdef23", destination, fail=fail, env=env)
        selection = install_adapter(plugin_store)
        assert selection["selected"] is False and selection["sha256"] == hashlib.sha256(adapter.read_bytes()).hexdigest()
        assert install_adapter(plugin_store) == selection  # idempotent immutable installation

        revoked = json.loads(trust.read_text(encoding="utf-8"))
        revoked["keys"][0]["state"] = "revoked"
        revoked_file = root / "revoked-trust.json"
        revoked_file.write_text(json.dumps(revoked), encoding="utf-8")
        denied = root / "must-not-install-revoked"
        install_adapter(denied, revoked_file, fail=True)
        assert not denied.exists()
        no_ca = environment.copy()
        no_ca.pop("GLYPHSHIFT_REGISTRY_CA_FILE")
        denied_tls = root / "must-not-install-untrusted-tls"
        install_adapter(denied_tls, fail=True, env=no_ca)
        assert not denied_tls.exists()

        operator("availability", "--actor", "Zbcdef23", "--kind", "adapter", "--package", report["packageId"],
                 "--version", report["version"], "--state", "quarantined", "--reason", "synthetic quarantine")
        denied_state = root / "must-not-install-quarantined"
        install_adapter(denied_state, fail=True)
        assert not denied_state.exists()
        result = {"status": "passed", "dictionaryInstallUpdate": True, "localChangesPreserved": True,
                  "adapterInstalledNotSelected": True, "idempotentInstall": True, "revokedKeyRejected": True,
                  "untrustedTLSRejected": True, "quarantinedReleaseRejected": True,
                  "adapterBytes": adapter.stat().st_size, "adapterSHA256": selection["sha256"]}
        (root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result, indent=2))
    finally:
        if proxy is not None:
            proxy.shutdown()
            proxy.server_close()
        if proxy_thread is not None:
            proxy_thread.join(timeout=5)
        backend.terminate()
        try:
            backend.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            backend.kill()
            backend.communicate(timeout=5)


if __name__ == "__main__":
    main()
