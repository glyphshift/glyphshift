use crate::command_error::CommandError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path, process::Command, time::Duration};
use tauri_plugin_opener::OpenerExt;
use yueli_distribution_sdk::{CheckRequest, Client, PublicUpdate, Target};

const DISTRIBUTION_BASE_URL: &str = "https://apps.yuelili.com";
const SOFTWARE_SLUG: &str = "glyphshift";
const UPDATE_CHANNEL: &str = "stable";
const MAX_INSTALLER_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Release {
    version: String,
    release_notes: String,
    sources: Vec<UpdateSource>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSource {
    id: String,
    kind: String,
    label: String,
    action: String,
    provider: String,
    extraction_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadConfig {
    release: DownloadRelease,
    #[serde(default)]
    sources: Vec<DownloadSource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadRelease {
    version: String,
    platform: String,
    architecture: String,
    channel: String,
    file_name: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DownloadSource {
    id: String,
    kind: String,
    label: String,
    priority: i64,
    delivery: DownloadDelivery,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadDelivery {
    mode: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    extraction_code: String,
}

#[derive(Debug, Clone)]
struct DirectInstaller {
    version: String,
    file_name: String,
    size_bytes: u64,
    sha256: String,
    url: reqwest::Url,
}

fn project_update(update: PublicUpdate, sources: Vec<UpdateSource>) -> Option<Release> {
    update.update_available.then_some(Release {
        version: update.release.version,
        release_notes: update.release.release_notes,
        sources,
    })
}

fn download_config_url(version: &str, target: &Target) -> Result<reqwest::Url, ()> {
    let mut url = reqwest::Url::parse(&format!(
        "{DISTRIBUTION_BASE_URL}/api/v1/software/{SOFTWARE_SLUG}/download-config"
    ))
    .map_err(|_| ())?;
    url.query_pairs_mut()
        .append_pair("version", version)
        .append_pair("channel", UPDATE_CHANNEL)
        .append_pair("platform", &target.platform)
        .append_pair("architecture", &target.architecture);
    Ok(url)
}

async fn fetch_download_config(version: &str, target: &Target) -> Result<DownloadConfig, ()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|_| ())?;
    client
        .get(download_config_url(version, target)?)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?
        .json()
        .await
        .map_err(|_| ())
}

fn direct_installer(
    config: DownloadConfig,
    expected_version: &str,
    target: &Target,
    source_id: &str,
) -> Option<DirectInstaller> {
    let release = config.release;
    if !valid_release(&release, expected_version, target) {
        return None;
    }
    let source = config.sources.into_iter().find(|source| {
        source.id == source_id
            && source.kind.eq_ignore_ascii_case("github")
            && source.delivery.mode.eq_ignore_ascii_case("file")
            && !source.delivery.url.trim().is_empty()
    })?;
    let url = github_url(&source.delivery.url)?;
    if url.path_segments().and_then(|segments| segments.last()) != Some(release.file_name.as_str())
    {
        return None;
    }
    Some(DirectInstaller {
        version: release.version,
        file_name: release.file_name,
        size_bytes: release.size_bytes,
        sha256: release.sha256.to_ascii_lowercase(),
        url,
    })
}

fn valid_release(release: &DownloadRelease, expected_version: &str, target: &Target) -> bool {
    release.version == expected_version
        && release.platform.eq_ignore_ascii_case(&target.platform)
        && release
            .architecture
            .eq_ignore_ascii_case(&target.architecture)
        && release.channel.eq_ignore_ascii_case(UPDATE_CHANNEL)
        && release.size_bytes > 0
        && release.size_bytes <= MAX_INSTALLER_BYTES
        && release.sha256.len() == 64
        && release.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        && safe_installer_name(&release.file_name)
}

fn github_url(value: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    let parts = url.path_segments()?.collect::<Vec<_>>();
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
        && parts.len() >= 6
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts[2] == "releases"
        && parts[3] == "download"
        && !parts[4].is_empty()
        && parts[4] != "latest")
        .then_some(url)
}

fn browser_url(value: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some())
    .then_some(url)
}

fn safe_source(source: &DownloadSource) -> bool {
    !source.id.trim().is_empty()
        && source.id.len() <= 120
        && !source.label.trim().is_empty()
        && source.label.len() <= 80
}

fn fallback_source() -> UpdateSource {
    UpdateSource {
        id: "website".into(),
        kind: "website".into(),
        label: "月离软件页".into(),
        action: "open".into(),
        provider: String::new(),
        extraction_code: String::new(),
    }
}

fn update_sources(
    config: &DownloadConfig,
    expected_version: &str,
    target: &Target,
) -> Vec<UpdateSource> {
    if !valid_release(&config.release, expected_version, target) {
        return Vec::new();
    }
    let mut sources = config
        .sources
        .iter()
        .filter(|source| safe_source(source))
        .filter_map(|source| {
            let kind = source.kind.to_ascii_lowercase();
            let mode = source.delivery.mode.to_ascii_lowercase();
            let action = match (kind.as_str(), mode.as_str()) {
                ("github", "file") if github_url(&source.delivery.url).is_some() => "install",
                ("hosted", "session") => "open",
                ("cloud_drive" | "external_link", "browser")
                    if browser_url(&source.delivery.url).is_some() =>
                {
                    "open"
                }
                _ => return None,
            };
            Some((
                source.priority,
                UpdateSource {
                    id: source.id.clone(),
                    kind,
                    label: source.label.trim().to_owned(),
                    action: action.into(),
                    provider: source.delivery.provider.trim().to_owned(),
                    extraction_code: source.delivery.extraction_code.trim().to_owned(),
                },
            ))
        })
        .collect::<Vec<_>>();
    sources.sort_by_key(|(priority, _)| *priority);
    sources.into_iter().map(|(_, source)| source).collect()
}

fn selected_browser_url(
    config: DownloadConfig,
    expected_version: &str,
    target: &Target,
    source_id: &str,
    software_url: reqwest::Url,
) -> Option<reqwest::Url> {
    if !valid_release(&config.release, expected_version, target) {
        return None;
    }
    let source = config
        .sources
        .into_iter()
        .find(|source| source.id == source_id && safe_source(source))?;
    match (
        source.kind.to_ascii_lowercase().as_str(),
        source.delivery.mode.to_ascii_lowercase().as_str(),
    ) {
        ("hosted", "session") => Some(software_url),
        ("cloud_drive" | "external_link", "browser") => browser_url(&source.delivery.url),
        _ => None,
    }
}

fn safe_installer_name(file_name: &str) -> bool {
    let path = Path::new(file_name);
    path.file_name().and_then(|name| name.to_str()) == Some(file_name)
        && file_name.to_ascii_lowercase().ends_with(".exe")
}

async fn download_installer(installer: &DirectInstaller) -> Result<std::path::PathBuf, ()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| ())?;
    let mut response = client
        .get(installer.url.clone())
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?;
    let version = installer
        .version
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let directory = std::env::temp_dir()
        .join("Glyphshift")
        .join("updates")
        .join(version);
    fs::create_dir_all(&directory).map_err(|_| ())?;
    let final_path = directory.join(&installer.file_name);
    let partial_path = directory.join(format!("{}.part", installer.file_name));
    let _ = fs::remove_file(&partial_path);
    let mut output = fs::File::create(&partial_path).map_err(|_| ())?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        downloaded = downloaded.checked_add(chunk.len() as u64).ok_or(())?;
        if downloaded > installer.size_bytes {
            let _ = fs::remove_file(&partial_path);
            return Err(());
        }
        output.write_all(&chunk).map_err(|_| ())?;
        hasher.update(&chunk);
    }
    output.flush().map_err(|_| ())?;
    drop(output);
    let digest = format!("{:x}", hasher.finalize());
    if downloaded != installer.size_bytes || !digest.eq_ignore_ascii_case(&installer.sha256) {
        let _ = fs::remove_file(&partial_path);
        return Err(());
    }
    let _ = fs::remove_file(&final_path);
    fs::rename(&partial_path, &final_path).map_err(|_| ())?;
    Ok(final_path)
}

