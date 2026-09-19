use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiReasoningEffort, AiTranslation,
    CancellationToken, CredentialUpdate, HttpRequest, HttpResponse, HttpTransport,
    HttpTransportError, TranslationItem, TranslationJobStatus, TranslationPlanRequest,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::tempdir;

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
fn malformed_translation_text_uses_the_configured_retry_budget() {
    for (budget, recover, expected_status, expected_attempts) in [
        (1, true, TranslationJobStatus::Completed, 2),
        (0, true, TranslationJobStatus::CompletedWithFailures, 1),
        (1, false, TranslationJobStatus::CompletedWithFailures, 2),
    ] {
        let root = tempdir().expect("profile root");
        let mut profiles = AiProfileCatalog::open(root.path()).expect("profiles");
        profiles
            .save_profile(
                AiProfileDraft::new(
                    "profile.format",
                    "Format retry",
                    AiProviderProtocol::OpenAiResponses,
                    "synthetic-model",
                )
                .with_max_retries(budget)
                .with_credential(CredentialUpdate::replace("synthetic-secret")),
            )
            .expect("profile");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let transport = Arc::new(SequencedTransport {
            requests: requests.clone(),
            responses: Mutex::new(VecDeque::from([
                HttpResponse::json(
                    200,
                    r#"{"status":"completed","output":[{"content":[{"type":"output_text","text":"Unable to provide the requested structure."}]}]}"#,
                ),
                HttpResponse::json(
                    200,
                    if recover {
                        r#"{"status":"completed","output":[{"content":[{"type":"output_text","text":"{\"translations\":[\"保存\"]}"}]}]}"#
                    } else {
                        r#"{"status":"completed","output":[{"content":[{"type":"output_text","text":"Still not JSON."}]}]}"#
                    },
                ),
            ])),
        });
        let mut translation = AiTranslation::new();
        translation.register_http_provider(AiProviderProtocol::OpenAiResponses, transport);
        let plan = translation
            .plan_translation(TranslationPlanRequest::new(
                "dictionary.pending",
                1,
                "en-US",
                "zh-CN",
                [TranslationItem::untranslated("save", "Save")],
            ))
            .expect("plan");
        let job = translation
            .start_translation(
                plan.token(),
                profiles
                    .resolve_profile("profile.format")
                    .expect("resolved profile"),
            )
            .expect("job");
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            let snapshot = translation.translation_job(&job).expect("snapshot");
            if snapshot.status().is_terminal() {
                break snapshot;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result.status(), expected_status);
        if expected_status == TranslationJobStatus::Completed {
            assert_eq!(result.results().len(), 1);
            assert_eq!(result.results()[0].translation(), "保存");
        } else {
            assert!(
                result.results().is_empty(),
                "malformed output must never be published"
            );
        }
        assert_eq!(requests.lock().unwrap().len(), expected_attempts);
    }
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
        let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
        let profile_id = format!("profile.{index}");
        profiles
            .save_profile(
                AiProfileDraft::new(
                    profile_id.clone(),
                    "Synthetic Provider",
                    case.protocol,
                    "synthetic-model",
                )
                .with_translation_prompt("Use concise nautical terminology.")
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
        assert!(wire_body.contains(r#"\"source\":\"Open\""#));
        assert!(wire_body.contains(r#"\"source\":\"Close\""#));
        assert!(wire_body.contains(r#"\"context\":null"#));
        assert!(wire_body.contains(r#"\"disambiguation\":null"#));
        assert!(!wire_body.contains("item_id"));
        assert!(!wire_body.contains("protected_tokens"));
        assert!(wire_body.contains(r#""minItems":2"#));
        assert!(wire_body.contains(r#""maxItems":2"#));
        assert!(wire_body.contains("same order"));
        assert!(wire_body.contains("Use concise nautical terminology."));
    }
}

#[test]
fn microsoft_translator_nmt_uses_native_wire_shape_region_and_character_usage() {
    let root = tempdir().expect("Microsoft Translator profile root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.microsoft-nmt",
                "Microsoft Translator",
                AiProviderProtocol::MicrosoftTranslator,
                "",
            )
            .with_provider_region("eastasia")
            .with_credential(CredentialUpdate::replace("synthetic-translator-key")),
        )
        .expect("save Microsoft Translator profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RecordingTransport {
        requests: requests.clone(),
        response: HttpResponse::new(
            200,
            [
                ("content-type", "application/json"),
                ("sourceCharactersCharged", "9"),
            ],
            r#"{"value":[{"translations":[{"language":"zh-Hans","sourceCharacters":4,"text":"打开"}]},{"translations":[{"language":"zh-Hans","sourceCharacters":5,"text":"关闭"}]}]}"#.as_bytes(),
        ),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::MicrosoftTranslator, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.microsoft-nmt",
            1,
            "en-US",
            "zh-CN",
            [
                TranslationItem::untranslated("open", "Open"),
                TranslationItem::untranslated("close", "Close"),
            ],
        ))
        .expect("plan Microsoft Translator request");
    let job = translation
        .start_translation(
            plan.token(),
            profiles
                .resolve_profile("profile.microsoft-nmt")
                .expect("resolve Microsoft Translator profile"),
        )
        .expect("start Microsoft Translator request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation.translation_job(&job).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.results()[0].translation(), "打开");
    assert_eq!(completed.results()[1].translation(), "关闭");
    let usage = completed.usage().expect("Microsoft Translator usage");
    assert_eq!(usage.source_characters(), 9);
    assert_eq!(usage.total_tokens(), 0);
    let captured = requests.lock().expect("captured request");
    let request = captured.first().expect("one captured request");
    assert_eq!(
        request.url(),
        "https://api.cognitive.microsofttranslator.com/translate?api-version=2026-06-06"
    );
    assert_eq!(
        request.header("ocp-apim-subscription-key"),
        Some("synthetic-translator-key")
    );
    assert_eq!(
        request.header("ocp-apim-subscription-region"),
        Some("eastasia")
    );
    let body: serde_json::Value =
        serde_json::from_slice(request.body()).expect("Microsoft Translator request JSON");
    assert_eq!(body.pointer("/inputs/0/language"), Some(&serde_json::json!("en")));
    assert_eq!(
        body.pointer("/inputs/0/targets/0/language"),
        Some(&serde_json::json!("zh-Hans"))
    );
    assert!(body.pointer("/inputs/0/targets/0/deploymentName").is_none());
    assert_eq!(body.pointer("/inputs/1/text"), Some(&serde_json::json!("Close")));
}

#[test]
fn libretranslate_uses_batch_wire_shape_without_requiring_a_key() {
    let root = tempdir().expect("LibreTranslate profile root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.libretranslate",
            "LibreTranslate",
            AiProviderProtocol::LibreTranslate,
            "",
        ))
        .expect("save LibreTranslate profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RecordingTransport {
        requests: requests.clone(),
        response: HttpResponse::new(
            200,
            [("content-type", "application/json")],
            r#"{"translatedText":["打开","关闭"]}"#.as_bytes(),
        ),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::LibreTranslate, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.libretranslate",
            1,
            "en-US",
            "zh-CN",
            [
                TranslationItem::untranslated("open", "Open"),
                TranslationItem::untranslated("close", "Close"),
            ],
        ))
        .expect("plan LibreTranslate request");
    let job = translation
        .start_translation(
            plan.token(),
            profiles
                .resolve_profile("profile.libretranslate")
                .expect("resolve LibreTranslate profile"),
        )
        .expect("start LibreTranslate request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation.translation_job(&job).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.results()[0].translation(), "打开");
    assert_eq!(completed.results()[1].translation(), "关闭");
    assert!(completed.usage().is_none());
    let captured = requests.lock().expect("captured request");
    let request = captured.first().expect("one captured request");
    assert_eq!(request.url(), "http://127.0.0.1:5000/translate");
    let body: serde_json::Value =
        serde_json::from_slice(request.body()).expect("LibreTranslate request JSON");
    assert_eq!(body.get("q"), Some(&serde_json::json!(["Open", "Close"])));
    assert_eq!(body.get("source"), Some(&serde_json::json!("en")));
    assert_eq!(body.get("target"), Some(&serde_json::json!("zh")));
    assert_eq!(body.get("format"), Some(&serde_json::json!("text")));
    assert!(body.get("api_key").is_none());
}

#[test]
fn google_translate_sends_multiple_sources_in_one_request() {
    let root = tempdir().expect("Google Translate profile root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.google-translate",
                "Google Translate",
                AiProviderProtocol::GoogleTranslate,
                "",
            )
            .with_credential(CredentialUpdate::replace("synthetic-google-key")),
        )
        .expect("save Google Translate profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RecordingTransport {
        requests: requests.clone(),
        response: HttpResponse::json(
            200,
            r#"{"data":{"translations":[{"translatedText":"打开"},{"translatedText":"关闭"}]}}"#,
        ),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::GoogleTranslate, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.google-translate",
            1,
            "en-US",
            "zh-CN",
            [
                TranslationItem::untranslated("open", "Open"),
                TranslationItem::untranslated("close", "Close"),
            ],
        ))
        .expect("plan Google Translate request");
    let job = translation
        .start_translation(
            plan.token(),
            profiles
                .resolve_profile("profile.google-translate")
                .expect("resolve Google Translate profile"),
        )
        .expect("start Google Translate request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation.translation_job(&job).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.results()[0].translation(), "打开");
    assert_eq!(completed.results()[1].translation(), "关闭");
    let captured = requests.lock().expect("captured request");
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    assert!(request.url().starts_with("https://translation.googleapis.com/language/translate/v2?"));
    assert!(request.url().contains("key=synthetic-google-key"));
    let body: serde_json::Value = serde_json::from_slice(request.body()).expect("Google Translate request JSON");
    assert_eq!(body.get("q"), Some(&serde_json::json!(["Open", "Close"])));
    assert_eq!(body.get("source"), Some(&serde_json::json!("en")));
    assert_eq!(body.get("target"), Some(&serde_json::json!("zh-CN")));
}

#[test]
fn baidu_translate_batches_sources_into_one_signed_request() {
    let root = tempdir().expect("Baidu Translate profile root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.baidu-translate",
                "Baidu Translate",
                AiProviderProtocol::BaiduTranslate,
                "",
            )
            .with_provider_app_id("synthetic-baidu-app")
            .with_credential(CredentialUpdate::replace("synthetic-baidu-secret")),
        )
        .expect("save Baidu Translate profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RecordingTransport {
        requests: requests.clone(),
        response: HttpResponse::json(
            200,
            r#"{"from":"en","to":"zh","trans_result":[{"src":"Open","dst":"打开"},{"src":"Close","dst":"关闭"}]}"#,
        ),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::BaiduTranslate, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.baidu-translate",
            1,
            "en-US",
            "zh-CN",
            [
                TranslationItem::untranslated("open", "Open"),
                TranslationItem::untranslated("close", "Close"),
            ],
        ))
        .expect("plan Baidu Translate request");
    let job = translation
        .start_translation(
            plan.token(),
            profiles
                .resolve_profile("profile.baidu-translate")
                .expect("resolve Baidu Translate profile"),
        )
        .expect("start Baidu Translate request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation.translation_job(&job).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.results()[0].translation(), "打开");
    assert_eq!(completed.results()[1].translation(), "关闭");
    let captured = requests.lock().expect("captured request");
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    assert_eq!(request.url(), "https://fanyi-api.baidu.com/api/trans/vip/translate");
    assert_eq!(request.header("content-type"), Some("application/x-www-form-urlencoded"));
    let body = String::from_utf8_lossy(request.body());
    assert!(body.contains("q=Open%0AClose"));
    assert!(body.contains("from=en"));
    assert!(body.contains("to=zh"));
    assert!(body.contains("appid=synthetic-baidu-app"));
    assert!(body.contains("sign="));
    assert!(!body.contains("synthetic-baidu-secret"));
}

#[test]
fn microsoft_translator_llm_uses_custom_endpoint_deployment_and_token_usage() {
    let root = tempdir().expect("Microsoft Translator LLM profile root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.microsoft-llm",
                "Microsoft Translator LLM",
                AiProviderProtocol::MicrosoftTranslator,
                "my-gpt-deployment",
            )
            .with_base_url("https://synthetic.cognitiveservices.azure.com")
            .with_credential(CredentialUpdate::replace("synthetic-translator-key")),
        )
        .expect("save Microsoft Translator LLM profile");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RecordingTransport {
        requests: requests.clone(),
        response: HttpResponse::new(
            200,
            [
                ("content-type", "application/json"),
                ("sourceCharactersCharged", "72"),
                ("sourceTokensCharged", "26"),
                ("targetTokensCharged", "16"),
            ],
            r#"{"value":[{"translations":[{"language":"zh-Hans","instructionTokens":12,"sourceTokens":14,"targetTokens":16,"text":"医生下周一有空。"}]}]}"#.as_bytes(),
        ),
    });
    let mut translation = AiTranslation::new();
    translation.register_http_provider(AiProviderProtocol::MicrosoftTranslator, transport);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.microsoft-llm",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated(
                "doctor",
                "Doctor is available next Monday.",
            )],
        ))
        .expect("plan Microsoft Translator LLM request");
    let job = translation
        .start_translation(
            plan.token(),
            profiles
                .resolve_profile("profile.microsoft-llm")
                .expect("resolve Microsoft Translator LLM profile"),
        )
        .expect("start Microsoft Translator LLM request");
    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation.translation_job(&job).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    let usage = completed.usage().expect("Microsoft Translator LLM usage");
    assert_eq!(usage.source_characters(), 72);
    assert_eq!(usage.input_tokens(), 26);
    assert_eq!(usage.output_tokens(), 16);
    assert_eq!(usage.total_tokens(), 42);
    let captured = requests.lock().expect("captured request");
    let request = captured.first().expect("one captured request");
    assert_eq!(
        request.url(),
        "https://synthetic.cognitiveservices.azure.com/translator/text/translate?api-version=2026-06-06"
    );
    let body: serde_json::Value =
        serde_json::from_slice(request.body()).expect("Microsoft Translator LLM request JSON");
    assert_eq!(
        body.pointer("/inputs/0/targets/0/deploymentName"),
        Some(&serde_json::json!("my-gpt-deployment"))
    );
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
        let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
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
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
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
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
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
    let mut profiles = AiProfileCatalog::open(root.path()).expect("open profiles");
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

#[test]
fn connection_checks_bound_timeout_and_do_not_retry() {
    for (scope, timeout, attempts) in [("connection:test", 30_000, 1), ("dictionary:test", 1_800_000, 3)] {
        let root = tempdir().unwrap();
        let mut profiles = AiProfileCatalog::open(root.path()).unwrap();
        profiles.save_profile(AiProfileDraft::new("test", "Test", AiProviderProtocol::OllamaChat, "synthetic-model").with_timeout_ms(1_800_000).with_max_retries(2)).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut translation = AiTranslation::new();
        translation.register_http_provider(AiProviderProtocol::OllamaChat, Arc::new(RecordingTransport { requests: requests.clone(), response: HttpResponse::json(200, r#"{"message":{"content":"not structured output"}}"#) }));
        let plan = translation.plan_translation(TranslationPlanRequest::new(scope, 1, "en-US", "zh-CN", [TranslationItem::untranslated("one", "Open")])).unwrap();
        let job = translation.start_translation(plan.token(), profiles.resolve_profile("test").unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !translation.translation_job(&job).unwrap().status().is_terminal() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let captured = requests.lock().unwrap();
        assert_eq!(captured.len(), attempts);
        assert!(captured.iter().all(|request| request.timeout_ms() == timeout));
        assert_eq!(profiles.resolve_profile("test").unwrap().timeout_ms(), 1_800_000);
    }
}
