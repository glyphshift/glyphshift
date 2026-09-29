//! Explicit operator/client entry point. Public Registry reads require no account token.
use glyphshift_dictionary_distribution::DictionaryReplacementPolicy;
use glyphshift_registry_client::{
    read_bounded, DictionarySearch, Error, RegistryClient, ReleaseRequest, TrustStore, MAX_PROOF,
};
use std::path::Path;

fn trust(path: &str) -> Result<TrustStore, Error> {
    TrustStore::from_json(&read_bounded(Path::new(path), 65536)?)
}
fn client(origin: &str, path: &str) -> Result<RegistryClient, Error> {
    let ca = std::env::var_os("GLYPHSHIFT_REGISTRY_CA_FILE")
        .map(|p| read_bounded(Path::new(&p), 65536))
        .transpose()?;
    RegistryClient::with_ca(origin, trust(path)?, ca.as_deref())
}

fn run(args: &[String]) -> Result<(), Error> {
    let a: Vec<_> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["search-dictionaries", origin, trusted, query] => {
            let query = DictionarySearch::from_json(&read_bounded(Path::new(query), 8192)?)?;
            let page = client(origin, trusted)?.search_dictionaries(&query)?;
            println!(
                "{}",
                serde_json::to_string(&page).map_err(|_| Error::InvalidArtifact)?
            );
        }
        ["dictionary-detail", origin, trusted, id, version, publisher] => {
            let request = ReleaseRequest::new("dictionary", id, version, publisher)?;
            let entry = client(origin, trusted)?.dictionary_detail(&request)?;
            println!(
                "{}",
                serde_json::to_string(&entry).map_err(|_| Error::InvalidArtifact)?
            );
        }
        ["verify", trusted, proof, kind, id, version, publisher, artifact] => {
            let request = ReleaseRequest::new(kind, id, version, publisher)?;
            let result = trust(trusted)?.verify_artifact(
                &read_bounded(Path::new(proof), MAX_PROOF)?,
                &request,
                &read_bounded(Path::new(artifact), request.byte_limit())?,
            )?;
            println!(
                "{}",
                serde_json::json!({"status":"verified-registry-attestation","statement":result.statement()})
            );
        }
        ["install-adapter", origin, trusted, id, version, publisher, root] => {
            let request = ReleaseRequest::new("adapter", id, version, publisher)?;
            let installed = client(origin, trusted)?.install_adapter(&request, Path::new(root))?;
            println!(
                "{}",
                serde_json::to_string(&installed).map_err(|_| Error::Storage)?
            );
        }
        ["install-dictionary", origin, trusted, catalog, id, version, publisher, root, policy] => {
            let replacement = match *policy {
                "reject-existing" => DictionaryReplacementPolicy::RejectExisting,
                "replace-verified" => DictionaryReplacementPolicy::ReplaceVerified,
                _ => return Err(Error::Configuration),
            };
            let request = ReleaseRequest::new("dictionary", id, version, publisher)?;
            let installed = client(origin, trusted)?.install_dictionary(
                &request,
                catalog,
                Path::new(root),
                replacement,
            )?;
            println!(
                "{}",
                serde_json::json!({"dictionaryId":installed.dictionary_id(),"state":format!("{:?}",installed.state()),"version":version})
            );
        }
        _ => return Err(Error::Configuration),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("Registry operation failed: {error:?}");
        if error == Error::Configuration {
            eprintln!("usage:\n  glyphshift-registry search-dictionaries <https-origin> <trust.json> <query.json>\n  glyphshift-registry dictionary-detail <https-origin> <trust.json> <id> <version> <publisher-userKey>\n  glyphshift-registry verify <trust.json> <proof.json> <kind> <id> <version> <publisher-userKey> <artifact>\n  glyphshift-registry install-adapter <https-origin> <trust.json> <id> <version> <publisher-userKey> <store>\n  glyphshift-registry install-dictionary <https-origin> <trust.json> <catalog-id> <id> <version> <publisher-userKey> <absolute-data-root> <reject-existing|replace-verified>");
        }
        std::process::exit(1);
    }
}
