use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiTranslation, CancellationToken,
    ProviderBatchResult, ProviderTranslation, ProviderUsage, TranslationItem, TranslationJobId,
    TranslationPlanRequest, TranslationProvider, TranslationRunHistory,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::tempdir;

struct UsageProvider;

struct BlockingProvider {
    entered: Sender<()>,
    release: Mutex<Receiver<()>>,
}

impl TranslationProvider for BlockingProvider {
    fn translate(
        &self,
        request: &glyphshift_ai_translation::ProviderRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, glyphshift_ai_translation::ProviderError> {
        self.entered.send(()).expect("report provider entry");
        self.release
            .lock()
            .expect("release lock")
            .recv()
            .expect("release provider");
        Ok(ProviderBatchResult::new(request.items().iter().map(
            |item| ProviderTranslation::new(item.item_id(), "合成译文"),
        )))
    }
}

impl TranslationProvider for UsageProvider {
    fn translate(
        &self,
        request: &glyphshift_ai_translation::ProviderRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, glyphshift_ai_translation::ProviderError> {
        Ok(ProviderBatchResult::new(
            request
                .items()
                .iter()
                .map(|item| ProviderTranslation::new(item.item_id(), "合成译文")),
        )
        .with_usage(ProviderUsage::new(120, 80, 70, 20, 200)))
    }
}

fn terminal_snapshot(
    translation: &AiTranslation,
    job_id: &TranslationJobId,
) -> glyphshift_ai_translation::TranslationJobSnapshot {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let snapshot = translation
            .translation_job(job_id)
            .expect("query translation job");
        if snapshot.status().is_terminal() {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "translation job did not finish");
        std::thread::yield_now();
    }
}

#[test]
fn terminal_run_records_survive_restart_without_copying_translation_content() {
    let root = tempdir().expect("history root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("profile catalog");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local translator",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve profile");
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OllamaChat, Arc::new(UsageProvider));
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "probe:private-run-id",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated(
                "private-item-id",
                "Private source text",
            )],
        ))
        .expect("plan translation");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start translation");
    let snapshot = terminal_snapshot(&translation, &job_id);

    let mut history = TranslationRunHistory::open(root.path()).expect("open history");
    history.record(&snapshot).expect("record terminal job");
    history.record(&snapshot).expect("deduplicate terminal job");
    assert_eq!(history.records().len(), 1);
    let record = &history.records()[0];
    assert_eq!(record.scope_kind(), "probe");
    assert_eq!(record.profile_name(), "Local translator");
    assert_eq!(record.model_id(), "synthetic-model");
    assert_eq!(record.total_count(), 1);
    assert_eq!(record.request_attempts(), 1);
    assert_eq!(record.usage().expect("record usage").reasoning_tokens(), 70);

    let persisted = std::fs::read_to_string(root.path().join("ai-translation-history.json"))
        .expect("read persisted history");
    assert!(!persisted.contains("Private source text"));
    assert!(!persisted.contains("合成译文"));
    assert!(!persisted.contains("private-item-id"));
    assert!(!persisted.contains("private-run-id"));
    assert!(!persisted.contains("baseUrl"));
    assert!(persisted.contains("glyphshift.ai-translation-history/2"));

    let mut artifact: serde_json::Value =
        serde_json::from_str(&persisted).expect("parse history artifact");
    artifact["unknownFutureField"] = serde_json::json!(true);
    let mut invalid_record = artifact["records"][0].clone();
    invalid_record["recordId"] = serde_json::json!(42);
    artifact["records"]
        .as_array_mut()
        .expect("history records")
        .push(invalid_record);
    std::fs::write(
        root.path().join("ai-translation-history.json"),
        serde_json::to_vec(&artifact).expect("encode partially invalid history"),
    )
    .expect("write partially invalid history");

    let mut reopened = TranslationRunHistory::open(root.path()).expect("reopen history");
    assert_eq!(reopened.records(), history.records());
    reopened.clear().expect("clear history");
    assert!(reopened.records().is_empty());
}

#[test]
fn an_active_checkpoint_becomes_an_interrupted_history_record_after_restart() {
    let root = tempdir().expect("interrupted history root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("profile catalog");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local translator",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save profile");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(BlockingProvider {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        }),
    );
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary:private-id",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated(
                "private-item",
                "Private text",
            )],
        ))
        .expect("plan task");
    let job_id = translation
        .start_translation(
            plan.token(),
            profiles
                .resolve_profile("profile.local")
                .expect("resolve profile"),
        )
        .expect("start task");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("provider starts");
    let snapshot = translation
        .translation_job(&job_id)
        .expect("running snapshot");
    let mut history = TranslationRunHistory::open(root.path()).expect("open history");
    history
        .checkpoint(&snapshot)
        .expect("checkpoint running task");
    drop(history);

    let reopened = TranslationRunHistory::open(root.path()).expect("recover history");
    assert_eq!(reopened.records().len(), 1);
    assert_eq!(
        reopened.records()[0].status(),
        glyphshift_ai_translation::TranslationJobStatus::Interrupted
    );
    let persisted = std::fs::read_to_string(root.path().join("ai-translation-history.json"))
        .expect("read history");
    assert!(!persisted.contains("Private text"));
    assert!(!persisted.contains("private-item"));
    assert!(!persisted.contains("private-id"));
    release_tx.send(()).expect("release provider");
    let _ = terminal_snapshot(&translation, &job_id);
}

#[test]
fn run_history_keeps_only_the_latest_one_hundred_terminal_jobs() {
    let root = tempdir().expect("bounded history root");
    let mut profiles = AiProfileCatalog::open(root.path()).expect("profile catalog");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local translator",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save profile");
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OllamaChat, Arc::new(UsageProvider));
    let mut history = TranslationRunHistory::open(root.path()).expect("open history");

    for index in 0..105 {
        let plan = translation
            .plan_translation(TranslationPlanRequest::new(
                "dictionary:bounded",
                1,
                "en-US",
                "zh-CN",
                [TranslationItem::untranslated(
                    format!("item-{index}"),
                    format!("Synthetic source {index}"),
                )],
            ))
            .expect("plan translation");
        let profile = profiles
            .resolve_profile("profile.local")
            .expect("resolve profile");
        let job_id = translation
            .start_translation(plan.token(), profile)
            .expect("start translation");
        let snapshot = terminal_snapshot(&translation, &job_id);
        history.record(&snapshot).expect("record terminal job");
    }

    assert_eq!(history.records().len(), 100);
    assert_eq!(
        TranslationRunHistory::open(root.path())
            .expect("reopen bounded history")
            .records()
            .len(),
        100
    );
}

#[test]
fn malformed_history_opens_empty_and_preserves_the_original_file() {
    let root = tempdir().expect("history recovery root");
    std::fs::write(root.path().join("ai-translation-history.json"), b"{")
        .expect("write malformed history");

    let history = TranslationRunHistory::open(root.path()).expect("open malformed history safely");

    assert!(history.records().is_empty());
    assert!(root.path().join("ai-translation-history.json").exists());
    assert!(root
        .path()
        .join("ai-translation-history.invalid.json")
        .exists());
}
