use glyphshift_acquisition::{
    AcquisitionAdapter, AcquisitionCandidate, AcquisitionError, AcquisitionRequest,
    AuthorizedTarget, DesktopPoint, DesktopRect, Granularity, InteractiveSelection,
    InteractiveTextAcquisition, Provenance, SourcePolicy,
};
use glyphshift_interactive_translation::{
    CancellationSignal, DictionaryLookup, DictionaryTranslation, ExternalTranslationPresenter,
    InteractiveTranslationError, InteractiveTranslationSession, NeverCancelled, PresentationBlock,
    PresentationError, ProviderRequest, ProviderTranslation, TranslationLocales, TranslationOrigin,
    TranslationProvider, TranslationProviderError,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone)]
struct FixtureAcquirer {
    candidates: Vec<AcquisitionCandidate>,
}

impl AcquisitionAdapter for FixtureAcquirer {
    fn provenance(&self) -> Provenance {
        Provenance::Structured
    }

    fn acquire(
        &mut self,
        _request: &AcquisitionRequest,
    ) -> Result<Vec<AcquisitionCandidate>, AcquisitionError> {
        Ok(self.candidates.clone())
    }
}

#[derive(Default)]
struct LocalDictionary {
    entries: BTreeMap<Box<str>, DictionaryTranslation>,
    lookups: Vec<Box<str>>,
}

impl LocalDictionary {
    fn with_entry(mut self, source: &str, dictionary_id: &str, translation: &str) -> Self {
        self.entries.insert(
            source.into(),
            DictionaryTranslation::new(dictionary_id, translation).expect("dictionary entry"),
        );
        self
    }
}

impl DictionaryLookup for LocalDictionary {
    fn lookup(&mut self, source: &str) -> Option<DictionaryTranslation> {
        self.lookups.push(source.into());
        self.entries.get(source).cloned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProviderCall {
    source: Box<str>,
    source_locale: Box<str>,
    target_locale: Box<str>,
}

#[derive(Default)]
struct FixtureProvider {
    outcomes: VecDeque<Result<ProviderTranslation, TranslationProviderError>>,
    calls: Vec<ProviderCall>,
}

impl FixtureProvider {
    fn with_outcomes(
        outcomes: impl IntoIterator<Item = Result<ProviderTranslation, TranslationProviderError>>,
    ) -> Self {
        Self {
            outcomes: outcomes.into_iter().collect(),
            calls: Vec::new(),
        }
    }
}

impl TranslationProvider for FixtureProvider {
    fn translate(
        &mut self,
        request: ProviderRequest<'_>,
        _cancellation: &dyn CancellationSignal,
    ) -> Result<ProviderTranslation, TranslationProviderError> {
        self.calls.push(ProviderCall {
            source: request.source().into(),
            source_locale: request.source_locale().into(),
            target_locale: request.target_locale().into(),
        });
        self.outcomes
            .pop_front()
            .unwrap_or(Err(TranslationProviderError::NoTranslation))
    }
}

#[derive(Default)]
struct PresenterFake {
    visible: Vec<PresentationBlock>,
    replace_calls: usize,
    clear_calls: usize,
    fail_replace: bool,
}

impl ExternalTranslationPresenter for PresenterFake {
    fn replace_session(&mut self, blocks: &[PresentationBlock]) -> Result<(), PresentationError> {
        self.replace_calls += 1;
        self.visible = blocks.to_vec();
        if self.fail_replace {
            Err(PresentationError::Unavailable)
        } else {
            Ok(())
        }
    }

    fn clear_session(&mut self) -> Result<(), PresentationError> {
        self.clear_calls += 1;
        self.visible.clear();
        Ok(())
    }
}

struct Cancelled;

impl CancellationSignal for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

fn rect(offset: i32) -> DesktopRect {
    DesktopRect::new(offset, 20, offset + 80, 44).expect("fixture anchor")
}

fn acquisition(sources: &[&str]) -> glyphshift_acquisition::AcquisitionResult {
    let candidates = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            AcquisitionCandidate::new(*source, [rect(index as i32 * 100)], Granularity::Word)
        })
        .collect();
    let mut acquisition = InteractiveTextAcquisition::new([
        Box::new(FixtureAcquirer { candidates }) as Box<dyn AcquisitionAdapter>,
    ]);
    acquisition
        .acquire(&AcquisitionRequest::new(
            AuthorizedTarget::new("opaque-target").expect("target"),
            InteractiveSelection::Point(DesktopPoint::new(20, 30)),
            SourcePolicy::StructuredOnly,
        ))
        .expect("fixture acquisition")
}

fn locales() -> TranslationLocales {
    TranslationLocales::new("en-US", "zh-CN").expect("locales")
}

fn provider_translation(text: &str) -> ProviderTranslation {
    ProviderTranslation::new("fixture.provider", text).expect("provider translation")
}

#[test]
fn composition_001_dictionary_exact_match_bypasses_provider() {
    let dictionary = LocalDictionary::default().with_entry("Open", "dictionary.local", "打开");
    let mut session = InteractiveTranslationSession::new(
        dictionary,
        FixtureProvider::default(),
        PresenterFake::default(),
    );

    let outcome = session
        .present(Ok(acquisition(&["Open"])), &locales(), &NeverCancelled)
        .expect("dictionary presentation");

    assert_eq!(outcome.blocks().len(), 1);
    assert_eq!(outcome.blocks()[0].translation(), "打开");
    assert_eq!(
        outcome.blocks()[0].origin(),
        &TranslationOrigin::Dictionary("dictionary.local".into())
    );
    let (dictionary, provider, presenter) = session.into_parts();
    assert_eq!(dictionary.lookups, [Box::<str>::from("Open")]);
    assert!(provider.calls.is_empty());
    assert_eq!(presenter.replace_calls, 1);
    assert_eq!(presenter.visible, outcome.blocks());
}

