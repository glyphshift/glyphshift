use glyphshift_dictionary_package::DictionaryPackage;
use semver::Version;
use std::collections::BTreeSet;
use url::Url;

pub const DICTIONARY_MEDIA_TYPE: &str = "application/vnd.glyphshift.dictionary+json;version=2";
pub const DICTIONARY_ARTIFACT_STATEMENT_SCHEMA: &str = "glyphshift.dictionary-artifact-statement/1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogContractError {
    InvalidIdentifier,
    InvalidLocale,
    InvalidReleaseVersion,
    InvalidPresentation,
    InvalidArtifact,
    InvalidSignature,
    InvalidQuery,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DictionaryReleaseKey {
    catalog_id: Box<str>,
    dictionary_id: Box<str>,
    release_version: Box<str>,
}

impl DictionaryReleaseKey {
    pub fn new(
        catalog_id: impl Into<Box<str>>,
        dictionary_id: impl Into<Box<str>>,
        release_version: impl Into<Box<str>>,
    ) -> Result<Self, CatalogContractError> {
        let key = Self {
            catalog_id: catalog_id.into(),
            dictionary_id: dictionary_id.into(),
            release_version: release_version.into(),
        };
        if !safe_identifier(&key.catalog_id) || !safe_identifier(&key.dictionary_id) {
            return Err(CatalogContractError::InvalidIdentifier);
        }
        Version::parse(&key.release_version)
            .map_err(|_| CatalogContractError::InvalidReleaseVersion)?;
        Ok(key)
    }

    #[must_use]
    pub fn catalog_id(&self) -> &str {
        &self.catalog_id
    }

    #[must_use]
    pub fn dictionary_id(&self) -> &str {
        &self.dictionary_id
    }

