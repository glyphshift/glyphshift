use crate::{
    BlockFailure, CancellationSignal, DictionaryLookup, ExternalTranslationPresenter,
    InteractiveTranslationError, InteractiveTranslationOutcome, PresentationBlock,
    PresentationError, ProviderRequest, TranslationLocales, TranslationProvider,
    TranslationProviderError,
};
use glyphshift_acquisition::{AcquisitionError, AcquisitionResult};

pub struct InteractiveTranslationSession<D, T, P> {
    dictionary: D,
    provider: T,
    presenter: P,
    active: bool,
}

impl<D, T, P> InteractiveTranslationSession<D, T, P>
where
    D: DictionaryLookup,
    T: TranslationProvider,
    P: ExternalTranslationPresenter,
{
    #[must_use]
    pub const fn new(dictionary: D, provider: T, presenter: P) -> Self {
        Self {
            dictionary,
            provider,
            presenter,
            active: false,
        }
    }

    pub fn present(
        &mut self,
        acquisition: Result<AcquisitionResult, AcquisitionError>,
        locales: &TranslationLocales,
        cancellation: &dyn CancellationSignal,
    ) -> Result<InteractiveTranslationOutcome, InteractiveTranslationError> {
        self.clear_active()?;
        let acquisition = acquisition.map_err(InteractiveTranslationError::Acquisition)?;
        if cancellation.is_cancelled() {
            return Err(InteractiveTranslationError::Cancelled);
        }

        let mut presented = Vec::with_capacity(acquisition.blocks().len());
        let mut failures = Vec::new();
        for (block_index, block) in acquisition.blocks().iter().enumerate() {
            if cancellation.is_cancelled() {
                return Err(InteractiveTranslationError::Cancelled);
            }
            if let Some(translation) = self.dictionary.lookup(block.source()) {
                presented.push(PresentationBlock::from_dictionary(block, translation));
                continue;
            }

            match self
                .provider
                .translate(ProviderRequest::new(block.source(), locales), cancellation)
            {
                Ok(translation) => {
                    presented.push(PresentationBlock::from_provider(block, translation));
                }
                Err(TranslationProviderError::Cancelled) => {
                    return Err(InteractiveTranslationError::Cancelled);
                }
                Err(error) => failures.push(BlockFailure::new(block_index, error)),
            }
        }

        if cancellation.is_cancelled() {
            return Err(InteractiveTranslationError::Cancelled);
        }
        if presented.is_empty() {
            return Err(InteractiveTranslationError::TranslationUnavailable(
                select_failure(&failures),
            ));
        }

        if let Err(error) = self.presenter.replace_session(&presented) {
            self.active = true;
            if self.presenter.clear_session().is_ok() {
                self.active = false;
            }
            return Err(InteractiveTranslationError::Presentation(error));
        }
        self.active = true;
        Ok(InteractiveTranslationOutcome::new(presented, failures))
    }

    pub fn deactivate(&mut self) -> Result<(), InteractiveTranslationError> {
        self.clear_active()
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub fn into_parts(self) -> (D, T, P) {
        (self.dictionary, self.provider, self.presenter)
    }

    fn clear_active(&mut self) -> Result<(), InteractiveTranslationError> {
        if !self.active {
            return Ok(());
        }
        self.presenter
            .clear_session()
            .map_err(InteractiveTranslationError::Presentation)?;
        self.active = false;
        Ok(())
    }
}

fn select_failure(failures: &[BlockFailure]) -> TranslationProviderError {
    [
        TranslationProviderError::TimedOut,
        TranslationProviderError::Rejected,
        TranslationProviderError::Unavailable,
        TranslationProviderError::NoTranslation,
    ]
    .into_iter()
    .find(|candidate| failures.iter().any(|failure| failure.error() == *candidate))
    .unwrap_or(TranslationProviderError::NoTranslation)
}

impl From<PresentationError> for InteractiveTranslationError {
    fn from(error: PresentationError) -> Self {
        Self::Presentation(error)
    }
}
