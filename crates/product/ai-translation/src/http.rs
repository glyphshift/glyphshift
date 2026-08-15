use crate::{
    AiProviderProtocol, AiTranslation, CancellationToken, ProviderBatchResult, ProviderError,
    ProviderErrorCategory, ProviderRequest, ProviderTranslation, TranslationProvider,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

pub struct HttpRequest {
    method: &'static str,
    url: Box<str>,
    headers: Vec<(Box<str>, Box<str>)>,
    body: Vec<u8>,
    timeout_ms: u64,
}

impl HttpRequest {
    #[must_use]
    pub const fn method(&self) -> &str {
        self.method
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_ref())
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[derive(Clone)]
pub struct HttpResponse {
    status: u16,
    headers: Vec<(Box<str>, Box<str>)>,
    body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(
        status: u16,
        headers: impl IntoIterator<Item = (impl Into<Box<str>>, impl Into<Box<str>>)>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn json(status: u16, body: &str) -> Self {
        Self::new(
            status,
            [("content-type", "application/json")],
            body.as_bytes(),
        )
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_ref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpTransportError {
    Network,
    Timeout,
    Cancelled,
    InvalidRequest,
    InvalidResponse,
}

pub trait HttpTransport: Send + Sync + 'static {
    fn send(
        &self,
        request: HttpRequest,
        cancellation: &CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError>;
}

#[derive(Clone, Copy, Default)]
pub struct ReqwestHttpTransport;

impl HttpTransport for ReqwestHttpTransport {
    fn send(
        &self,
        request: HttpRequest,
        cancellation: &CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError> {
        if cancellation.is_cancelled() {
            return Err(HttpTransportError::Cancelled);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| HttpTransportError::Network)?;
        let request_future = async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_millis(request.timeout_ms))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| HttpTransportError::InvalidRequest)?;
            let mut outgoing = client.post(request.url.as_ref());
            for (name, value) in request.headers {
                outgoing = outgoing.header(name.as_ref(), value.as_ref());
            }
            let response = outgoing.body(request.body).send().await.map_err(|error| {
                if error.is_timeout() {
                    HttpTransportError::Timeout
                } else if error.is_builder() {
                    HttpTransportError::InvalidRequest
                } else {
                    HttpTransportError::Network
                }
            })?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (Box::<str>::from(name.as_str()), Box::<str>::from(value)))
                })
                .collect::<Vec<_>>();
            let body = response
                .bytes()
                .await
                .map_err(|_| HttpTransportError::InvalidResponse)?
                .to_vec();
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        };
        let mut request_future = Box::pin(request_future);
        loop {
            if cancellation.is_cancelled() {
                return Err(HttpTransportError::Cancelled);
            }
            match runtime.block_on(async {
                tokio::time::timeout(Duration::from_millis(25), &mut request_future).await
            }) {
                Ok(response) => {
                    if cancellation.is_cancelled() {
                        return Err(HttpTransportError::Cancelled);
                    }
                    return response;
                }
                Err(_) => continue,
            }
        }
    }
}

struct HttpTranslationProvider {
    protocol: AiProviderProtocol,
    transport: Arc<dyn HttpTransport>,
}

impl AiTranslation {
    pub fn register_http_provider(
        &mut self,
        protocol: AiProviderProtocol,
        transport: Arc<dyn HttpTransport>,
    ) {
        self.register_provider(
            protocol,
            Arc::new(HttpTranslationProvider {
                protocol,
                transport,
            }),
        );
    }

    pub fn register_first_release_http_providers(&mut self, transport: Arc<dyn HttpTransport>) {
        for protocol in [
            AiProviderProtocol::OpenAiResponses,
            AiProviderProtocol::OpenAiChatCompletions,
            AiProviderProtocol::OpenAiCompatible,
            AiProviderProtocol::AnthropicMessages,
            AiProviderProtocol::GeminiGenerateContent,
            AiProviderProtocol::OllamaChat,
        ] {
            self.register_http_provider(protocol, transport.clone());
        }
    }
}

impl TranslationProvider for HttpTranslationProvider {
    fn translate(
        &self,
        request: &ProviderRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        let outgoing = build_request(self.protocol, request)?;
        let response = self
            .transport
            .send(outgoing, cancellation)
            .map_err(transport_error)?;
        decode_response(self.protocol, response)
    }
}

