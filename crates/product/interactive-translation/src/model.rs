use glyphshift_acquisition::{
    AcquisitionError, Confidence, DesktopRect, Granularity, Provenance, SourceBlock,
};

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_LOCALE_BYTES: usize = 64;
const MAX_TRANSLATION_UNITS: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationLocales {
    source: Box<str>,
    target: Box<str>,
}

impl TranslationLocales {
    pub fn new(
        source: impl Into<Box<str>>,
        target: impl Into<Box<str>>,
    ) -> Result<Self, TranslationInputError> {
        let source = source.into();
        let target = target.into();
        if !valid_locale(&source) || !valid_locale(&target) {
            return Err(TranslationInputError::InvalidLocale);
        }
        Ok(Self { source, target })
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryTranslation {
    dictionary_id: Box<str>,
    translation: Box<str>,
}

impl DictionaryTranslation {
    pub fn new(
        dictionary_id: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Result<Self, TranslationInputError> {
        let translation = translation.into();
        Ok(Self {
            dictionary_id: checked_identifier(dictionary_id.into())?,
            translation: checked_translation(&translation)?,
        })
    }

    pub(crate) fn into_parts(self) -> (Box<str>, Box<str>) {
        (self.dictionary_id, self.translation)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderTranslation {
    provider_id: Box<str>,
    translation: Box<str>,
}

impl ProviderTranslation {
    pub fn new(
        provider_id: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Result<Self, TranslationInputError> {
        let translation = translation.into();
        Ok(Self {
            provider_id: checked_identifier(provider_id.into())?,
            translation: checked_translation(&translation)?,
        })
    }

    pub(crate) fn into_parts(self) -> (Box<str>, Box<str>) {
        (self.provider_id, self.translation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRequest<'a> {
    source: &'a str,
    source_locale: &'a str,
    target_locale: &'a str,
}

impl<'a> ProviderRequest<'a> {
    pub(crate) fn new(source: &'a str, locales: &'a TranslationLocales) -> Self {
        Self {
            source,
            source_locale: locales.source(),
            target_locale: locales.target(),
        }
    }

    #[must_use]
    pub const fn source(self) -> &'a str {
        self.source
    }

    #[must_use]
    pub const fn source_locale(self) -> &'a str {
        self.source_locale
    }

    #[must_use]
    pub const fn target_locale(self) -> &'a str {
        self.target_locale
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslationOrigin {
    Dictionary(Box<str>),
    Provider(Box<str>),
}

impl TranslationOrigin {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Dictionary(id) | Self::Provider(id) => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentationBlock {
    source: Box<str>,
    translation: Box<str>,
    anchors: Vec<DesktopRect>,
    granularity: Granularity,
    provenance: Provenance,
    confidence: Option<Confidence>,
    origin: TranslationOrigin,
}

impl PresentationBlock {
    pub(crate) fn from_dictionary(block: &SourceBlock, translation: DictionaryTranslation) -> Self {
        let (dictionary_id, translation) = translation.into_parts();
        Self::new(
            block,
            translation,
            TranslationOrigin::Dictionary(dictionary_id),
        )
    }

    pub(crate) fn from_provider(block: &SourceBlock, translation: ProviderTranslation) -> Self {
        let (provider_id, translation) = translation.into_parts();
        Self::new(block, translation, TranslationOrigin::Provider(provider_id))
    }

    fn new(block: &SourceBlock, translation: Box<str>, origin: TranslationOrigin) -> Self {
        Self {
            source: block.source().into(),
            translation,
            anchors: block.anchors().to_vec(),
            granularity: block.granularity(),
            provenance: block.provenance(),
            confidence: block.confidence(),
            origin,
        }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }

    #[must_use]
    pub fn anchors(&self) -> &[DesktopRect] {
        &self.anchors
    }

    #[must_use]
    pub const fn granularity(&self) -> Granularity {
        self.granularity
    }

    #[must_use]
    pub const fn provenance(&self) -> Provenance {
        self.provenance
    }

    #[must_use]
    pub const fn confidence(&self) -> Option<Confidence> {
        self.confidence
    }

    #[must_use]
    pub const fn origin(&self) -> &TranslationOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockFailure {
    block_index: usize,
    error: TranslationProviderError,
}

impl BlockFailure {
    pub(crate) const fn new(block_index: usize, error: TranslationProviderError) -> Self {
        Self { block_index, error }
    }

    #[must_use]
    pub const fn block_index(&self) -> usize {
        self.block_index
    }

    #[must_use]
    pub const fn error(&self) -> TranslationProviderError {
        self.error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractiveTranslationOutcome {
    blocks: Vec<PresentationBlock>,
    failures: Vec<BlockFailure>,
}

impl InteractiveTranslationOutcome {
    pub(crate) const fn new(blocks: Vec<PresentationBlock>, failures: Vec<BlockFailure>) -> Self {
        Self { blocks, failures }
    }

    #[must_use]
    pub fn blocks(&self) -> &[PresentationBlock] {
        &self.blocks
    }

    #[must_use]
    pub fn failures(&self) -> &[BlockFailure] {
        &self.failures
    }

    #[must_use]
    pub const fn is_partial(&self) -> bool {
        !self.failures.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationInputError {
    InvalidIdentifier,
    InvalidLocale,
    InvalidTranslation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationProviderError {
    NoTranslation,
    Unavailable,
    TimedOut,
    Rejected,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationError {
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractiveTranslationError {
    Acquisition(AcquisitionError),
    Cancelled,
    TranslationUnavailable(TranslationProviderError),
    Presentation(PresentationError),
}

fn checked_identifier(value: Box<str>) -> Result<Box<str>, TranslationInputError> {
    if value.trim().is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        Err(TranslationInputError::InvalidIdentifier)
    } else {
        Ok(value)
    }
}

fn checked_translation(value: &str) -> Result<Box<str>, TranslationInputError> {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let normalized = normalized.trim();
    if normalized.is_empty() || normalized.encode_utf16().count() > MAX_TRANSLATION_UNITS {
        Err(TranslationInputError::InvalidTranslation)
    } else {
        Ok(normalized.into())
    }
}

fn valid_locale(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_LOCALE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
