use crate::{
    DictionaryTranslation, PresentationBlock, PresentationError, ProviderRequest,
    ProviderTranslation, TranslationProviderError,
};

pub trait DictionaryLookup {
    fn lookup(&mut self, source: &str) -> Option<DictionaryTranslation>;
}

pub trait TranslationProvider {
    fn translate(
        &mut self,
        request: ProviderRequest<'_>,
        cancellation: &dyn CancellationSignal,
    ) -> Result<ProviderTranslation, TranslationProviderError>;
}

pub trait ExternalTranslationPresenter {
    fn replace_session(&mut self, blocks: &[PresentationBlock]) -> Result<(), PresentationError>;

    fn clear_session(&mut self) -> Result<(), PresentationError>;
}

pub trait CancellationSignal {
    fn is_cancelled(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl CancellationSignal for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}