fn build_request(
    protocol: AiProviderProtocol,
    request: &ProviderRequest<'_>,
) -> Result<HttpRequest, ProviderError> {
    let profile = request.profile();
    let base_url = reqwest::Url::parse(profile.base_url())
        .map_err(|_| invalid_request("provider base URL was invalid"))?;
    if !matches!(base_url.scheme(), "http" | "https")
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(invalid_request("provider base URL was invalid"));
    }
    let loopback = matches!(base_url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if base_url.scheme() == "http" && profile.credential().is_some() && !loopback {
        return Err(invalid_request(
            "provider credentials require HTTPS or a loopback endpoint",
        ));
    }
    let schema = translation_schema();
    let input = json!({
        "source_locale": request.source_locale(),
        "target_locale": request.target_locale(),
        "items": request.items().iter().map(|item| json!({
            "item_id": item.item_id(),
            "source": item.source(),
            "protected_tokens": item.protected_tokens(),
        })).collect::<Vec<_>>(),
    });
    let user_text = serde_json::to_string(&input)
        .map_err(|_| invalid_request("could not encode translation input"))?;
    let system_text = format!(
        "Translate UI text from {} to {}. Return only the requested JSON object. Preserve every item_id and protected token exactly. Do not add or omit items.",
        request.source_locale(),
        request.target_locale()
    );
    let (endpoint, body) = match protocol {
        AiProviderProtocol::OpenAiResponses => (
            "responses".to_owned(),
            json!({
                "model": profile.model_id(),
                "store": false,
                "input": [
                    {"role": "system", "content": [{"type": "input_text", "text": system_text}]},
                    {"role": "user", "content": [{"type": "input_text", "text": user_text}]},
                ],
                "text": {"format": {"type": "json_schema", "name": "translation_batch", "strict": true, "schema": schema}},
            }),
        ),
        AiProviderProtocol::OpenAiChatCompletions | AiProviderProtocol::OpenAiCompatible => (
            "chat/completions".to_owned(),
            json!({
                "model": profile.model_id(),
                "messages": [
                    {"role": "system", "content": system_text},
                    {"role": "user", "content": user_text},
                ],
                "response_format": {"type": "json_schema", "json_schema": {"name": "translation_batch", "strict": true, "schema": schema}},
            }),
        ),
        AiProviderProtocol::AnthropicMessages => (
            "v1/messages".to_owned(),
            json!({
                "model": profile.model_id(),
                "max_tokens": 4096,
                "system": system_text,
                "messages": [{"role": "user", "content": user_text}],
                "output_config": {"format": {"type": "json_schema", "schema": schema}},
            }),
        ),
        AiProviderProtocol::GeminiGenerateContent => (
            format!(
                "models/{}:generateContent",
                encode_path_segment(profile.model_id().trim_start_matches("models/"))
            ),
            json!({
                "systemInstruction": {"parts": [{"text": system_text}]},
                "contents": [{"role": "user", "parts": [{"text": user_text}]}],
                "generationConfig": {"responseMimeType": "application/json", "responseJsonSchema": schema},
            }),
        ),
        AiProviderProtocol::OllamaChat => (
            "chat".to_owned(),
            json!({
                "model": profile.model_id(),
                "stream": false,
                "keep_alive": "5m",
                "messages": [
                    {"role": "system", "content": system_text},
                    {"role": "user", "content": user_text},
                ],
                "format": schema,
            }),
        ),
    };
    let url = format!("{}/{}", profile.base_url().trim_end_matches('/'), endpoint);
    let mut headers = vec![(
        Box::<str>::from("content-type"),
        Box::<str>::from("application/json"),
    )];
    match protocol {
        AiProviderProtocol::AnthropicMessages => {
            let credential = profile
                .credential()
                .ok_or_else(|| invalid_request("missing provider credential"))?;
            headers.push(("x-api-key".into(), credential.into()));
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
        }
        AiProviderProtocol::GeminiGenerateContent => {
            let credential = profile
                .credential()
                .ok_or_else(|| invalid_request("missing provider credential"))?;
            headers.push(("x-goog-api-key".into(), credential.into()));
        }
        AiProviderProtocol::OpenAiResponses | AiProviderProtocol::OpenAiChatCompletions => {
            let credential = profile
                .credential()
                .ok_or_else(|| invalid_request("missing provider credential"))?;
            headers.push((
                "authorization".into(),
                format!("Bearer {credential}").into(),
            ));
        }
        AiProviderProtocol::OpenAiCompatible | AiProviderProtocol::OllamaChat => {
            if let Some(credential) = profile.credential() {
                headers.push((
                    "authorization".into(),
                    format!("Bearer {credential}").into(),
                ));
            }
        }
    }
    let body = serde_json::to_vec(&body)
        .map_err(|_| invalid_request("could not encode provider request"))?;
    Ok(HttpRequest {
        method: "POST",
        url: url.into(),
        headers,
        body,
        timeout_ms: profile.timeout_ms(),
    })
}

fn translation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["translations"],
        "properties": {
            "translations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["item_id", "text"],
                    "properties": {
                        "item_id": {"type": "string"},
                        "text": {"type": "string"},
                    },
                },
            },
        },
    })
}

