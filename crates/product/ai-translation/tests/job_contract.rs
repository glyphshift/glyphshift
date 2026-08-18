use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiTranslation, CancellationToken,
    CredentialVault, CredentialVaultError, ProviderBatchResult, ProviderError, ProviderRequest,
    ProviderTranslation, TranslationBatchPolicy, TranslationItem, TranslationJobError,
    TranslationJobStatus, TranslationPlanRequest, TranslationProvider,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tempfile::tempdir;

struct EmptyCredentialVault;

impl CredentialVault for EmptyCredentialVault {
    fn replace(&self, _credential_ref: &str, _secret: &str) -> Result<(), CredentialVaultError> {
        Ok(())
    }

    fn contains(&self, _credential_ref: &str) -> Result<bool, CredentialVaultError> {
        Ok(false)
    }

    fn delete(&self, _credential_ref: &str) -> Result<(), CredentialVaultError> {
        Ok(())
    }

    fn expose(&self, _credential_ref: &str) -> Result<Box<str>, CredentialVaultError> {
        Err(CredentialVaultError::Missing)
    }
}

#[test]
fn only_one_non_terminal_translation_job_can_run() {
    let root = tempdir().expect("single task data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save profile");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(DelayedSyntheticProvider {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        }),
    );
    let first = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary:first",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("first", "First")],
        ))
        .expect("plan first task");
    let first_id = translation
        .start_translation(
            first.token(),
            profiles
                .resolve_profile("profile.local")
                .expect("resolve profile"),
        )
        .expect("start first task");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first task enters provider");
    let second = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary:second",
            1,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("second", "Second")],
        ))
        .expect("plan second task");

    assert_eq!(
        translation.start_translation(
            second.token(),
            profiles
                .resolve_profile("profile.local")
                .expect("resolve profile"),
        ),
        Err(TranslationJobError::ActiveJob(first_id.as_str().into()))
    );
    assert_eq!(
        translation
            .active_translation_job()
            .expect("query active job")
            .expect("active job")
            .job_id(),
        first_id.as_str()
    );
    release_tx.send(()).expect("release first task");
    let _ = wait_for_terminal_job(&translation, &first_id);
}

struct ReorderedSyntheticProvider;

impl TranslationProvider for ReorderedSyntheticProvider {
    fn translate(
        &self,
        request: &ProviderRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        Ok(ProviderBatchResult::new(request.items().iter().rev().map(
            |item| {
                let text = match item.source() {
                    "Save" => "保存".to_owned(),
                    "Hello {name}" => "你好 {name}".to_owned(),
                    other => format!("译文：{other}"),
                };
                ProviderTranslation::new(item.item_id(), text)
            },
        )))
    }
}

struct DelayedSyntheticProvider {
    entered: Sender<()>,
    release: Mutex<Receiver<()>>,
}

struct TokenDroppingSyntheticProvider;

#[derive(Default)]
struct RetryOnceSyntheticProvider {
    attempts: AtomicUsize,
}

#[derive(Default)]
struct BatchRecordingSyntheticProvider {
    request_sizes: Mutex<Vec<usize>>,
}

#[derive(Default)]
struct SecondBatchFailsProvider {
    request_sizes: Mutex<Vec<usize>>,
}

impl TranslationProvider for BatchRecordingSyntheticProvider {
    fn translate(
        &self,
        request: &ProviderRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        self.request_sizes
            .lock()
            .expect("record batch size")
            .push(request.items().len());
        Ok(ProviderBatchResult::new(request.items().iter().map(
            |item| ProviderTranslation::new(item.item_id(), format!("译文：{}", item.source())),
        )))
    }
}

impl TranslationProvider for SecondBatchFailsProvider {
    fn translate(
        &self,
        request: &ProviderRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        let mut request_sizes = self.request_sizes.lock().expect("record batch size");
        request_sizes.push(request.items().len());
        let batch_number = request_sizes.len();
        drop(request_sizes);
        if batch_number == 2 {
            return Err(ProviderError::new(
                glyphshift_ai_translation::ProviderErrorCategory::InvalidRequest,
                false,
                "synthetic batch rejection",
            ));
        }
        Ok(ProviderBatchResult::new(request.items().iter().map(
            |item| ProviderTranslation::new(item.item_id(), format!("译文：{}", item.source())),
        )))
    }
}

fn wait_for_terminal_job(
    translation: &AiTranslation,
    job_id: &glyphshift_ai_translation::TranslationJobId,
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
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl TranslationProvider for TokenDroppingSyntheticProvider {
    fn translate(
        &self,
        request: &ProviderRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        Ok(ProviderBatchResult::new([ProviderTranslation::new(
            request.items()[0].item_id(),
            "你好",
        )]))
    }
}

impl TranslationProvider for RetryOnceSyntheticProvider {
    fn translate(
        &self,
        request: &ProviderRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        if self.attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            return Err(ProviderError::new(
                glyphshift_ai_translation::ProviderErrorCategory::Overloaded,
                true,
                "synthetic provider overloaded",
            ));
        }
        Ok(ProviderBatchResult::new(request.items().iter().map(
            |item| ProviderTranslation::new(item.item_id(), format!("译文：{}", item.source())),
        )))
    }
}

