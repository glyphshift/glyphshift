use super::*;
use crate::fixtures::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

struct Server {
    origin: Url,
    paths: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(replies: Vec<(u16, String, Vec<u8>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let paths = Arc::new(Mutex::new(Vec::new()));
        let observed = paths.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut replies = replies.into_iter();
            while !done.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => break,
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0; 1];
                while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 8192 {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                let req = String::from_utf8_lossy(&bytes);
                observed
                    .lock()
                    .unwrap()
                    .push(req.lines().next().unwrap_or("").into());
                let Some((status, mime, body)) = replies.next() else {
                    break;
                };
                let response=format!("HTTP/1.1 {status} Fixture\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nLocation: http://127.0.0.1:1/foreign\r\nConnection: close\r\n\r\n",body.len());
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
                // Finish the HTTP/1 close handshake before reusing the fixture loop.
                // Dropping a Windows socket while the peer is still finishing can reset its response.
                let _ = stream.shutdown(std::net::Shutdown::Write);
                let _ = stream.read_to_end(&mut Vec::new());
            }
        });
        Self {
            origin,
            paths,
            stop,
            thread: Some(thread),
        }
    }
    fn client(&self, trust: TrustStore) -> RegistryClient {
        // Only this unit-test module can construct an HTTP transport; production has no insecure switch.
        RegistryClient {
            origin: self.origin.clone(),
            http: Client::builder()
                .no_proxy()
                .redirect(redirect::Policy::none())
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            trust,
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn ok_proof(f: &Fixture, kind: &str) -> (u16, String, Vec<u8>) {
    (200, "application/json".into(), f.proof(kind))
}
fn ok_body(f: &Fixture, kind: &str) -> (u16, String, Vec<u8>) {
    (
        200,
        if kind == "dictionary" {
            "application/vnd.glyphshift.dictionary+json;version=3"
        } else {
            "application/vnd.glyphshift.plugin+zip;version=1"
        }
        .into(),
        f.body.clone(),
    )
}

#[test]
fn secure_configuration_rejects_non_origin_or_insecure_urls() {
    let f = Fixture::dictionary("1.0.0");
    for url in [
        "http://fixture.invalid",
        "https://user:secret@fixture.invalid",
        "https://fixture.invalid/path",
        "https://fixture.invalid/?url=evil",
        "https://fixture.invalid/#fragment",
        "file:///fixture",
    ] {
        assert!(RegistryClient::new(url, f.trust()).is_err());
    }
}

fn discovery_entry() -> serde_json::Value {
    serde_json::json!({"packageId":"fixture.dictionary","version":"1.0.0","publisherUserKey":"Abcdef23",
        "name":"Synthetic","summary":"Menus","sourceLocale":"en-US","targetLocale":"zh-CN","tags":["menus"]})
}

fn discovery_reply(value: serde_json::Value) -> (u16, String, Vec<u8>) {
    (
        200,
        "application/json".into(),
        serde_json::to_vec(&value).unwrap(),
    )
}

#[test]
fn discovery_rejects_invalid_pages_and_checks_exact_details() {
    use crate::DictionarySearch;
    let fixture = Fixture::dictionary("1.0.0");
    let entry = discovery_entry();
    let query = DictionarySearch::default();
    for page in [
        serde_json::json!({"items":[entry.clone(),entry.clone()],"nextCursor":null}),
        serde_json::json!({"items":[],"nextCursor":"https://foreign.invalid"}),
        serde_json::json!({"items":[],"nextCursor":null,"extra":true}),
        {
            let mut e = entry.clone();
            e["version"] = serde_json::json!("1.0.0-rc.1");
            serde_json::json!({"items":[e],"nextCursor":null})
        },
    ] {
        let server = Server::new(vec![discovery_reply(page)]);
        assert!(server
            .client(fixture.trust())
            .search_dictionaries(&query)
            .is_err());
    }
    let server = Server::new(vec![discovery_reply(
        serde_json::json!({"items":[],"nextCursor":"next"}),
    )]);
    let query = DictionarySearch {
        cursor: Some("next".into()),
        ..Default::default()
    };
    assert!(server
        .client(fixture.trust())
        .search_dictionaries(&query)
        .is_err());
    let server = Server::new(vec![discovery_reply(entry.clone()), discovery_reply(entry)]);
    let client = server.client(fixture.trust());
    assert_eq!(
        client
            .dictionary_detail(
                &ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.0", "Abcdef23")
                    .unwrap()
            )
            .unwrap()
            .name,
        "Synthetic"
    );
    assert!(matches!(
        client.dictionary_detail(
            &ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.0", "Zbcdef23").unwrap()
        ),
        Err(Error::IdentityMismatch)
    ));
}

#[test]
fn discovery_query_is_bounded_and_encoded_as_values() {
    use crate::DictionarySearch;
    for json in [
        r#"{"size":0}"#,
        r#"{"size":101}"#,
        r#"{"url":"https://foreign.invalid"}"#,
        r#"{"size":1,"size":2}"#,
        r#"{"sourceLocale":"../../"}"#,
        r#"{"tag":""}"#,
    ] {
        assert!(DictionarySearch::from_json(json.as_bytes()).is_err());
    }
    let f = Fixture::dictionary("1.0.0");
    let server = Server::new(vec![discovery_reply(
        serde_json::json!({"items":[],"nextCursor":null}),
    )]);
    let query = DictionarySearch {
        text: "menu&cursor=foreign".into(),
        ..Default::default()
    };
    server
        .client(f.trust())
        .search_dictionaries(&query)
        .unwrap();
    let paths = server.paths.lock().unwrap();
    assert!(paths[0].contains("text=menu%26cursor%3Dforeign"));
    assert!(!paths[0].contains("&cursor=foreign"));
}

#[test]
fn withdrawal_during_download_never_creates_installation() {
    let f = Fixture::dictionary("1.0.0");
    let server = Server::new(vec![
        ok_proof(&f, "dictionary"),
        ok_body(&f, "dictionary"),
        (404, "application/problem+json".into(), b"{}".to_vec()),
    ]);
    let root = root();
    let destination = root.path().join("install");
    let r = ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.0", "Abcdef23").unwrap();
    assert_eq!(
        server
            .client(f.trust())
            .install_dictionary(
                &r,
                "fixture.registry",
                &destination,
                DictionaryReplacementPolicy::RejectExisting
            )
            .unwrap_err(),
        Error::NotFound
    );
    assert!(!destination.exists());
    assert_eq!(server.paths.lock().unwrap().len(), 3);
}

#[test]
fn network_failures_and_revoked_keys_do_not_write_files() {
    let f = Fixture::dictionary("1.0.0");
    let request =
        ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.0", "Abcdef23").unwrap();
    let root = root();
    let destination = root.path().join("install");
    for replies in [
        vec![(302, "application/json".into(), b"{}".to_vec())],
        vec![(503, "application/problem+json".into(), b"{}".to_vec())],
        vec![
            ok_proof(&f, "dictionary"),
            (
                200,
                "application/vnd.glyphshift.dictionary+json;version=3".into(),
                vec![0; f.body.len() + 1],
            ),
        ],
        vec![
            ok_proof(&f, "dictionary"),
            (200, "text/html".into(), f.body.clone()),
        ],
    ] {
        let server = Server::new(replies);
        assert!(server
            .client(f.trust())
            .install_dictionary(
                &request,
                "fixture.registry",
                &destination,
                DictionaryReplacementPolicy::RejectExisting
            )
            .is_err());
        assert!(!destination.exists());
    }
    let server = Server::new(vec![ok_proof(&f, "dictionary")]);
    assert_eq!(
        server
            .client(TrustStore::from_json(&f.trust_bytes("revoked")).unwrap())
            .install_dictionary(
                &request,
                "fixture.registry",
                &destination,
                DictionaryReplacementPolicy::RejectExisting
            )
            .unwrap_err(),
        Error::UntrustedKey
    );
    assert!(!destination.exists());
    assert_eq!(server.paths.lock().unwrap().len(), 1);
}

#[test]
fn adapter_online_install_is_immutable_and_not_selected() {
    let (f, _source) = adapter();
    let server = Server::new(vec![
        ok_proof(&f, "adapter"),
        ok_body(&f, "adapter"),
        ok_proof(&f, "adapter"),
    ]);
    let root = root();
    let r = ReleaseRequest::new(
        "adapter",
        "glyphshift-adapter-synthetic",
        "1.0.0",
        "Abcdef23",
    )
    .unwrap();
    let selection = server
        .client(f.trust())
        .install_adapter(&r, root.path())
        .unwrap();
    assert!(!selection.selected);
    let installed = PluginStore::new(root.path()).list().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].sha256, selection.sha256);
}