#[tauri::command]
pub(crate) async fn desktop_check_update() -> Result<Option<Release>, CommandError> {
    let failed = || CommandError::new("updates.check_failed");
    let client = Client::new(DISTRIBUTION_BASE_URL).map_err(|_| failed())?;
    let target = Target::tauri_runtime();
    let update = client
        .check_public(CheckRequest {
            slug: SOFTWARE_SLUG,
            current_version: env!("CARGO_PKG_VERSION"),
            channel: UPDATE_CHANNEL,
            target: target.clone(),
        })
        .await
        .map_err(|_| failed())?;
    let sources = if update.update_available {
        let mut sources = fetch_download_config(&update.release.version, &target)
            .await
            .ok()
            .map(|config| update_sources(&config, &update.release.version, &target))
            .unwrap_or_default();
        if sources.is_empty() {
            sources.push(fallback_source());
        }
        sources
    } else {
        Vec::new()
    };
    Ok(project_update(update, sources))
}

#[tauri::command]
pub(crate) async fn desktop_install_update(
    version: String,
    source_id: String,
) -> Result<(), CommandError> {
    let failed = || CommandError::new("updates.install_failed");
    let version = version.trim();
    let source_id = source_id.trim();
    if version.is_empty() || version.len() > 120 || source_id.is_empty() || source_id.len() > 120 {
        return Err(failed());
    }
    let target = Target::tauri_runtime();
    let installer = fetch_download_config(version, &target)
        .await
        .ok()
        .and_then(|config| direct_installer(config, version, &target, source_id))
        .ok_or_else(failed)?;
    let path = download_installer(&installer).await.map_err(|_| failed())?;
    Command::new(path).spawn().map_err(|_| failed())?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn desktop_open_update_source(
    app: tauri::AppHandle,
    version: String,
    source_id: String,
) -> Result<(), CommandError> {
    let failed = || CommandError::new("updates.open_failed");
    let version = version.trim();
    let source_id = source_id.trim();
    if version.is_empty() || version.len() > 120 || source_id.is_empty() || source_id.len() > 120 {
        return Err(failed());
    }
    let client = Client::new(DISTRIBUTION_BASE_URL).map_err(|_| failed())?;
    let software_url = client
        .software_page_url(SOFTWARE_SLUG)
        .map_err(|_| failed())?;
    let url = if source_id == "website" {
        software_url
    } else {
        let target = Target::tauri_runtime();
        let config = fetch_download_config(version, &target)
            .await
            .map_err(|_| failed())?;
        selected_browser_url(config, version, &target, source_id, software_url)
            .ok_or_else(failed)?
    };
    app.opener()
        .open_url(url.to_string(), None::<String>)
        .map_err(|_| failed())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(version: &str, available: bool) -> PublicUpdate {
        serde_json::from_value(serde_json::json!({
            "release": {"id": "release", "version": version, "releaseNotes": "Changes"},
            "updateAvailable": available
        }))
        .unwrap()
    }

    #[test]
    fn distribution_projection_preserves_update_ui_contract() {
        let release = project_update(
            update("0.6.0", true),
            vec![UpdateSource {
                id: "github".into(),
                kind: "github".into(),
                label: "GitHub 下载".into(),
                action: "install".into(),
                provider: String::new(),
                extraction_code: String::new(),
            }],
        )
        .unwrap();
        assert_eq!(release.version, "0.6.0");
        assert_eq!(release.release_notes, "Changes");
        assert_eq!(release.sources[0].action, "install");
        assert!(project_update(update("0.6.0", false), Vec::new()).is_none());
    }

    #[test]
    fn github_download_config_requires_exact_target_and_fixed_https_file() {
        let target = Target::normalize("windows", "x86_64");
        let config: DownloadConfig = serde_json::from_value(serde_json::json!({
            "release": {
                "version": "0.6.0",
                "platform": "windows",
                "architecture": "amd64",
                "channel": "stable",
                "fileName": "Glyphshift_0.6.0_x64-setup.exe",
                "sizeBytes": 123,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "sources": [{
                "id": "github-release",
                "kind": "github",
                "label": "GitHub 下载",
                "priority": 20,
                "delivery": {
                    "mode": "file",
                    "url": "https://github.com/Yuelioi/glyphshift/releases/download/v0.6.0/Glyphshift_0.6.0_x64-setup.exe"
                }
            }]
        }))
        .unwrap();
        let installer = direct_installer(config, "0.6.0", &target, "github-release").unwrap();
        assert_eq!(installer.file_name, "Glyphshift_0.6.0_x64-setup.exe");
        assert_eq!(installer.size_bytes, 123);
    }

    #[test]
    fn download_config_projects_selectable_install_and_browser_sources() {
        let target = Target::normalize("windows", "x86_64");
        let config: DownloadConfig = serde_json::from_value(serde_json::json!({
            "release": {
                "version": "0.6.0",
                "platform": "windows",
                "architecture": "amd64",
                "channel": "stable",
                "fileName": "Glyphshift_0.6.0_x64-setup.exe",
                "sizeBytes": 123,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "sources": [
                {"id":"hosted","kind":"hosted","label":"本站付费下载","priority":10,"delivery":{"mode":"session"}},
                {"id":"github","kind":"github","label":"GitHub 下载","priority":20,"delivery":{"mode":"file","url":"https://github.com/Yuelioi/glyphshift/releases/download/v0.6.0/Glyphshift_0.6.0_x64-setup.exe"}},
                {"id":"cloud","kind":"cloud_drive","label":"夸克网盘","priority":30,"delivery":{"mode":"browser","url":"https://pan.quark.cn/s/example","provider":"夸克网盘","extractionCode":"1234"}},
                {"id":"web","kind":"external_link","label":"网页下载","priority":40,"delivery":{"mode":"browser","url":"https://downloads.example.com/glyphshift"}}
            ]
        }))
        .unwrap();
        let sources = update_sources(&config, "0.6.0", &target);
        assert_eq!(sources.len(), 4);
        assert_eq!(sources[0].id, "hosted");
        assert_eq!(sources[0].action, "open");
        assert_eq!(sources[1].id, "github");
        assert_eq!(sources[1].action, "install");
        assert_eq!(sources[2].provider, "夸克网盘");
        assert_eq!(sources[2].extraction_code, "1234");
    }
}
