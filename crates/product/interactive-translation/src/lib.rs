//! Product policy for resolving one-shot acquired text and presenting it outside the target.
//!
//! Dictionary lookup receives source text only, translation providers additionally receive the
//! requested locales, and presenters receive translated text plus ephemeral anchors. None of the
//! ports can observe an authorized target token or perform target-process text replacement.

mod model;
mod ports;
mod session;

pub use model::{
    BlockFailure, DictionaryTranslation, InteractiveTranslationError,
    InteractiveTranslationOutcome, PresentationBlock, PresentationError, ProviderRequest,
    ProviderTranslation, TranslationInputError, TranslationLocales, TranslationOrigin,
    TranslationProviderError,
};
pub use ports::{
    CancellationSignal, DictionaryLookup, ExternalTranslationPresenter, NeverCancelled,
    TranslationProvider,
};
pub use session::InteractiveTranslationSession;