#[test]
fn dictionary_install_and_update_keep_local_modification_guard() {
    let f = Fixture::dictionary("1.0.0");
    let next = Fixture::dictionary("1.0.1");
    let root = root();
    let destination = root.path().join("data");
    let server = Server::new(vec![
        ok_proof(&f, "dictionary"),
        ok_body(&f, "dictionary"),
        ok_proof(&f, "dictionary"),
        ok_proof(&next, "dictionary"),
        ok_body(&next, "dictionary"),
        ok_proof(&next, "dictionary"),
    ]);
    let client = server.client(f.trust());
    let r = ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.0", "Abcdef23").unwrap();
    // Loopback HTTP exercises the transport; the domain port keeps its HTTPS-only contract.
    // The complete HTTPS-to-store path is also covered by the Go/Rust smoke harness.
    let install = |request: &ReleaseRequest, catalog: &str, destination: &Path, policy| {
        let fetched = client.fetch_current(request)?;
        crate::dictionary::install(
            client.trust.clone(),
            fetched.proof,
            fetched.body,
            fetched.download_url.replacen("http://", "https://", 1),
            catalog,
            destination,
            policy,
        )
    };
    let installed = install(
        &r,
        "fixture.registry",
        &destination,
        DictionaryReplacementPolicy::RejectExisting,
    )
    .unwrap();
    assert_eq!(
        installed.state(),
        glyphshift_dictionary_distribution::DictionaryInstallationState::Verified
    );
    let source = installed.source().unwrap();
    assert_eq!(source.signature().scheme(), "glyphshift-registry-jws-v1");
    let dictionary = destination.join("dictionaries/fixture.dictionary.json");
    let original = std::fs::read_to_string(&dictionary).unwrap();
    let changed = original.replace("打开", "本地编辑");
    assert_ne!(changed, original);
    std::fs::write(&dictionary, &changed).unwrap();
    let request =
        ReleaseRequest::new("dictionary", "fixture.dictionary", "1.0.1", "Abcdef23").unwrap();
    assert_eq!(
        install(
            &request,
            "fixture.registry",
            &destination,
            DictionaryReplacementPolicy::ReplaceVerified
        )
        .unwrap_err(),
        Error::LocalChangesConflict
    );
    assert_eq!(std::fs::read_to_string(dictionary).unwrap(), changed);
}
