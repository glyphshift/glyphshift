use reqwest::{Method, StatusCode, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{error::Error as StdError, fmt, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub platform: String,
    pub architecture: String,
}

impl Target {
    pub fn tauri_runtime() -> Self {
        Self::normalize(std::env::consts::OS, std::env::consts::ARCH)
    }

    pub fn normalize(platform: &str, architecture: &str) -> Self {
        let platform = match platform.trim().to_ascii_lowercase().as_str() {
            "macos" | "mac" | "darwin" => "darwin".to_owned(),
            "win" | "win32" | "windows" => "windows".to_owned(),
            "linux" => "linux".to_owned(),
            other => other.to_owned(),
        };
        let architecture = match architecture.trim().to_ascii_lowercase().as_str() {
            "x86_64" | "x64" | "amd64" => "amd64".to_owned(),
            "aarch64" | "arm64" => "arm64".to_owned(),
            "x86" | "i386" | "i686" | "386" => "386".to_owned(),
            other => other.to_owned(),
        };
        Self {
            platform,
            architecture,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckRequest<'a> {
    pub slug: &'a str,
    pub current_version: &'a str,
    pub channel: &'a str,
    pub target: Target,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub release_notes: String,
    #[serde(default)]
    pub price_cents: i64,
    #[serde(default)]
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    pub state: String,
    #[serde(default)]
    pub can_download: bool,
    #[serde(default)]
    pub product_access_required: bool,
    #[serde(default)]
    pub purchase_required: bool,
    #[serde(default)]
    pub purchase_id: String,
    #[serde(default)]
    pub pay_url: String,
    #[serde(default)]
    pub base_price_cents: i64,
    #[serde(default)]
    pub discount_cents: i64,
    #[serde(default)]
    pub price_cents: i64,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub membership_plan_key: String,
    #[serde(default)]
    pub membership_plan_version: i64,
    #[serde(default)]
    pub max_sessions: i64,
    #[serde(default)]
    pub used_sessions: i64,
    #[serde(default)]
    pub remaining_sessions: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub release: Release,
    pub update_available: bool,
    pub access: Access,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUpdate {
    pub release: Release,
    pub update_available: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Purchase {
    pub id: String,
    pub release_id: String,
    #[serde(default)]
    pub price_cents: i64,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub max_sessions: i64,
    #[serde(default)]
    pub used_sessions: i64,
    #[serde(default)]
    pub pay_url: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSession {
    pub id: String,
    #[serde(default)]
    pub purchase_id: String,
    #[serde(default)]
    pub attempt_no: i64,
    pub url: String,
    #[serde(default)]
    pub state: String,
}

#[derive(Debug)]
pub enum Error {
    InvalidBaseUrl,
    HttpsRequired,
    AccessTokenRequired,
    Http(reqwest::Error),
    HttpStatus(StatusCode, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBaseUrl => write!(f, "distribution: invalid base URL"),
            Self::HttpsRequired => write!(f, "distribution: HTTPS is required"),
            Self::AccessTokenRequired => write!(f, "distribution: access token is required"),
            Self::Http(error) => write!(f, "distribution: {error}"),
            Self::HttpStatus(status, body) => write!(f, "distribution: HTTP {status}: {body}"),
        }
    }
}

impl StdError for Error {}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

pub struct Client {
    base_url: Url,
    http: reqwest::Client,
}

impl Client {
    pub fn new(base_url: &str) -> Result<Self, Error> {
        Self::new_with_http_policy(base_url, false)
    }

    pub fn new_with_http_policy(base_url: &str, allow_insecure_http: bool) -> Result<Self, Error> {
        let base_url =
            Url::parse(base_url.trim_end_matches('/')).map_err(|_| Error::InvalidBaseUrl)?;
        if base_url.host_str().is_none() {
            return Err(Error::InvalidBaseUrl);
        }
        if base_url.scheme() != "https" && !(allow_insecure_http && base_url.scheme() == "http") {
            return Err(Error::HttpsRequired);
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()?;
        Ok(Self { base_url, http })
    }

    pub async fn check(
        &self,
        access_token: &str,
        request: CheckRequest<'_>,
    ) -> Result<Update, Error> {
        let mut endpoint = self.endpoint(&format!(
            "api/v1/updater/{}/latest",
            encode_segment(request.slug)
        ))?;
        endpoint
            .query_pairs_mut()
            .append_pair("currentVersion", request.current_version)
            .append_pair(
                "channel",
                if request.channel.trim().is_empty() {
                    "stable"
                } else {
                    request.channel
                },
            )
            .append_pair("platform", &request.target.platform)
            .append_pair("architecture", &request.target.architecture);
        self.json(Method::GET, endpoint, access_token, None::<&()>)
            .await
    }

    pub async fn check_public(&self, request: CheckRequest<'_>) -> Result<PublicUpdate, Error> {
        let mut endpoint = self.endpoint(&format!(
            "api/v1/updater/{}/latest-metadata",
            encode_segment(request.slug)
        ))?;
        endpoint
            .query_pairs_mut()
            .append_pair("currentVersion", request.current_version)
            .append_pair(
                "channel",
                if request.channel.trim().is_empty() {
                    "stable"
                } else {
                    request.channel
                },
            )
            .append_pair("platform", &request.target.platform)
            .append_pair("architecture", &request.target.architecture);
        self.public_json(endpoint).await
    }

    pub fn software_page_url(&self, slug: &str) -> Result<Url, Error> {
        self.endpoint(&format!("software/{}", encode_segment(slug)))
    }

    pub async fn create_purchase(
        &self,
        access_token: &str,
        release_id: &str,
        idempotency_key: &str,
    ) -> Result<Purchase, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            idempotency_key: &'a str,
        }
        #[derive(Deserialize)]
        struct Response {
            purchase: Purchase,
        }
        let endpoint = self.endpoint(&format!(
            "api/v1/releases/{}/purchases",
            encode_segment(release_id)
        ))?;
        let response: Response = self
            .json(
                Method::POST,
                endpoint,
                access_token,
                Some(&Body { idempotency_key }),
            )
            .await?;
        Ok(response.purchase)
    }

    pub async fn create_session(
        &self,
        access_token: &str,
        slug: &str,
        release_id: &str,
    ) -> Result<DownloadSession, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            release_id: &'a str,
        }
        #[derive(Deserialize)]
        struct Response {
            session: DownloadSession,
        }
        let endpoint =
            self.endpoint(&format!("api/v1/updater/{}/sessions", encode_segment(slug)))?;
        let response: Response = self
            .json(
                Method::POST,
                endpoint,
                access_token,
                Some(&Body { release_id }),
            )
            .await?;
        Ok(response.session)
    }

    async fn json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: Method,
        endpoint: Url,
        access_token: &str,
        body: Option<&B>,
    ) -> Result<T, Error> {
        let token = access_token
            .trim()
            .strip_prefix("Bearer ")
            .unwrap_or(access_token.trim());
        if token.is_empty() {
            return Err(Error::AccessTokenRequired);
        }
        let mut request = self
            .http
            .request(method, endpoint)
            .bearer_auth(token)
            .header("Accept", "application/json");
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::HttpStatus(status, body));
        }
        Ok(response.json().await?)
    }

    async fn public_json<T: DeserializeOwned>(&self, endpoint: Url) -> Result<T, Error> {
        let response = self
            .http
            .get(endpoint)
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::HttpStatus(status, body));
        }
        Ok(response.json().await?)
    }

    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        let mut endpoint = self.base_url.clone();
        let base_path = endpoint.path().trim_end_matches('/');
        endpoint.set_path(&format!("{base_path}/{}", path.trim_start_matches('/')));
        endpoint.set_query(None);
        endpoint.set_fragment(None);
        Ok(endpoint)
    }
}

