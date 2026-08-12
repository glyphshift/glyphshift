use glyphshift_ai_translation::{
    AiProfileCatalog, AiProfileDraft, AiProviderProtocol, AiTranslation, CancellationToken,
    CredentialVault, CredentialVaultError, ProviderBatchResult, ProviderError, ProviderRequest,
    ProviderTranslation, TranslationBatchPolicy, TranslationItem, TranslationJobStatus,
    TranslationPlanRequest, TranslationProvider,
};
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
        .start_translation(plan.token(), profile, TranslationBatchPolicy::default())
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
        .start_translation(plan.token(), profile, TranslationBatchPolicy::default())
        .expect("start translation");
    let snapshot = wait_for_terminal_job(&translation, &job_id);

    let value = serde_json::to_value(snapshot).expect("serialize job snapshot");

    assert_eq!(value["jobId"], job_id.as_str());
    assert_eq!(value["planToken"], plan.token());
    assert_eq!(value["snapshotRevision"], 7);
    assert_eq!(value["status"], "completed");
    assert_eq!(value["totalBatches"], 1);
    assert_eq!(value["finishedBatches"], 1);
    assert_eq!(value["failedBatches"], 0);
    assert_eq!(value["results"][0]["itemId"], "save");
    assert_eq!(value["results"][0]["translation"], "保存");
    assert!(value.get("rawResponse").is_none());
    assert!(value.get("credential").is_none());
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
        .start_translation(plan.token(), profile, TranslationBatchPolicy::default())
        .expect("start cancelled job");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("provider starts");

    assert_eq!(
        translation.cancel_translation(&job_id).expect("cancel job"),
        glyphshift_ai_translation::CancellationOutcome::Requested
    );
    release_tx.send(()).expect("release delayed response");
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
        .start_translation(plan.token(), profile, TranslationBatchPolicy::default())
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
        .start_translation(
            plan.token(),
            profile,
            TranslationBatchPolicy::new(20, 16_000).expect("valid batch policy"),
        )
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
fn global_input_token_budget_splits_a_batch_before_the_item_limit() {
    let root = tempdir().expect("token-budgeted AI job data root");
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
    let provider = Arc::new(BatchRecordingSyntheticProvider::default());
    let mut translation = AiTranslation::new();
    translation.register_provider(AiProviderProtocol::OllamaChat, provider.clone());
    let plan = translation
        .plan_translation(TranslationPlanRequest::new(
            "dictionary.pending",
            10,
            "en-US",
            "zh-CN",
            (1..=5).map(|index| {
                TranslationItem::untranslated(format!("item-{index}"), format!("Source {index}"))
            }),
        ))
        .expect("plan token-budgeted candidates");
    let job_id = translation
        .start_translation(
            plan.token(),
            profile,
            TranslationBatchPolicy::new(100, 420).expect("valid batch policy"),
        )
        .expect("start token-budgeted job");
    let completed = wait_for_terminal_job(&translation, &job_id);

    assert_eq!(completed.status(), TranslationJobStatus::Completed);
    assert_eq!(completed.total_batches(), 3);
    assert_eq!(completed.completed_count(), 5);
    assert_eq!(
        *provider.request_sizes.lock().expect("batch sizes"),
        vec![2, 2, 1]
    );
}

#[test]
fn failed_batch_does_not_block_later_batches_and_progress_stays_complete() {
    let root = tempdir().expect("partially failed AI job data root");
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
        .start_translation(
            plan.token(),
            profile,
            TranslationBatchPolicy::new(20, 16_000).expect("valid batch policy"),
        )
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
