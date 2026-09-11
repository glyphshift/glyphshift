use glyphshift_ai_translation::AiProviderProtocol;
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelListRequest {
    protocol: AiProviderProtocol,
    base_url: String,
    secret: String,
}

fn endpoint(request: &ModelListRequest) -> Result<Url, &'static str> {
    use AiProviderProtocol::*;
    if request.protocol == CodexSubscription {
        return Err("unsupported");
    }
    let mut url = Url::parse(request.base_url.trim()).map_err(|_| "invalid_url")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("invalid_url");
    }
    let base = url.path().trim_end_matches('/');
    let path = match request.protocol {
        AnthropicMessages if !base.ends_with("/v1") => format!("{base}/v1/models"),
        GeminiGenerateContent if base.is_empty() => "/v1beta/models".into(),
        OllamaChat if base.ends_with("/api") => format!("{base}/tags"),
        OllamaChat => format!("{base}/api/tags"),
        _ => format!("{base}/models"),
    };
    url.set_path(&path);
    if request.protocol.credential_required() && request.secret.trim().is_empty() {
        return Err("missing_key");
    }
    Ok(url)
}

fn network_error(error: reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else {
        "network"
    }
}

fn parse_page(
    protocol: AiProviderProtocol,
    value: &Value,
) -> Result<(Vec<String>, Option<String>), &'static str> {
    use AiProviderProtocol::*;
    let array_key = if matches!(protocol, GeminiGenerateContent | OllamaChat) {
        "models"
    } else {
        "data"
    };
    let rows = value
        .get(array_key)
        .and_then(Value::as_array)
        .ok_or("invalid_response")?;
    let mut models = Vec::new();
    for row in rows {
        if protocol == GeminiGenerateContent
            && !row["supportedGenerationMethods"]
                .as_array()
                .is_some_and(|methods| methods.iter().any(|method| method == "generateContent"))
        {
            continue;
        }
        let key = if matches!(protocol, GeminiGenerateContent | OllamaChat) {
            "name"
        } else {
            "id"
        };
        let id = row
            .get(key)
            .and_then(Value::as_str)
            .ok_or("invalid_response")?;
        let id = if protocol == GeminiGenerateContent {
            id.strip_prefix("models/").unwrap_or(id)
        } else {
            id
        };
        if !id.trim().is_empty() && id.len() <= 512 {
            models.push(id.to_string());
        }
    }
    let next = match protocol {
        GeminiGenerateContent => value.get("nextPageToken").and_then(Value::as_str),
        AnthropicMessages if value["has_more"] == true => {
            Some(value["last_id"].as_str().ok_or("invalid_response")?)
        }
        _ => None,
    }
    .filter(|token| !token.is_empty())
    .map(String::from);
    Ok((models, next))
}

#[tauri::command]
pub(crate) async fn desktop_ai_models(
    request: ModelListRequest,
) -> Result<Vec<String>, &'static str> {
    let url = endpoint(&request)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(network_error)?;
    fetch_models(&client, &request, url).await
}