impl TranslationProvider for DelayedSyntheticProvider {
    fn translate(
        &self,
        request: &ProviderRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ProviderBatchResult, ProviderError> {
        self.entered.send(()).expect("report provider entry");
        self.release
            .lock()
            .expect("release receiver")
            .recv()
            .expect("release delayed provider");
        Ok(ProviderBatchResult::new(request.items().iter().map(
            |item| ProviderTranslation::new(item.item_id(), "迟到译文"),
        )))
    }
}

#[test]
fn default_batch_policy_sends_at_most_fifty_items_per_json_request() {
    assert_eq!(
        TranslationBatchPolicy::default().max_items_per_request(),
        50
    );
}

#[test]
fn job_validates_provider_output_and_exposes_results_in_plan_order() {
    let root = tempdir().expect("AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save local profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve local profile");

    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(ReorderedSyntheticProvider),
    );
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            3,
            "en-US",
            "zh-CN",
            [
                TranslationItem::untranslated("save", "Save"),
                TranslationItem::untranslated("hello", "Hello {name}"),
            ],
        ))
        .expect("plan job");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start job");

    let deadline = Instant::now() + Duration::from_secs(2);
    let completed = loop {
        let snapshot = translation.translation_job(&job_id).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "synthetic job did not finish");
        std::thread::yield_now();
    };

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(
        (completed.completed_count(), completed.failed_count()),
        (2, 0)
    );
    assert_eq!(
        completed
            .results()
            .iter()
            .map(|result| (result.item_id(), result.translation()))
            .collect::<Vec<_>>(),
        vec![("save", "保存"), ("hello", "你好 {name}")]
    );
}

#[test]
fn completed_job_snapshot_has_a_safe_stable_ipc_shape() {
    let root = tempdir().expect("AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save local profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve local profile");
    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(ReorderedSyntheticProvider),
    );
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            7,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("save", "Save")],
        ))
        .expect("plan translation");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start translation");
    let snapshot = wait_for_terminal_job(&translation, &job_id);

    let value = serde_json::to_value(snapshot).expect("serialize job snapshot");

    assert_eq!(value["jobId"], job_id.as_str());
    assert_eq!(value["planToken"], plan.token());
    assert_eq!(value["snapshotRevision"], 7);
    assert_eq!(value["profileName"], "Local");
    assert_eq!(value["protocol"], "ollama_chat");
    assert_eq!(value["modelId"], "synthetic-model");
    assert!(value["startedAtMs"].is_number());
    assert!(value["finishedAtMs"].is_number());
    assert_eq!(value["status"], "completed");
    assert_eq!(value["totalBatches"], 1);
    assert_eq!(value["batchSize"], 50);
    assert_eq!(value["maxConcurrency"], 1);
    assert_eq!(value["maxRetries"], 2);
    assert_eq!(value["finishedBatches"], 1);
    assert_eq!(value["failedBatches"], 0);
    assert_eq!(value["peakConcurrency"], 1);
    assert_eq!(value["batches"][0]["batchNumber"], 1);
    assert_eq!(value["batches"][0]["itemCount"], 1);
    assert_eq!(value["batches"][0]["status"], "completed");
    assert_eq!(value["batches"][0]["attemptCount"], 1);
    assert!(value["batches"][0]["elapsedMs"].is_number());
    assert!(value["batches"][0]["usage"].is_null());
    assert!(value["usage"].is_null());
    assert_eq!(value["results"][0]["itemId"], "save");
    assert_eq!(value["results"][0]["translation"], "保存");
    assert!(value.get("rawResponse").is_none());
    assert!(value.get("credential").is_none());
    assert!(value.get("baseUrl").is_none());
}

#[test]
fn cancelled_job_discards_a_provider_result_that_arrives_late() {
    let root = tempdir().expect("cancelled AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save local profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve local profile");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(DelayedSyntheticProvider {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        }),
    );
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            3,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("save", "Save")],
        ))
        .expect("plan cancelled job");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start cancelled job");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("provider starts");

    assert_eq!(
        translation.cancel_translation(&job_id).expect("cancel job"),
        glyphshift_ai_translation::CancellationOutcome::Requested
    );
    let cancelled_before_provider_returns = translation
        .translation_job(&job_id)
        .expect("query immediately cancelled job");
    release_tx.send(()).expect("release delayed response");
    assert_eq!(
        cancelled_before_provider_returns.status(),
        TranslationJobStatus::Cancelled,
        "the user-visible job must stop before an in-flight provider request returns"
    );
    assert_eq!(
        cancelled_before_provider_returns.batches()[0].status(),
        glyphshift_ai_translation::TranslationBatchStatus::Cancelled,
        "the visible in-flight batch must leave the running state immediately"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let cancelled = loop {
        let snapshot = translation.translation_job(&job_id).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "cancelled job did not finish");
        std::thread::yield_now();
    };

    assert_eq!(cancelled.status(), TranslationJobStatus::Cancelled);
    assert_eq!(cancelled.completed_count(), 0);
    assert!(cancelled.results().is_empty());
}

