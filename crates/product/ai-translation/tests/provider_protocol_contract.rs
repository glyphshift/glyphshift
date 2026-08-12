use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiTranslation, CancellationToken,
    CredentialUpdate, CredentialVault, CredentialVaultError, HttpRequest, HttpResponse,
    HttpTransport, HttpTransportError, TranslationItem, TranslationJobStatus,
    TranslationPlanRequest,
};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[derive(Clone, Default)]
struct MemoryVault(Arc<Mutex<BTreeMap<Box<str>, Box<str>>>>);

impl CredentialVault for MemoryVault {
    fn replace(&self, credential_ref: &str, secret: &str) -> Result<(), CredentialVaultError> {
        self.0
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .insert(credential_ref.into(), secret.into());
        Ok(())
    }

    fn contains(&self, credential_ref: &str) -> Result<bool, CredentialVaultError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .contains_key(credential_ref))
    }

    fn delete(&self, credential_ref: &str) -> Result<(), CredentialVaultError> {
        self.0
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .remove(credential_ref);
        Ok(())
    }

    fn expose(&self, credential_ref: &str) -> Result<Box<str>, CredentialVaultError> {
        self.0
            .lock()
            .map_err(|_| CredentialVaultError::Unavailable)?
            .get(credential_ref)
            .cloned()
            .ok_or(CredentialVaultError::Missing)
    }
}

struct RecordingTransport {
    requests: Arc<Mutex<Vec<HttpRequest>>>,
    response: HttpResponse,
}

struct SequencedTransport {
    requests: Arc<Mutex<Vec<HttpRequest>>>,
    responses: Mutex<VecDeque<HttpResponse>>,
}