async fn fetch_models(
    client: &Client,
    request: &ModelListRequest,
    url: Url,
) -> Result<Vec<String>, &'static str> {
    use AiProviderProtocol::*;
    let mut models = BTreeSet::new();
    let mut cursor: Option<String> = None;
    let mut cursors = BTreeSet::new();
    let mut total_bytes = 0;
    let started = Instant::now();
    for _ in 0..20 {
        let remaining = Duration::from_secs(30)
            .checked_sub(started.elapsed())
            .ok_or("timeout")?;
        let mut page_url = url.clone();
        if request.protocol == AnthropicMessages {
            page_url.query_pairs_mut().append_pair("limit", "1000");
        }
        if request.protocol == GeminiGenerateContent {
            page_url.query_pairs_mut().append_pair("pageSize", "1000");
        }
        if let Some(ref token) = cursor {
            page_url.query_pairs_mut().append_pair(
                if request.protocol == AnthropicMessages {
                    "after_id"
                } else {
                    "pageToken"
                },
                token,
            );
        }
        let mut http = client
            .get(page_url)
            .timeout(remaining.min(Duration::from_secs(15)));
        let secret = request.secret.trim();
        if !secret.is_empty() {
            http = match request.protocol {
                AnthropicMessages => http.header("x-api-key", secret),
                GeminiGenerateContent => http.header("x-goog-api-key", secret),
                _ => http.bearer_auth(secret),
            };
        }
        if request.protocol == AnthropicMessages {
            http = http.header("anthropic-version", "2023-06-01");
        }
        let mut response = http.send().await.map_err(network_error)?;
        match response.status().as_u16() {
            200 => {}
            401 => return Err("unauthorized"),
            403 => return Err("forbidden"),
            404 | 405 | 501 => return Err("unsupported"),
            429 => return Err("rate_limited"),
            300..=399 => return Err("redirect"),
            _ => return Err("service_error"),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            total_bytes += chunk.len();
            if total_bytes > 8 * 1024 * 1024 {
                return Err("too_large");
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid_response")?;
        let (page, next) = parse_page(request.protocol, &value)?;
        models.extend(page);
        if models.len() > 10_000 {
            return Err("too_large");
        }
        match next {
            Some(token) if cursors.insert(token.clone()) => cursor = Some(token),
            Some(_) => return Err("invalid_response"),
            None => {
                return if models.is_empty() {
                    Err("empty")
                } else {
                    Ok(models.into_iter().collect())
                };
            }
        }
    }
    Err("too_large")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    fn request(protocol: AiProviderProtocol, base_url: &str, secret: &str) -> ModelListRequest {
        ModelListRequest {
            protocol,
            base_url: base_url.into(),
            secret: secret.into(),
        }
    }

    fn server(
        responses: Vec<(u16, Value)>,
    ) -> (String, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 1024];
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = stream.read(&mut chunk).unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                tx.send(String::from_utf8(bytes).unwrap()).unwrap();
                let body = body.to_string();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        (base, rx, handle)
    }

    fn run(request: ModelListRequest) -> Result<Vec<String>, &'static str> {
        let url = endpoint(&request)?;
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        tauri::async_runtime::block_on(fetch_models(&client, &request, url))
    }

    #[test]
    fn model_list_validates_credentials_and_preserves_provider_paths() {
        use AiProviderProtocol::*;
        assert_eq!(
            endpoint(&request(OpenAiResponses, "https://service.example/v1", "")).unwrap_err(),
            "missing_key"
        );
        for address in [
            "file:///models",
            "https://user:secret@service.example",
            "https://service.example?key=secret",
        ] {
            assert_eq!(
                endpoint(&request(OpenAiCompatible, address, "")).unwrap_err(),
                "invalid_url"
            );
        }
        for (protocol, base, path) in [
            (AnthropicMessages, "/v1", "/v1/models"),
            (OllamaChat, "/api", "/api/tags"),
            (OpenAiCompatible, "/openai/v1/", "/openai/v1/models"),
        ] {
            assert_eq!(
                endpoint(&request(
                    protocol,
                    &format!("https://service.example{base}"),
                    "synthetic"
                ))
                .unwrap()
                .path(),
                path
            );
        }
    }

    #[test]
    fn model_list_openai_sorts_deduplicates_and_sends_bearer() {
        let (base, rx, thread) = server(vec![(
            200,
            serde_json::json!({"data":[{"id":"beta"},{"id":"alpha"},{"id":"beta"}]}),
        )]);
        assert_eq!(
            run(request(
                AiProviderProtocol::OpenAiResponses,
                &format!("{base}/v1"),
                "synthetic-key"
            ))
            .unwrap(),
            ["alpha", "beta"]
        );
        let captured = rx.recv().unwrap().to_lowercase();
        assert!(captured.starts_with("get /v1/models "));
        assert!(captured.contains("authorization: bearer synthetic-key"));
        thread.join().unwrap();
    }

    #[test]
    fn model_list_native_providers_paginate_and_filter() {
        use AiProviderProtocol::*;
        let (base, rx, thread) = server(vec![
            (
                200,
                serde_json::json!({"models":[{"name":"models/chat-a","supportedGenerationMethods":["generateContent"]},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}],"nextPageToken":"next token"}),
            ),
            (
                200,
                serde_json::json!({"models":[{"name":"models/chat-b","supportedGenerationMethods":["generateContent"]}]}),
            ),
        ]);
        assert_eq!(
            run(request(GeminiGenerateContent, &base, "synthetic-key")).unwrap(),
            ["chat-a", "chat-b"]
        );
        let first = rx.recv().unwrap().to_lowercase();
        assert!(first.starts_with("get /v1beta/models?pagesize=1000 "));
        assert!(first.contains("x-goog-api-key: synthetic-key"));
        assert!(rx.recv().unwrap().contains("pageToken=next+token"));
        thread.join().unwrap();
        let (base, rx, thread) = server(vec![
            (
                200,
                serde_json::json!({"data":[{"id":"claude-a"}],"has_more":true,"last_id":"claude-a"}),
            ),
            (
                200,
                serde_json::json!({"data":[{"id":"claude-b"}],"has_more":false}),
            ),
        ]);
        assert_eq!(
            run(request(AnthropicMessages, &base, "synthetic-key")).unwrap(),
            ["claude-a", "claude-b"]
        );
        let first = rx.recv().unwrap().to_lowercase();
        assert!(first.contains("x-api-key: synthetic-key"));
        assert!(first.contains("anthropic-version: 2023-06-01"));
        assert!(rx.recv().unwrap().contains("after_id=claude-a"));
        thread.join().unwrap();
        let (base, rx, thread) = server(vec![(
            200,
            serde_json::json!({"models":[{"name":"local:latest"}]}),
        )]);
        assert_eq!(
            run(request(OllamaChat, &base, "")).unwrap(),
            ["local:latest"]
        );
        assert!(!rx.recv().unwrap().to_lowercase().contains("authorization"));
        thread.join().unwrap();
    }

    #[test]
    fn model_list_errors_are_safe_and_actionable() {
        for (status, expected) in [
            (401, "unauthorized"),
            (403, "forbidden"),
            (404, "unsupported"),
            (429, "rate_limited"),
            (302, "redirect"),
            (500, "service_error"),
            (200, "invalid_response"),
        ] {
            let (base, rx, thread) = server(vec![(
                status,
                serde_json::json!({"error":"secret-must-not-escape"}),
            )]);
            assert_eq!(
                run(request(AiProviderProtocol::OpenAiCompatible, &base, "")).unwrap_err(),
                expected
            );
            rx.recv().unwrap();
            thread.join().unwrap();
        }
        assert!(
            parse_page(
                AiProviderProtocol::OpenAiResponses,
                &serde_json::json!({"data":[]})
            )
            .unwrap()
            .0
            .is_empty()
        );
    }
}