#[test]
fn job_rejects_translation_that_changes_protected_tokens() {
    let root = tempdir().expect("invalid AI output data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(AiProfileDraft::new(
            "profile.local",
            "Local",
            AiProviderProtocol::OllamaChat,
            "synthetic-model",
        ))
        .expect("save local profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve local profile");
    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(TokenDroppingSyntheticProvider),
    );
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            3,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("hello", "Hello {name}")],
        ))
        .expect("plan invalid-output job");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start invalid-output job");
    let deadline = Instant::now() + Duration::from_secs(2);
    let failed = loop {
        let snapshot = translation.translation_job(&job_id).expect("query job");
        if snapshot.status().is_terminal() {
            break snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "invalid-output job did not finish"
        );
        std::thread::yield_now();
    };

    assert_eq!(failed.status(), TranslationJobStatus::CompletedWithFailures);
    assert_eq!((failed.completed_count(), failed.failed_count()), (0, 1));
    assert!(failed.results().is_empty());
    assert_eq!(
        failed.errors()[0].category(),
        glyphshift_ai_translation::ProviderErrorCategory::MalformedOutput
    );
}

#[test]
fn one_click_job_drains_every_candidate_across_bounded_provider_requests() {
    let root = tempdir().expect("batched AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.local",
                "Local",
                AiProviderProtocol::OllamaChat,
                "synthetic-model",
            )
            .with_max_items_per_request(20),
        )
        .expect("save local profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve local profile");
    let provider = Arc::new(BatchRecordingSyntheticProvider::default());
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OllamaChat, provider.clone());
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            9,
            "en-US",
            "zh-CN",
            (1..=51).map(|index| {
                TranslationItem::untranslated(format!("item-{index}"), format!("Source {index}"))
            }),
        ))
        .expect("plan every candidate");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start batched job");
    let completed = wait_for_terminal_job(&translation, &job_id);

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.total_count(), 51);
    assert_eq!(completed.completed_count(), 51);
    assert_eq!(completed.total_batches(), 3);
    assert_eq!(completed.finished_batches(), 3);
    assert_eq!(completed.failed_batches(), 0);
    assert_eq!(completed.results().len(), 51);
    assert_eq!(
        *provider.request_sizes.lock().expect("batch sizes"),
        vec![20, 20, 11]
    );
}

#[test]
fn failed_batch_does_not_block_later_batches_and_progress_stays_complete() {
    let root = tempdir().expect("partially failed AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.local",
                "Local",
                AiProviderProtocol::OllamaChat,
                "synthetic-model",
            )
            .with_max_items_per_request(20),
        )
        .expect("save local profile");
    let profile = profiles
        .resolve_profile("profile.local")
        .expect("resolve local profile");
    let provider = Arc::new(SecondBatchFailsProvider::default());
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OllamaChat, provider.clone());
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            9,
            "en-US",
            "zh-CN",
            (1..=51).map(|index| {
                TranslationItem::untranslated(format!("item-{index}"), format!("Source {index}"))
            }),
        ))
        .expect("plan every candidate");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start batched job");
    let completed = wait_for_terminal_job(&translation, &job_id);

    assert_eq!(
        completed.status(),
        TranslationJobStatus::CompletedWithFailures
    );
    assert_eq!(
        (completed.completed_count(), completed.failed_count()),
        (31, 20)
    );
    assert_eq!(
        (
            completed.total_batches(),
            completed.finished_batches(),
            completed.failed_batches(),
        ),
        (3, 3, 1)
    );
    assert_eq!(completed.results().len(), 31);
    assert_eq!(
        *provider.request_sizes.lock().expect("batch sizes"),
        vec![20, 20, 11]
    );
}