#[test]
fn composition_002_provider_receives_only_misses_and_locales() {
    let dictionary = LocalDictionary::default().with_entry("Known", "dictionary.local", "已知");
    let provider = FixtureProvider::with_outcomes([Ok(provider_translation("缺失项"))]);
    let mut session =
        InteractiveTranslationSession::new(dictionary, provider, PresenterFake::default());

    let outcome = session
        .present(
            Ok(acquisition(&["Known", "Missing"])),
            &locales(),
            &NeverCancelled,
        )
        .expect("mixed presentation");

    assert_eq!(outcome.blocks().len(), 2);
    assert!(!outcome.is_partial());
    assert_eq!(outcome.blocks()[0].translation(), "已知");
    assert_eq!(outcome.blocks()[1].translation(), "缺失项");
    assert_eq!(outcome.blocks()[1].anchors(), &[rect(100)]);
    let (_, provider, _) = session.into_parts();
    assert_eq!(
        provider.calls,
        [ProviderCall {
            source: "Missing".into(),
            source_locale: "en-US".into(),
            target_locale: "zh-CN".into(),
        }]
    );
}

#[test]
fn composition_003_provider_failure_preserves_other_translated_blocks() {
    let provider = FixtureProvider::with_outcomes([
        Ok(provider_translation("一")),
        Err(TranslationProviderError::TimedOut),
        Ok(provider_translation("三")),
    ]);
    let mut session = InteractiveTranslationSession::new(
        LocalDictionary::default(),
        provider,
        PresenterFake::default(),
    );

    let outcome = session
        .present(
            Ok(acquisition(&["One", "Two", "Three"])),
            &locales(),
            &NeverCancelled,
        )
        .expect("partial provider result");

    assert!(outcome.is_partial());
    assert_eq!(outcome.blocks().len(), 2);
    assert_eq!(outcome.failures().len(), 1);
    assert_eq!(outcome.failures()[0].block_index(), 1);
    assert_eq!(
        outcome.failures()[0].error(),
        TranslationProviderError::TimedOut
    );
}

#[test]
fn composition_004_failed_or_cancelled_next_request_clears_previous_presentation() {
    let dictionary = LocalDictionary::default().with_entry("Open", "dictionary.local", "打开");
    let mut session = InteractiveTranslationSession::new(
        dictionary,
        FixtureProvider::default(),
        PresenterFake::default(),
    );
    session
        .present(Ok(acquisition(&["Open"])), &locales(), &NeverCancelled)
        .expect("initial presentation");

    assert_eq!(
        session.present(
            Err(AcquisitionError::PermissionDenied),
            &locales(),
            &NeverCancelled
        ),
        Err(InteractiveTranslationError::Acquisition(
            AcquisitionError::PermissionDenied
        ))
    );
    assert!(!session.is_active());

    session
        .present(Ok(acquisition(&["Open"])), &locales(), &NeverCancelled)
        .expect("second presentation");
    assert_eq!(
        session.present(Ok(acquisition(&["Open"])), &locales(), &Cancelled),
        Err(InteractiveTranslationError::Cancelled)
    );
    let (_, provider, presenter) = session.into_parts();
    assert!(provider.calls.is_empty());
    assert!(presenter.visible.is_empty());
    assert_eq!(presenter.clear_calls, 2);
}

#[test]
fn composition_005_all_provider_failures_do_not_publish_empty_content() {
    let provider = FixtureProvider::with_outcomes([Err(TranslationProviderError::Unavailable)]);
    let mut session = InteractiveTranslationSession::new(
        LocalDictionary::default(),
        provider,
        PresenterFake::default(),
    );

    assert_eq!(
        session.present(Ok(acquisition(&["Missing"])), &locales(), &NeverCancelled),
        Err(InteractiveTranslationError::TranslationUnavailable(
            TranslationProviderError::Unavailable
        ))
    );
    let (_, _, presenter) = session.into_parts();
    assert_eq!(presenter.replace_calls, 0);
    assert!(presenter.visible.is_empty());
}

#[test]
fn composition_006_presenter_failure_is_cleared_and_deactivate_is_idempotent() {
    let dictionary = LocalDictionary::default().with_entry("Open", "dictionary.local", "打开");
    let presenter = PresenterFake {
        fail_replace: true,
        ..PresenterFake::default()
    };
    let mut session =
        InteractiveTranslationSession::new(dictionary, FixtureProvider::default(), presenter);

    assert_eq!(
        session.present(Ok(acquisition(&["Open"])), &locales(), &NeverCancelled),
        Err(InteractiveTranslationError::Presentation(
            PresentationError::Unavailable
        ))
    );
    assert!(!session.is_active());
    session.deactivate().expect("inactive deactivation");
    let (_, _, presenter) = session.into_parts();
    assert_eq!(presenter.replace_calls, 1);
    assert_eq!(presenter.clear_calls, 1);
    assert!(presenter.visible.is_empty());
}