impl HttpTransport for SequencedTransport {
    fn send(
        &self,
        request: HttpRequest,
        _cancellation: &CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError> {
        self.requests.lock().expect("request capture").push(request);
        self.responses
            .lock()
            .expect("response sequence")
            .pop_front()
            .ok_or(HttpTransportError::InvalidResponse)
    }
}

impl HttpTransport for RecordingTransport {
    fn send(
        &self,
        request: HttpRequest,
        _cancellation: &CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError> {
        self.requests.lock().expect("request capture").push(request);
        Ok(self.response.clone())
    }
}

struct ProtocolCase {
    protocol: AiProviderProtocol,
    endpoint_suffix: &'static str,
    request_marker: &'static str,
    auth_header: &'static str,
    response: &'static str,
}

#[test]
fn first_release_protocols_use_distinct_wire_shapes_and_decode_structured_results() {
    let cases = [
        ProtocolCase {
            protocol: AiProviderProtocol::OpenAiResponses,
            endpoint_suffix: "/responses",
            request_marker: "text",
            auth_header: "authorization",
            response: r#"{"output":[{"content":[{"type":"output_text","text":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"}]}]}"#,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::OpenAiChatCompletions,
            endpoint_suffix: "/chat/completions",
            request_marker: "response_format",
            auth_header: "authorization",
            response: r#"{"choices":[{"message":{"content":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"}}]}"#,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::OpenAiCompatible,
            endpoint_suffix: "/chat/completions",
            request_marker: "response_format",
            auth_header: "authorization",
            response: r#"{"choices":[{"message":{"content":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"}}]}"#,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::AnthropicMessages,
            endpoint_suffix: "/v1/messages",
            request_marker: "output_config",
            auth_header: "x-api-key",
            response: r#"{"content":[{"type":"text","text":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"}]}"#,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::GeminiGenerateContent,
            endpoint_suffix: "/models/synthetic-model:generateContent",
            request_marker: "generationConfig",
            auth_header: "x-goog-api-key",
            response: r#"{"candidates":[{"content":{"parts":[{"text":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"}]}}]}"#,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::OllamaChat,
            endpoint_suffix: "/chat",
            request_marker: "format",
            auth_header: "authorization",
            response: r#"{"message":{"role":"assistant","content":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"},"done":true}"#,
        },
    ];

    for (index, case) in cases.into_iter().enumerate() {
        let root = tempdir().expect("provider profile root");
        let mut profiles = AiProfileCatalog::open(root.path(), Box::new(MemoryVault::default()))
            .expect("open profiles");
        let profile_id = format!("profile.{index}");
        profiles
            .save_profile(
                AiProfileDraft::new(
                    profile_id.clone(),
                    "Synthetic Provider",
                    case.protocol,
                    "synthetic-model",
                )
                .with_base_url("https://provider.invalid/gateway")
                .with_credential(CredentialUpdate::replace("synthetic-provider-secret")),
            )
            .expect("save provider profile");
        let profile = profiles
            .resolve_profile(&profile_id)
            .expect("resolve provider profile");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let transport = Arc::new(RecordingTransport {
            requests: requests.clone(),
            response: HttpResponse::json(200, case.response),
        });
        let mut translation = AiTranslation::new();
        translation.register_http_provider(case.protocol, transport);
        let plan = translation
            .plan_translation(TranslationPlanRequest::new(
                "dictionary.pending",
                1,
                "en-US",
                "zh-CN",
                [TranslationItem::untranslated("save", "Save")],
            ))
            .expect("plan protocol request");
        let job_id = translation
            .start_translation(plan.token(), profile)
            .expect("start protocol request");
        let deadline = Instant::now() + Duration::from_secs(2);
        let completed = loop {
            let snapshot = translation
                .translation_job(&job_id)
                .expect("query protocol job");
            if snapshot.status().is_terminal() {
                break snapshot;
            }
            assert!(Instant::now() < deadline, "protocol job did not finish");
            std::thread::yield_now();
        };

        assert_eq!(completed.status(), TranslationJobStatus::Completed);
        assert_eq!(completed.results()[0].translation(), "保存");
        let captured = requests.lock().expect("captured request");
        let request = captured.first().expect("one captured request");
        assert!(request.url().ends_with(case.endpoint_suffix));
        assert_eq!(
            request.header(case.auth_header),
            Some(if case.auth_header == "authorization" {
                "Bearer synthetic-provider-secret"
            } else {
                "synthetic-provider-secret"
            })
        );
        let body: serde_json::Value =
            serde_json::from_slice(request.body()).expect("provider request JSON");
        assert!(
            body.get(case.request_marker).is_some(),
            "missing protocol marker"
        );
    }
}

#[test]
fn remote_plain_http_endpoint_never_receives_a_profile_credential() {
    let root = tempdir().expect("plain HTTP profile root");
    let mut profiles = AiProfileCatalog::open(root.path(), Box::new(MemoryVault::default()))
        .expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.insecure",
                "Insecure Gateway",
                AiProviderProtocol::OpenAiCompatible,
                "synthetic-model",
            )
            .with_base_url("http://gateway.invalid/v1")
            .with_credential(CredentialUpdate::replace("synthetic-provider-secret")),
        )
        .expect("save insecure profile metadata");
    let profile = profiles
        .resolve_profile("profile.insecure")
        .expect("resolve insecure profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RecordingTransport {
        requests: requests.clone(),
        response: HttpResponse::json(200, r#"{"choices":[]}"#),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::OpenAiCompatible, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("save", "Save")],
        ))
        .expect("plan insecure request");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start insecure request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let failed = loop {
        let snapshot = translation
            .translation_job(&job_id)
            .expect("query insecure job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "insecure job did not finish");
        std::thread::yield_now();
    };

    assert_eq!(failed.status(), TranslationJobStatus::CompletedWithFailures);
    assert_eq!(
        failed.errors()[0].category(),
        glyphshift_ai_translation::ProviderErrorCategory::InvalidRequest
    );
    assert!(requests.lock().expect("request capture").is_empty());
}

#[test]
fn retryable_rate_limit_response_is_retried_before_the_batch_fails() {
    let root = tempdir().expect("retry profile root");
    let mut profiles = AiProfileCatalog::open(root.path(), Box::new(MemoryVault::default()))
        .expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.retry",
                "Retry Provider",
                AiProviderProtocol::OpenAiChatCompletions,
                "synthetic-model",
            )
            .with_credential(CredentialUpdate::replace("synthetic-provider-secret")),
        )
        .expect("save retry profile");
    let profile = profiles
        .resolve_profile("profile.retry")
        .expect("resolve retry profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(SequencedTransport {
        requests: requests.clone(),
        responses: Mutex::new(VecDeque::from([
            HttpResponse::new(429, [("retry-after-ms", "0")], b"{}"),
            HttpResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"{\"translations\":[{\"item_id\":\"save\",\"text\":\"保存\"}]}"}}]}"#,
            ),
        ])),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::OpenAiChatCompletions, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("save", "Save")],
        ))
        .expect("plan retry request");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start retry request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation
            .translation_job(&job_id)
            .expect("query retry job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "retry job did not finish");
        std::thread::yield_now();
    };

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.results()[0].translation(), "保存");
    assert_eq!(requests.lock().expect("request capture").len(), 2);
}