#[test]
fn profile_concurrency_starts_multiple_short_text_batches_in_parallel() {
    let root = tempdir().expect("concurrent AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.concurrent",
                "Concurrent",
                AiProviderProtocol::OpenAiCompatible,
                "synthetic-model",
            )
            .with_base_url("http://synthetic.invalid/v1"),
        )
        .expect("save concurrent profile");
    let profile = profiles
        .resolve_profile("profile.concurrent")
        .expect("resolve concurrent profile");
    assert_eq!(profile.max_concurrency(), 2);

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let provider = Arc::new(DelayedSyntheticProvider {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OpenAiCompatible, provider);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            12,
            "en-US",
            "zh-CN",
            (1..=100).map(|index| {
                TranslationItem::untranslated(format!("item-{index}"), format!("Text {index:03}"))
            }),
        ))
        .expect("plan short-text job");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start concurrent job");

    entered_rx
        .recv_timeout(Duration::from_millis(250))
        .expect("first provider request starts");
    entered_rx
        .recv_timeout(Duration::from_millis(250))
        .expect("profile concurrency starts a second provider request before the first finishes");

    for _ in 0..2 {
        release_tx.send(()).expect("release provider request");
    }
    let completed = wait_for_terminal_job(&translation, &job_id);
    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.completed_count(), 100);
    assert_eq!(completed.finished_batches(), 2);
}

#[test]
fn running_job_snapshot_exposes_three_parallel_batches_instead_of_only_completion_count() {
    let root = tempdir().expect("observable concurrent AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.concurrent",
                "Concurrent",
                AiProviderProtocol::OpenAiCompatible,
                "synthetic-model",
            )
            .with_base_url("http://synthetic.invalid/v1")
            .with_max_items_per_request(10)
            .with_max_concurrency(3),
        )
        .expect("save concurrent profile");
    let profile = profiles
        .resolve_profile("profile.concurrent")
        .expect("resolve concurrent profile");

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let provider = Arc::new(DelayedSyntheticProvider {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OpenAiCompatible, provider);
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            13,
            "en-US",
            "zh-CN",
            (1..=40).map(|index| {
                TranslationItem::untranslated(format!("item-{index}"), format!("Text {index:03}"))
            }),
        ))
        .expect("plan observable concurrent job");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start observable concurrent job");

    for _ in 0..3 {
        entered_rx
            .recv_timeout(Duration::from_millis(250))
            .expect("three provider requests start together");
    }
    let running = serde_json::to_value(
        translation
            .translation_job(&job_id)
            .expect("query running job"),
    )
    .expect("serialize running job");
    assert_eq!(running["peakConcurrency"], 3);
    assert_eq!(
        running["batches"]
            .as_array()
            .expect("batch telemetry")
            .iter()
            .filter(|batch| batch["status"] == "running")
            .count(),
        3
    );
    assert_eq!(
        running["batches"]
            .as_array()
            .expect("batch telemetry")
            .iter()
            .filter(|batch| batch["status"] == "queued")
            .count(),
        1
    );

    for _ in 0..3 {
        release_tx.send(()).expect("release provider request");
    }
    entered_rx
        .recv_timeout(Duration::from_millis(250))
        .expect("remaining batch starts when a slot is free");
    release_tx
        .send(())
        .expect("release remaining provider request");
    let completed = wait_for_terminal_job(&translation, &job_id);
    assert_eq!(completed.status(), TranslationJobStatus::Completed);
}

#[test]
fn retry_wait_is_visible_with_the_next_attempt_number_and_safe_error() {
    let root = tempdir().expect("retry telemetry AI job data root");
    let mut profiles =
        AiProfileCatalog::open(root.path(), Box::new(EmptyCredentialVault)).expect("open profiles");
    profiles
        .save_profile(
            AiProfileDraft::new(
                "profile.retry",
                "Retry",
                AiProviderProtocol::OllamaChat,
                "synthetic-model",
            )
            .with_max_retries(1),
        )
        .expect("save retry profile");
    let profile = profiles
        .resolve_profile("profile.retry")
        .expect("resolve retry profile");
    let mut translation = AiTranslation::new();
    translation.register_provider(
        AiProviderProtocol::OllamaChat,
        Arc::new(RetryOnceSyntheticProvider::default()),
    );
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            14,
            "en-US",
            "zh-CN",
            [TranslationItem::untranslated("save", "Save")],
        ))
        .expect("plan retry telemetry job");
    let job_id = translation
        .start_translation(plan.token(), profile)
        .expect("start retry telemetry job");

    let deadline = Instant::now() + Duration::from_millis(200);
    let retrying = loop {
        let snapshot = translation
            .translation_job(&job_id)
            .expect("query retrying job");
        if snapshot.batches()[0].status()
            == glyphshift_ai_translation::TranslationBatchStatus::Retrying
        {
            break serde_json::to_value(snapshot).expect("serialize retrying job");
        }
        assert!(Instant::now() < deadline, "retry wait was never observable");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(retrying["batches"][0]["attemptCount"], 2);
    assert_eq!(
        retrying["batches"][0]["lastError"]["safeMessage"],
        "synthetic provider overloaded"
    );

    let completed = wait_for_terminal_job(&translation, &job_id);
    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.batches()[0].attempt_count(), 2);
}
