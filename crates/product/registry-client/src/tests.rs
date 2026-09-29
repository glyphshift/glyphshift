use crate::{fixtures::*, *};
use serde_json::json;

fn request() -> ReleaseRequest {
    ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.0", "Abcdef23").unwrap()
}

#[test]
fn proof_binds_identity_bytes_and_authoritative_inspection() {
    let f = Fixture::dictionary("1.0.0");
    let proof = f.proof("dictionary");
    assert!(f
        .trust()
        .verify_artifact(&proof, &request(), &f.body)
        .is_ok());
    for (kind, id, version, publisher) in [
        ("adapter", "fixture.dictionary", "1.0.0", "Abcdef23"),
        ("dictionary", "other", "1.0.0", "Abcdef23"),
        ("dictionary", "fixture.dictionary", "1.0.1", "Abcdef23"),
        ("dictionary", "fixture.dictionary", "1.0.0", "Zbcdef23"),
    ] {
        assert_eq!(
            f.trust()
                .verify_artifact(
                    &proof,
                    &ReleaseRequest::new(kind, id, version, publisher).unwrap(),
                    &f.body
                )
                .unwrap_err(),
            Error::IdentityMismatch
        );
    }
    assert_eq!(
        f.trust()
            .verify_artifact(&proof, &request(), b"tampered")
            .unwrap_err(),
        Error::InvalidArtifact
    );
    let mut statement = f.statement("dictionary");
    statement["artifact"]["compatibility"]["sourceLocale"] = json!("fr-FR");
    let wrong = f.sign(&serde_json::to_vec(&statement).unwrap());
    assert_eq!(
        f.trust()
            .verify_artifact(&wrong, &request(), &f.body)
            .unwrap_err(),
        Error::InvalidArtifact
    );
    let invalid = br#"{"schema":"glyphshift.dictionary/3","metadata":{"id":"fixture.dictionary"}}"#;
    statement["artifact"]["sha256"] = json!(glyphshift_plugin_package::sha256(invalid));
    statement["artifact"]["size"] = json!(invalid.len());
    assert_eq!(
        f.trust()
            .verify_artifact(
                &f.sign(&serde_json::to_vec(&statement).unwrap()),
                &request(),
                invalid
            )
            .unwrap_err(),
        Error::InvalidArtifact
    );
}

#[test]
fn algorithm_header_duplicate_fields_and_foreign_signatures_fail_closed() {
    let f = Fixture::dictionary("1.0.0");
    let payload = serde_json::to_vec(&f.statement("dictionary")).unwrap();
    for header in [
        r#"{"alg":"HS256","kid":"fixture-key","typ":"glyphshift-release+jws"}"#,
        r#"{"alg":"EdDSA","kid":"fixture-key","typ":"JWT"}"#,
        r#"{"alg":"EdDSA","kid":"fixture-key","typ":"glyphshift-release+jws","jku":"https://foreign.invalid/keys"}"#,
        r#"{"alg":"EdDSA","kid":"fixture-key","kid":"fixture-key","typ":"glyphshift-release+jws"}"#,
        r#"{"alg":"EdDSA","kid":"foreign","typ":"glyphshift-release+jws"}"#,
    ] {
        assert!(f
            .trust()
            .verify(&f.sign_header(header.as_bytes(), &payload))
            .is_err());
    }
    let payload = String::from_utf8(payload).unwrap();
    for altered in [
        payload.replacen(
            "\"publisherUserKey\":",
            "\"publisherUserKey\":\"Abcdef23\",\"publisherUserKey\":",
            1,
        ),
        payload.replacen("\"entryCount\":1", "\"entryCount\":1,\"entryCount\":1", 1),
        payload.replacen("\"publisherUserKey\":", "\"PublisherUserKey\":", 1),
        format!("{payload}{{}}"),
    ] {
        assert!(f.trust().verify(&f.sign(altered.as_bytes())).is_err());
    }
    let mut foreign = Fixture::dictionary("1.0.0");
    foreign.key = ed25519_dalek::SigningKey::from_bytes(&[19; 32]);
    assert_eq!(
        f.trust().verify(&foreign.proof("dictionary")).unwrap_err(),
        Error::InvalidProof
    );
}

#[test]
fn rotation_revocation_and_malformed_configuration() {
    let f = Fixture::dictionary("1.0.0");
    let proof = f.proof("dictionary");
    let revoked = TrustStore::from_json(&f.trust_bytes("revoked")).unwrap();
    assert_eq!(revoked.verify(&proof).unwrap_err(), Error::UntrustedKey);
    let mut config: serde_json::Value = serde_json::from_slice(&f.trust_bytes("active")).unwrap();
    let mut other = config["keys"][0].clone();
    other["id"] = json!("new-key");
    config["keys"].as_array_mut().unwrap().push(other.clone());
    assert!(TrustStore::from_json(&serde_json::to_vec(&config).unwrap())
        .unwrap()
        .verify(&proof)
        .is_ok());
    config["keys"].as_array_mut().unwrap().push(other);
    assert!(TrustStore::from_json(&serde_json::to_vec(&config).unwrap()).is_err());
    let wrong = String::from_utf8(f.trust_bytes("active"))
        .unwrap()
        .replace("urn:glyphshift:registry:fixture", "urn:glyphshift:other");
    assert_eq!(
        TrustStore::from_json(wrong.as_bytes())
            .unwrap()
            .verify(&proof)
            .unwrap_err(),
        Error::InvalidProof
    );
    for id in ["../escape", "a/b", "a%2fb", "a?b", ""] {
        assert!(ReleaseRequest::new("dictionary", id, "1.0.0", "Abcdef23").is_err());
    }
    for version in ["1.0", "1.0.0+build", "01.0.0"] {
        assert!(ReleaseRequest::new("dictionary", "fixture", version, "Abcdef23").is_err());
    }
}

#[test]
fn adapter_static_inspection_matches_signed_variants() {
    let (f, _root) = adapter();
    let r = ReleaseRequest::new(
        "adapter",
        "glyphshift-adapter-synthetic",
        "1.0.0",
        "Abcdef23",
    )
    .unwrap();
    assert!(f
        .trust()
        .verify_artifact(&f.proof("adapter"), &r, &f.body)
        .is_ok());
    let mut s = f.statement("adapter");
    s["artifact"]["compatibility"]["variants"][0]["architecture"] = json!("x86");
    assert_eq!(
        f.trust()
            .verify_artifact(&f.sign(&serde_json::to_vec(&s).unwrap()), &r, &f.body)
            .unwrap_err(),
        Error::InvalidArtifact
    );
}