    #[must_use]
    pub fn release_version(&self) -> &str {
        &self.release_version
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogQuery {
    text: Box<str>,
    source_locale: Option<Box<str>>,
    target_locale: Option<Box<str>>,
    tag: Option<Box<str>>,
    cursor: Option<Box<str>>,
    page_size: u16,
}

impl CatalogQuery {
    #[must_use]
    pub fn new(text: impl Into<Box<str>>) -> Self {
        Self {
            text: text.into(),
            source_locale: None,
            target_locale: None,
            tag: None,
            cursor: None,
            page_size: 50,
        }
    }

    #[must_use]
    pub fn with_source_locale(mut self, locale: impl Into<Box<str>>) -> Self {
        self.source_locale = Some(locale.into());
        self
    }

    #[must_use]
    pub fn with_target_locale(mut self, locale: impl Into<Box<str>>) -> Self {
        self.target_locale = Some(locale.into());
        self
    }

    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<Box<str>>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    #[must_use]
    pub fn with_cursor(mut self, cursor: impl Into<Box<str>>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    #[must_use]
    pub const fn with_page_size(mut self, page_size: u16) -> Self {
        self.page_size = page_size;
        self
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn source_locale(&self) -> Option<&str> {
        self.source_locale.as_deref()
    }

    #[must_use]
    pub fn target_locale(&self) -> Option<&str> {
        self.target_locale.as_deref()
    }

    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    #[must_use]
    pub const fn page_size(&self) -> u16 {
        self.page_size
    }

    pub(crate) fn validate(&self) -> Result<(), CatalogContractError> {
        if self.text.chars().count() > 256
            || self.page_size == 0
            || self.page_size > 100
            || self
                .source_locale
                .as_deref()
                .is_some_and(|locale| !valid_locale(locale))
            || self
                .target_locale
                .as_deref()
                .is_some_and(|locale| !valid_locale(locale))
            || self
                .tag
                .as_deref()
                .is_some_and(|tag| tag.trim().is_empty() || tag.chars().count() > 32)
            || self
                .cursor
                .as_deref()
                .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 256)
        {
            return Err(CatalogContractError::InvalidQuery);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactPresentation {
    locale: Box<str>,
    name: Box<str>,
    summary: Box<str>,
    tags: Vec<Box<str>>,
}

impl ArtifactPresentation {
    pub fn new(
        locale: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        summary: impl Into<Box<str>>,
    ) -> Result<Self, CatalogContractError> {
        let presentation = Self {
            locale: locale.into(),
            name: name.into(),
            summary: summary.into(),
            tags: Vec::new(),
        };
        presentation.validate()?;
        Ok(presentation)
    }

    pub fn with_tags(
        mut self,
        tags: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Result<Self, CatalogContractError> {
        self.tags = tags.into_iter().map(Into::into).collect();
        self.validate()?;
        Ok(self)
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn tags(&self) -> &[Box<str>] {
        &self.tags
    }

    fn validate(&self) -> Result<(), CatalogContractError> {
        let unique_tags = self
            .tags
            .iter()
            .map(|tag| tag.to_ascii_lowercase())
            .collect::<BTreeSet<_>>()
            .len()
            == self.tags.len();
        if !valid_locale(&self.locale)
            || self.name.trim().is_empty()
            || self.name.chars().count() > 128
            || self.summary.chars().count() > 512
            || self.tags.len() > 16
            || self
                .tags
                .iter()
                .any(|tag| tag.trim().is_empty() || tag.chars().count() > 32)
            || !unique_tags
        {
            return Err(CatalogContractError::InvalidPresentation);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    #[must_use]
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    #[must_use]
    pub fn to_hex(self) -> Box<str> {
        self.0
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            .into_boxed_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublisherIdentity(Box<str>);

impl PublisherIdentity {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, CatalogContractError> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().count() > 256 {
            return Err(CatalogContractError::InvalidArtifact);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignatureEnvelope {
    scheme: Box<str>,
    key_id: Box<str>,
    value: Box<str>,
}

impl SignatureEnvelope {
    pub fn new(
        scheme: impl Into<Box<str>>,
        key_id: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
    ) -> Result<Self, CatalogContractError> {
        let envelope = Self {
            scheme: scheme.into(),
            key_id: key_id.into(),
            value: value.into(),
        };
        if !safe_identifier(&envelope.scheme)
            || envelope.key_id.trim().is_empty()
            || envelope.key_id.chars().count() > 256
            || envelope.value.trim().is_empty()
            || envelope.value.chars().count() > 16_384
        {
            return Err(CatalogContractError::InvalidSignature);
        }
        Ok(envelope)
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryArtifactDescriptor {
    media_type: Box<str>,
    size: u64,
    digest: Sha256Digest,
    download_urls: Vec<Box<str>>,
    publisher_identity: PublisherIdentity,
    signature: SignatureEnvelope,
}

impl DictionaryArtifactDescriptor {
    pub fn new(
        size: u64,
        digest: Sha256Digest,
        download_urls: impl IntoIterator<Item = impl Into<Box<str>>>,
        publisher_identity: PublisherIdentity,
        signature: SignatureEnvelope,
    ) -> Result<Self, CatalogContractError> {
        let descriptor = Self {
            media_type: DICTIONARY_MEDIA_TYPE.into(),
            size,
            digest,
            download_urls: download_urls.into_iter().map(Into::into).collect(),
            publisher_identity,
            signature,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn digest(&self) -> Sha256Digest {
        self.digest
    }

    #[must_use]
    pub fn download_urls(&self) -> &[Box<str>] {
        &self.download_urls
    }

    #[must_use]
    pub const fn publisher_identity(&self) -> &PublisherIdentity {
        &self.publisher_identity
    }

    #[must_use]
    pub const fn signature(&self) -> &SignatureEnvelope {
        &self.signature
    }

    fn validate(&self) -> Result<(), CatalogContractError> {
        let valid_urls = !self.download_urls.is_empty()
            && self.download_urls.iter().all(|value| {
                Url::parse(value).is_ok_and(|url| url.scheme() == "https" && url.host().is_some())
            });
        if self.media_type.as_ref() != DICTIONARY_MEDIA_TYPE || self.size == 0 || !valid_urls {
            return Err(CatalogContractError::InvalidArtifact);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRelease {
    key: DictionaryReleaseKey,
    source_locale: Box<str>,
    target_locale: Box<str>,
    default_presentation_locale: Box<str>,
    presentations: Vec<ArtifactPresentation>,
    artifact: DictionaryArtifactDescriptor,
}

impl CatalogRelease {
    pub fn new(
        key: DictionaryReleaseKey,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
        default_presentation_locale: impl Into<Box<str>>,
        presentations: Vec<ArtifactPresentation>,
        artifact: DictionaryArtifactDescriptor,
    ) -> Result<Self, CatalogContractError> {
        let release = Self {
            key,
            source_locale: source_locale.into(),
            target_locale: target_locale.into(),
            default_presentation_locale: default_presentation_locale.into(),
            presentations,
            artifact,
        };
        release.validate()?;
        Ok(release)
    }

    #[must_use]
    pub const fn key(&self) -> &DictionaryReleaseKey {
        &self.key
    }

    #[must_use]
    pub fn source_locale(&self) -> &str {
        &self.source_locale
    }

    #[must_use]
    pub fn target_locale(&self) -> &str {
        &self.target_locale
    }

    #[must_use]
    pub fn default_presentation_locale(&self) -> &str {
        &self.default_presentation_locale
    }

    #[must_use]
    pub fn presentations(&self) -> &[ArtifactPresentation] {
        &self.presentations
    }

    #[must_use]
    pub const fn artifact(&self) -> &DictionaryArtifactDescriptor {
        &self.artifact
    }

    pub(crate) fn validate(&self) -> Result<(), CatalogContractError> {
        let presentation_locales = self
            .presentations
            .iter()
            .map(|presentation| presentation.locale.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        if !valid_locale(&self.source_locale)
            || !valid_locale(&self.target_locale)
            || !valid_locale(&self.default_presentation_locale)
            || self.presentations.is_empty()
            || presentation_locales.len() != self.presentations.len()
            || !self.presentations.iter().any(|presentation| {
                presentation
                    .locale
                    .eq_ignore_ascii_case(&self.default_presentation_locale)
            })
        {
            return Err(CatalogContractError::InvalidPresentation);
        }
        self.artifact.validate()
    }

    pub(crate) fn presentation_for(&self, requested_locale: &str) -> Option<ArtifactPresentation> {
        let mut candidate = requested_locale;
        loop {
            if let Some(presentation) = self
                .presentations
                .iter()
                .find(|presentation| presentation.locale.eq_ignore_ascii_case(candidate))
            {
                return Some(presentation.clone());
            }
            let Some((parent, _)) = candidate.rsplit_once('-') else {
                break;
            };
            candidate = parent;
        }
        self.presentations
            .iter()
            .find(|presentation| {
                presentation
                    .locale
                    .eq_ignore_ascii_case(&self.default_presentation_locale)
            })
            .cloned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogSourcePage {
    releases: Vec<CatalogRelease>,
    next_cursor: Option<Box<str>>,
}

impl CatalogSourcePage {
    #[must_use]
    pub fn new(releases: Vec<CatalogRelease>, next_cursor: Option<Box<str>>) -> Self {
        Self {
            releases,
            next_cursor,
        }
    }

    #[must_use]
    pub fn releases(&self) -> &[CatalogRelease] {
        &self.releases
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }

    pub(crate) fn into_parts(self) -> (Vec<CatalogRelease>, Option<Box<str>>) {
        (self.releases, self.next_cursor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogReleaseSummary {
    key: DictionaryReleaseKey,
    source_locale: Box<str>,
    target_locale: Box<str>,
    effective_presentation_locale: Box<str>,
    presentation: ArtifactPresentation,
    publisher_identity: PublisherIdentity,
}

impl CatalogReleaseSummary {
    pub(crate) fn from_release(
        release: &CatalogRelease,
        presentation: ArtifactPresentation,
    ) -> Self {
        Self {
            key: release.key.clone(),
            source_locale: release.source_locale.clone(),
            target_locale: release.target_locale.clone(),
            effective_presentation_locale: presentation.locale.clone(),
            presentation,
            publisher_identity: release.artifact.publisher_identity.clone(),
        }
    }

    #[must_use]
    pub const fn key(&self) -> &DictionaryReleaseKey {
        &self.key
    }

    #[must_use]
    pub fn source_locale(&self) -> &str {
        &self.source_locale
    }

    #[must_use]
    pub fn target_locale(&self) -> &str {
        &self.target_locale
    }

    #[must_use]
    pub fn effective_presentation_locale(&self) -> &str {
        &self.effective_presentation_locale
    }

    #[must_use]
    pub const fn presentation(&self) -> &ArtifactPresentation {
        &self.presentation
    }

    #[must_use]
    pub const fn publisher_identity(&self) -> &PublisherIdentity {
        &self.publisher_identity
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogPage {
    releases: Vec<CatalogReleaseSummary>,
    next_cursor: Option<Box<str>>,
}

impl CatalogPage {
    pub(crate) fn new(releases: Vec<CatalogReleaseSummary>, next_cursor: Option<Box<str>>) -> Self {
        Self {
            releases,
            next_cursor,
        }
    }

    #[must_use]
    pub fn releases(&self) -> &[CatalogReleaseSummary] {
        &self.releases
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactStatement {
    schema: Box<str>,
    dictionary_id: Box<str>,
    release_version: Box<str>,
    media_type: Box<str>,
    size: u64,
    digest_algorithm: Box<str>,
    digest: Sha256Digest,
    publisher_identity: PublisherIdentity,
}

impl ArtifactStatement {
    #[must_use]
    pub fn for_release(release: &CatalogRelease) -> Self {
        Self {
            schema: DICTIONARY_ARTIFACT_STATEMENT_SCHEMA.into(),
            dictionary_id: release.key.dictionary_id.clone(),
            release_version: release.key.release_version.clone(),
            media_type: release.artifact.media_type.clone(),
            size: release.artifact.size,
            digest_algorithm: "sha256".into(),
            digest: release.artifact.digest,
            publisher_identity: release.artifact.publisher_identity.clone(),
        }
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    #[must_use]
    pub fn dictionary_id(&self) -> &str {
        &self.dictionary_id
    }

    #[must_use]
    pub fn release_version(&self) -> &str {
        &self.release_version
    }

    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn digest_algorithm(&self) -> &str {
        &self.digest_algorithm
    }

    #[must_use]
    pub const fn digest(&self) -> Sha256Digest {
        self.digest
    }

    #[must_use]
    pub const fn publisher_identity(&self) -> &PublisherIdentity {
        &self.publisher_identity
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictionaryReplacementPolicy {
    RejectExisting,
    ReplaceVerified,
    ReplaceAny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallRequest {
    release: DictionaryReleaseKey,
    replacement: DictionaryReplacementPolicy,
}

impl InstallRequest {
    #[must_use]
    pub const fn new(
        release: DictionaryReleaseKey,
        replacement: DictionaryReplacementPolicy,
    ) -> Self {
        Self {
            release,
            replacement,
        }
    }

    #[must_use]
    pub const fn release(&self) -> &DictionaryReleaseKey {
        &self.release
    }

    #[must_use]
    pub const fn replacement(&self) -> DictionaryReplacementPolicy {
        self.replacement
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictionaryInstallationState {
    Verified,
    Modified,
    Missing,
    Unmanaged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryInstallationSource {
    release: DictionaryReleaseKey,
    media_type: Box<str>,
    size: u64,
    digest: Sha256Digest,
    publisher_identity: PublisherIdentity,
    signature: SignatureEnvelope,
    installed_at_unix_ms: u64,
    payload_revision: u64,
}

pub(crate) struct DictionaryInstallationSourceParts {
    pub release: DictionaryReleaseKey,
    pub media_type: Box<str>,
    pub size: u64,
    pub digest: Sha256Digest,
    pub publisher_identity: PublisherIdentity,
    pub signature: SignatureEnvelope,
    pub installed_at_unix_ms: u64,
    pub payload_revision: u64,
}

impl DictionaryInstallationSource {
    pub(crate) fn from_verified(artifact: &VerifiedDictionaryArtifact) -> Self {
        Self {
            release: artifact.release.clone(),
            media_type: artifact.statement.media_type.clone(),
            size: artifact.statement.size,
            digest: artifact.statement.digest,
            publisher_identity: artifact.publisher_identity.clone(),
            signature: artifact.signature.clone(),
            installed_at_unix_ms: artifact.installed_at_unix_ms,
            payload_revision: artifact.package.revision(),
        }
    }

    pub(crate) fn restore(
        parts: DictionaryInstallationSourceParts,
    ) -> Result<Self, CatalogContractError> {
        if parts.media_type.as_ref() != DICTIONARY_MEDIA_TYPE
            || parts.size == 0
            || parts.payload_revision == 0
        {
            return Err(CatalogContractError::InvalidArtifact);
        }
        Ok(Self {
            release: parts.release,
            media_type: parts.media_type,
            size: parts.size,
            digest: parts.digest,
            publisher_identity: parts.publisher_identity,
            signature: parts.signature,
            installed_at_unix_ms: parts.installed_at_unix_ms,
            payload_revision: parts.payload_revision,
        })
    }

    #[must_use]
    pub const fn release(&self) -> &DictionaryReleaseKey {
        &self.release
    }

    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn digest(&self) -> Sha256Digest {
        self.digest
    }

    #[must_use]
    pub const fn publisher_identity(&self) -> &PublisherIdentity {
        &self.publisher_identity
    }

    #[must_use]
    pub const fn signature(&self) -> &SignatureEnvelope {
        &self.signature
    }

    #[must_use]
    pub const fn installed_at_unix_ms(&self) -> u64 {
        self.installed_at_unix_ms
    }

    #[must_use]
    pub const fn payload_revision(&self) -> u64 {
        self.payload_revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryInstallationView {
    dictionary_id: Box<str>,
    state: DictionaryInstallationState,
    source: Option<DictionaryInstallationSource>,
}

impl DictionaryInstallationView {
    pub(crate) fn new(
        dictionary_id: Box<str>,
        state: DictionaryInstallationState,
        source: Option<DictionaryInstallationSource>,
    ) -> Self {
        Self {
            dictionary_id,
            state,
            source,
        }
    }

    #[must_use]
    pub fn dictionary_id(&self) -> &str {
        &self.dictionary_id
    }

    #[must_use]
    pub const fn state(&self) -> DictionaryInstallationState {
        self.state
    }

    #[must_use]
    pub const fn source(&self) -> Option<&DictionaryInstallationSource> {
        self.source.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct VerifiedDictionaryArtifact {
    pub(crate) release: DictionaryReleaseKey,
    pub(crate) payload: Vec<u8>,
    pub(crate) package: DictionaryPackage,
    pub(crate) statement: ArtifactStatement,
    pub(crate) signature: SignatureEnvelope,
    pub(crate) publisher_identity: PublisherIdentity,
    pub(crate) installed_at_unix_ms: u64,
}

impl VerifiedDictionaryArtifact {
    #[must_use]
    pub const fn release(&self) -> &DictionaryReleaseKey {
        &self.release
    }

    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    #[must_use]
    pub const fn package(&self) -> &DictionaryPackage {
        &self.package
    }

    #[must_use]
    pub const fn statement(&self) -> &ArtifactStatement {
        &self.statement
    }

    #[must_use]
    pub const fn signature(&self) -> &SignatureEnvelope {
        &self.signature
    }

    #[must_use]
    pub const fn publisher_identity(&self) -> &PublisherIdentity {
        &self.publisher_identity
    }

    #[must_use]
    pub const fn installed_at_unix_ms(&self) -> u64 {
        self.installed_at_unix_ms
    }
}

pub(crate) fn valid_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.split('-').all(|part| {
            !part.is_empty()
                && part.len() <= 8
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}
