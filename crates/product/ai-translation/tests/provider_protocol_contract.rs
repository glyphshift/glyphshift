use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiReasoningEffort, AiTranslation,
    CancellationToken, CredentialUpdate, CredentialVault, CredentialVaultError, HttpRequest,
    HttpResponse, HttpTransport, HttpTransportError, TranslationItem, TranslationJobStatus,
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
    expected_reasoning_tokens: u64,
    expected_cached_input_tokens: u64,
}

#[test]
fn first_release_protocols_use_distinct_wire_shapes_and_decode_structured_results() {
    let cases = [
        ProtocolCase {
            protocol: AiProviderProtocol::OpenAiResponses,
            endpoint_suffix: "/responses",
            request_marker: "text",
            auth_header: "authorization",
            response: r#"{"output":[{"content":[{"type":"output_text","text":"{\"translations\":[\"打开\",\"关闭\"]}"}]}],"usage":{"input_tokens":120,"output_tokens":80,"total_tokens":200,"input_tokens_details":{"cached_tokens":20},"output_tokens_details":{"reasoning_tokens":70}}}"#,
            expected_reasoning_tokens: 70,
            expected_cached_input_tokens: 20,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::OpenAiChatCompletions,
            endpoint_suffix: "/chat/completions",
            request_marker: "response_format",
            auth_header: "authorization",
            response: r#"{"choices":[{"message":{"content":"{\"translations\":[\"打开\",\"关闭\"]}"}}],"usage":{"prompt_tokens":120,"completion_tokens":80,"total_tokens":200,"prompt_cache_hit_tokens":20,"completion_tokens_details":{"reasoning_tokens":70}}}"#,
            expected_reasoning_tokens: 70,
            expected_cached_input_tokens: 20,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::OpenAiCompatible,
            endpoint_suffix: "/chat/completions",
            request_marker: "response_format",
            auth_header: "authorization",
            response: r#"{"choices":[{"message":{"content":"{\"translations\":[\"打开\",\"关闭\"]}"}}],"usage":{"prompt_tokens":120,"completion_tokens":80,"total_tokens":200,"prompt_cache_hit_tokens":20,"completion_tokens_details":{"reasoning_tokens":70}}}"#,
            expected_reasoning_tokens: 70,
            expected_cached_input_tokens: 20,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::AnthropicMessages,
            endpoint_suffix: "/v1/messages",
            request_marker: "output_config",
            auth_header: "x-api-key",
            response: r#"{"content":[{"type":"text","text":"{\"translations\":[\"打开\",\"关闭\"]}"}],"usage":{"input_tokens":120,"output_tokens":80,"cache_read_input_tokens":20}}"#,
            expected_reasoning_tokens: 0,
            expected_cached_input_tokens: 20,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::GeminiGenerateContent,
            endpoint_suffix: "/models/synthetic-model:generateContent",
            request_marker: "generationConfig",
            auth_header: "x-goog-api-key",
            response: r#"{"candidates":[{"content":{"parts":[{"text":"{\"translations\":[\"打开\",\"关闭\"]}"}]}}],"usageMetadata":{"promptTokenCount":120,"candidatesTokenCount":80,"thoughtsTokenCount":70,"cachedContentTokenCount":20,"totalTokenCount":200}}"#,
            expected_reasoning_tokens: 70,
            expected_cached_input_tokens: 20,
        },
        ProtocolCase {
            protocol: AiProviderProtocol::OllamaChat,
            endpoint_suffix: "/chat",
            request_marker: "format",
            auth_header: "authorization",
            response: r#"{"message":{"role":"assistant","content":"{\"translations\":[\"打开\",\"关闭\"]}"},"done":true,"prompt_eval_count":120,"eval_count":80}"#,
            expected_reasoning_tokens: 0,
            expected_cached_input_tokens: 0,
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
                [
                    TranslationItem::untranslated("open", "Open"),
                    TranslationItem::untranslated("close", "Close"),
                ],
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
        assert_eq!(completed.results()[0].item_id(), "open");
        assert_eq!(completed.results()[0].translation(), "打开");
        assert_eq!(completed.results()[1].item_id(), "close");
        assert_eq!(completed.results()[1].translation(), "关闭");
        let usage = completed.usage().expect("provider usage");
        assert_eq!(usage.input_tokens(), 120);
        assert_eq!(usage.output_tokens(), 80);
        assert_eq!(usage.reasoning_tokens(), case.expected_reasoning_tokens);
        assert_eq!(
            usage.cached_input_tokens(),
            case.expected_cached_input_tokens
        );
        assert_eq!(usage.total_tokens(), 200);
        assert_eq!(
            completed.batches()[0]
                .usage()
                .expect("batch provider usage"),
            usage
        );
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
        if case.protocol == AiProviderProtocol::OpenAiResponses {
            assert_eq!(
                body.pointer("/reasoning/effort"),
                Some(&serde_json::json!("none")),
                "translation profiles must disable provider reasoning by default"
            );
        }
        if matches!(
            case.protocol,
            AiProviderProtocol::OpenAiChatCompletions | AiProviderProtocol::OpenAiCompatible
        ) {
            assert_eq!(
                body.get("reasoning_effort"),
                Some(&serde_json::json!("none")),
                "translation chat profiles must disable provider reasoning by default"
            );
        }
        let wire_body = String::from_utf8_lossy(request.body());
        assert!(wire_body.contains(r#"\"items\":[\"Open\",\"Close\"]"#));
        assert!(!wire_body.contains("item_id"));
        assert!(!wire_body.contains("protected_tokens"));
        assert!(wire_body.contains(r#""minItems":2"#));
        assert!(wire_body.contains(r#""maxItems":2"#));
        assert!(wire_body.contains("same order"));
    }
}

#[test]
fn deepseek_chat_profiles_map_reasoning_without_fake_low_levels() {
    let cases = [
        (AiReasoningEffort::Disabled, "disabled", None),
        (AiReasoningEffort::Low, "enabled", Some("high")),
        (AiReasoningEffort::Maximum, "enabled", Some("max")),
    ];
    for (index, (reasoning, thinking_type, expected_effort)) in cases.into_iter().enumerate() {
        let root = tempdir().expect("DeepSeek reasoning profile root");
        let mut profiles = AiProfileCatalog::open(root.path(), Box::new(MemoryVault::default()))
            .expect("open profiles");
        let profile_id = format!("profile.deepseek-{index}");
        profiles
            .save_profile(
                AiProfileDraft::new(
                    profile_id.clone(),
                    "DeepSeek",
                    AiProviderProtocol::OpenAiChatCompletions,
                    "deepseek-v4-flash",
                )
                .with_base_url("https://api.deepseek.com")
                .with_reasoning_effort(reasoning)
                .with_credential(CredentialUpdate::replace("synthetic-provider-secret")),
            )
            .expect("save DeepSeek profile");
        let profile = profiles
            .resolve_profile(&profile_id)
            .expect("resolve DeepSeek profile");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let transport = Arc::new(RecordingTransport {
            requests: requests.clone(),
            response: HttpResponse::json(
                200,
                r#"{"choices":[{"message":{"content":"{\"translations\":[\"打开\"]}"}}]}"#,
            ),
        });
        let mut translation = AiTranslation::new();
        translation.register_http_provider(AiProviderProtocol::OpenAiChatCompletions, transport);
        let plan = translation
            .plan_translation(TranslationPlanRequest::new(
                "dictionary.pending",
                1,
                "en-US",
                "zh-CN",
                [TranslationItem::untranslated("open", "Open")],
            ))
            .expect("plan DeepSeek request");
        let job_id = translation
            .start_translation(plan.token(), profile)
            .expect("start DeepSeek request");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = translation.translation_job(&job_id).expect("query job");
            if snapshot.status().is_terminal() {
                assert_eq!(snapshot.status(), TranslationJobStatus::Completed);
                break;
            }
            assert!(Instant::now() < deadline, "DeepSeek request did not finish");
            std::thread::yield_now();
        }
        let captured = requests.lock().expect("captured request");
        let body: serde_json::Value =
            serde_json::from_slice(captured.first().expect("one request").body())
                .expect("request JSON");
        assert_eq!(
            body.pointer("/thinking/type"),
            Some(&serde_json::json!(thinking_type))
        );
        assert_eq!(
            body.get("reasoning_effort")
                .and_then(serde_json::Value::as_str),
            expected_effort
        );
    }
}

#[test]
fn positional_wire_response_rejects_a_translation_count_mismatch() {
    let root = tempdir().expect("provider profile root");
    let mut profiles = AiProfileCatalog::open(root.path(), Box::new(MemoryVault::default()))
        .expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.positional-count",
                "Synthetic Provider",
                AiProviderProtocol::OpenAiChatCompletions,
                "synthetic-model",
            )
            .with_credential(CredentialUpdate::replace("synthetic-provider-secret")),
        )
        .expect("save provider profile");
    let profile = profiles
        .resolve_profile("profile.positional-count")
        .expect("resolve provider profile");
    let transport = Arc::new(RecordingTransport {
        requests: Arc::new(Mutex::new(Vec::new())),
        response: HttpResponse::json(
            200,
            r#"{"choices":[{"message":{"content":"{\"translations\":[\"打开\"]}"}}]}"#,
        ),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::OpenAiChatCompletions, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            1,
            "en-US",
            "zh-CN",
            [
                TranslationItem::untranslated("open", "Open"),
                TranslationItem::untranslated("close", "Close"),
            ],
        ))
        .expect("plan positional request");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start positional request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let failed = loop {
        let snapshot = translation
            .translation_job(&job_id)
            .expect("query positional job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "positional job did not finish");
        std::thread::yield_now();
    };

    assert_eq!(failed.status(), TranslationJobStatus::CompletedWithFailures);
    assert_eq!(
        failed.errors()[0].category(),
        glyphshift_ai_translation::ProviderErrorCategory::MalformedOutput
    );
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
            .with_max_retries(1)
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
                r#"{"choices":[{"message":{"content":"{\"translations\":[\"保存\"]}"}}]}"#,
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