fn decode_response(
    protocol: AiProviderProtocol,
    response: HttpResponse,
) -> Result<ProviderBatchResult, ProviderError> {
    if !(200..300).contains(&response.status) {
        return Err(http_status_error(&response));
    }
    let value: Value = serde_json::from_slice(&response.body)
        .map_err(|_| malformed_response("provider returned invalid JSON"))?;
    let text = match protocol {
        AiProviderProtocol::OpenAiResponses => value
            .get("output")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|output| {
                output
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .find(|content| content.get("type").and_then(Value::as_str) == Some("output_text"))
            .and_then(|content| content.get("text"))
            .and_then(Value::as_str),
        AiProviderProtocol::OpenAiChatCompletions | AiProviderProtocol::OpenAiCompatible => value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str),
        AiProviderProtocol::AnthropicMessages => value
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|content| content.get("type").and_then(Value::as_str) == Some("text"))
            .and_then(|content| content.get("text"))
            .and_then(Value::as_str),
        AiProviderProtocol::GeminiGenerateContent => value
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(Value::as_str),
        AiProviderProtocol::OllamaChat => value.pointer("/message/content").and_then(Value::as_str),
    }
    .ok_or_else(|| malformed_response("provider response did not contain text output"))?;
    let structured: StructuredTranslations = serde_json::from_str(text)
        .map_err(|_| malformed_response("provider text was not a translation result"))?;
    Ok(ProviderBatchResult::new(
        structured
            .translations
            .into_iter()
            .map(|translation| ProviderTranslation::new(translation.item_id, translation.text)),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredTranslations {
    translations: Vec<StructuredTranslation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredTranslation {
    item_id: Box<str>,
    text: Box<str>,
}

fn transport_error(error: HttpTransportError) -> ProviderError {
    match error {
        HttpTransportError::Network => ProviderError::new(
            ProviderErrorCategory::Network,
            true,
            "provider network failure",
        ),
        HttpTransportError::Timeout => ProviderError::new(
            ProviderErrorCategory::Timeout,
            true,
            "provider request timed out",
        ),
        HttpTransportError::Cancelled => ProviderError::new(
            ProviderErrorCategory::Cancelled,
            false,
            "provider request was cancelled",
        ),
        HttpTransportError::InvalidRequest => ProviderError::new(
            ProviderErrorCategory::InvalidRequest,
            false,
            "provider request was invalid",
        ),
        HttpTransportError::InvalidResponse => ProviderError::new(
            ProviderErrorCategory::ProviderInternal,
            true,
            "provider response could not be read",
        ),
    }
}

fn http_status_error(response: &HttpResponse) -> ProviderError {
    let (category, retryable) = match response.status {
        401 => (ProviderErrorCategory::Authentication, false),
        403 => (ProviderErrorCategory::Permission, false),
        404 => (ProviderErrorCategory::ModelNotFound, false),
        408 => (ProviderErrorCategory::Timeout, true),
        429 => (ProviderErrorCategory::RateLimited, true),
        502 | 503 | 504 | 529 => (ProviderErrorCategory::Overloaded, true),
        400..=499 => (ProviderErrorCategory::InvalidRequest, false),
        _ => (ProviderErrorCategory::ProviderInternal, true),
    };
    ProviderError::new(category, retryable, "provider rejected the request")
        .with_http_status(response.status)
        .with_request_id(
            response
                .header("x-request-id")
                .or_else(|| response.header("request-id")),
        )
        .with_retry_after_ms(
            response
                .header("retry-after-ms")
                .and_then(|value| value.parse().ok())
                .or_else(|| {
                    response
                        .header("retry-after")
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(|seconds| seconds.saturating_mul(1_000))
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn reqwest_transport_aborts_an_in_flight_request_when_cancelled() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed HTTP server");
        let address = listener.local_addr().expect("delayed HTTP address");
        let (entered_tx, entered_rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept delayed request");
            let mut request = [0_u8; 1_024];
            let _ = stream.read(&mut request);
            entered_tx.send(()).expect("report delayed request");
            thread::sleep(Duration::from_secs(2));
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
            );
        });

        let token = CancellationToken::new(Arc::new(AtomicBool::new(false)));
        let worker_token = token.clone();
        let (result_tx, result_rx) = mpsc::channel();
        thread::spawn(move || {
            let result = ReqwestHttpTransport.send(
                HttpRequest {
                    method: "POST",
                    url: format!("http://{address}/translate").into(),
                    headers: vec![("content-type".into(), "application/json".into())],
                    body: b"{}".to_vec(),
                    timeout_ms: 5_000,
                },
                &worker_token,
            );
            result_tx.send(result).expect("report transport result");
        });
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("request reaches delayed server");

        token.cancel();

        assert!(matches!(
            result_rx
                .recv_timeout(Duration::from_millis(500))
                .expect("cancelled transport returns promptly"),
            Err(HttpTransportError::Cancelled)
        ));
    }
}

fn malformed_response(message: &'static str) -> ProviderError {
    ProviderError::new(ProviderErrorCategory::MalformedOutput, false, message)
}

fn invalid_request(message: &'static str) -> ProviderError {
    ProviderError::new(ProviderErrorCategory::InvalidRequest, false, message)
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