pub fn verify_sha256(bytes: &[u8], expected: &str) -> bool {
    let actual = format!("{:x}", Sha256::digest(bytes));
    actual.eq_ignore_ascii_case(expected.trim())
}

fn encode_segment(value: &str) -> String {
    url_escape(value.trim())
}

fn url_escape(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_aliases_and_hash_contract_are_stable() {
        assert_eq!(
            Target::normalize("macos", "aarch64"),
            Target {
                platform: "darwin".into(),
                architecture: "arm64".into()
            }
        );
        assert_eq!(
            Target::normalize("win32", "x86_64"),
            Target {
                platform: "windows".into(),
                architecture: "amd64".into()
            }
        );
        assert!(verify_sha256(
            b"distribution",
            "93354845030274cd4bf1686abd60ab28ec52e1a792fa6f7ccb9cbd0ddff53d12"
        ));
    }

    #[test]
    fn update_payload_matches_distribution_contract() {
        let update: Update = serde_json::from_str(r#"{"release":{"id":"rel","version":"2.0.0","sha256":"abc"},"updateAvailable":true,"access":{"state":"ready","canDownload":true}}"#).unwrap();
        assert!(update.update_available);
        assert_eq!(update.access.state, "ready");
    }

    #[test]
    fn public_update_payload_and_software_page_are_stable() {
        let update: PublicUpdate = serde_json::from_str(
            r#"{"release":{"id":"rel","version":"2.0.0","releaseNotes":"Changes"},"updateAvailable":true}"#,
        )
        .unwrap();
        assert!(update.update_available);
        assert_eq!(update.release.release_notes, "Changes");
        let client = Client::new("https://apps.yuelili.com").unwrap();
        assert_eq!(
            client.software_page_url("glyphshift").unwrap().as_str(),
            "https://apps.yuelili.com/software/glyphshift"
        );
    }
}
