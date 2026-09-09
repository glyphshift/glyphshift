use crate::command_error::CommandError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const RELEASE_URL: &str = "https://www.yuelili.com/api/v1/apps/glyphshift/release";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Release {
    version: String,
    release_notes: String,
    download_url: String,
}

fn newer_release(bytes: &[u8], current: &str) -> Result<Option<Release>, ()> {
    let release: Release = serde_json::from_slice(bytes).map_err(|_| ())?;
    let version = semver::Version::parse(release.version.trim().trim_start_matches('v')).map_err(|_| ())?;
    let current = semver::Version::parse(current).map_err(|_| ())?;
    let url = reqwest::Url::parse(&release.download_url).map_err(|_| ())?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err(());
    }
    Ok((version.cmp_precedence(&current).is_gt()).then_some(release))
}

#[tauri::command]
pub(crate) async fn desktop_check_update() -> Result<Option<Release>, CommandError> {
    let failed = || CommandError::new("updates.check_failed");
    let client = reqwest::Client::builder().timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3)).build().map_err(|_| failed())?;
    let mut response = client.get(RELEASE_URL).send().await.map_err(|_| failed())?
        .error_for_status().map_err(|_| failed())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failed())? {
        if bytes.len() + chunk.len() > 64 * 1024 { return Err(failed()); }
        bytes.extend_from_slice(&chunk);
    }
    newer_release(&bytes, env!("CARGO_PKG_VERSION")).map_err(|_| failed())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(version: &str, url: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"version":version,"releaseNotes":"Changes","downloadUrl":url})).unwrap()
    }
    #[test]
    fn release_checks_version_order_and_download_address() {
        for version in ["0.3.0", "0.2.99", "0.3.0-beta.1", "0.3.0+build.4"] {
            assert!(newer_release(&payload(version,"https://example.com/download"),"0.3.0").unwrap().is_none());
        }
        for version in ["0.3.1", "0.10.0", "v1.0.0"] {
            assert!(newer_release(&payload(version,"https://example.com/download"),"0.3.0").unwrap().is_some());
        }
        for url in ["javascript:alert(1)", "file:///download", "http://example.com", "https://user:pass@example.com"] {
            assert!(newer_release(&payload("1.0.0",url),"0.3.0").is_err());
        }
        assert!(newer_release(b"{}", "0.3.0").is_err());
        assert!(newer_release(&payload("broken","https://example.com"),"0.3.0").is_err());
    }
}
