use crate::TrustStore;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

pub(crate) struct Fixture {
    pub key: SigningKey,
    pub body: Vec<u8>,
}
impl Fixture {
    pub fn dictionary(version: &str) -> Self {
        let body = serde_json::to_vec(&json!({"schema":"glyphshift.dictionary/3","revision":1,
            "metadata":{"id":"fixture.dictionary","releaseVersion":version,"name":"Synthetic dictionary","sourceLocale":"en-US","targetLocale":"zh-CN"},
            "entries":[{"source":"Open","translation":"打开"}]})).unwrap();
        Self {
            key: SigningKey::from_bytes(&[11; 32]),
            body,
        }
    }
    pub fn trust_bytes(&self, state: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({"schema":"glyphshift.release-trust/1","issuer":"urn:glyphshift:registry:fixture","keys":[{
            "id":"fixture-key","publicKey":URL_SAFE_NO_PAD.encode(self.key.verifying_key().as_bytes()),"state":state}]})).unwrap()
    }
    pub fn trust(&self) -> TrustStore {
        TrustStore::from_json(&self.trust_bytes("active")).unwrap()
    }
    pub fn statement(&self, kind: &str) -> Value {
        json!({"schema":"glyphshift.release-statement/1","issuer":"urn:glyphshift:registry:fixture","publisherUserKey":"Abcdef23",
            "artifact":glyphshift_resource_inspector::inspect(kind,&self.body).unwrap()})
    }
    pub fn proof(&self, kind: &str) -> Vec<u8> {
        self.sign(&serde_json::to_vec(&self.statement(kind)).unwrap())
    }
    pub fn sign(&self, payload: &[u8]) -> Vec<u8> {
        self.sign_header(
            br#"{"alg":"EdDSA","kid":"fixture-key","typ":"glyphshift-release+jws"}"#,
            payload,
        )
    }
    pub fn sign_header(&self, header: &[u8], payload: &[u8]) -> Vec<u8> {
        let unsigned = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header),
            URL_SAFE_NO_PAD.encode(payload)
        );
        let sig = self.key.sign(unsigned.as_bytes());
        serde_json::to_vec(&json!({"schema":"glyphshift.release-proof/1","jws":format!("{}.{}",unsigned,URL_SAFE_NO_PAD.encode(sig.to_bytes()))})).unwrap()
    }
}

pub(crate) fn root() -> tempfile::TempDir {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../local-test/evidence/registry-client/contracts");
    std::fs::create_dir_all(&base).unwrap();
    tempfile::tempdir_in(base).unwrap()
}

pub(crate) fn adapter() -> (Fixture, tempfile::TempDir) {
    use glyphshift_adapter_native_host::NativeAdapterMetadata;
    use glyphshift_plugin_package::*;
    let root = root();
    let mut image = vec![0; 90];
    image[..2].copy_from_slice(b"MZ");
    image[60..64].copy_from_slice(&64u32.to_le_bytes());
    image[64..68].copy_from_slice(b"PE\0\0");
    image[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
    image[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    std::fs::write(root.path().join("adapter.dll"), &image).unwrap();
    std::fs::write(root.path().join("license.txt"), b"synthetic fixture").unwrap();
    let manifest = Manifest {
        schema: PACKAGE_SCHEMA.into(),
        package_id: "glyphshift-adapter-synthetic".into(),
        version: [1, 0, 0],
        runtime_bundle_schema: "glyphshift.runtime-bundle/4".into(),
        license_file: "license.txt".into(),
        files: vec![
            PackageFile {
                path: "adapter.dll".into(),
                sha256: sha256(&image),
                size: image.len() as u64,
                role: FileRole::NativeAdapter,
            },
            PackageFile {
                path: "license.txt".into(),
                sha256: sha256(b"synthetic fixture"),
                size: 17,
                role: FileRole::License,
            },
        ],
        variants: vec![Variant {
            platform: "windows".into(),
            architecture: "x86_64".into(),
            adapters: vec![Adapter {
                file: "adapter.dll".into(),
                native_metadata: NativeAdapterMetadata {
                    adapter_id: "synthetic.inline".into(),
                    version: [1, 0, 0],
                    abi: [1, 0],
                    apply_model: 1,
                    placement: 1,
                    feature_bits: 3,
                    platform_bits: 1,
                    architecture_bits: 3,
                    source_policy: 0,
                },
                name: "Synthetic".into(),
                summary: "Non-executable test image".into(),
                technology: "Synthetic".into(),
                process_resident_after_deactivate: true,
            }],
        }],
    };
    (
        Fixture {
            key: SigningKey::from_bytes(&[11; 32]),
            body: build_package(&manifest, root.path()).unwrap(),
        },
        root,
    )
}
