use super::*;

const MAX_DICTIONARY_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionarySummaryView {
    pub(super) metadata: DictionaryMetadata,
    pub(super) revision: u64,
    pub(super) entry_count: usize,
    pub(super) installation: DictionaryInstallationSummaryView,
}

impl DictionarySummaryView {
    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.name()
    }

    #[must_use]
    pub fn description(&self) -> &str {
        self.metadata.description()
    }

    #[must_use]
    pub const fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    #[must_use]
    pub const fn installation(&self) -> &DictionaryInstallationSummaryView {
        &self.installation
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryInstallationSummaryView {
    state: &'static str,
    installed_release: Option<Box<str>>,
    verified_publisher: Option<Box<str>>,
    update_release: Option<Box<str>>,
}

impl DictionaryInstallationSummaryView {
    pub(super) fn unmanaged() -> Self {
        Self {
            state: "unmanaged",
            installed_release: None,
            verified_publisher: None,
            update_release: None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> &str {
        self.state
    }

    #[must_use]
    pub fn installed_release(&self) -> Option<&str> {
        self.installed_release.as_deref()
    }

    #[must_use]
    pub fn verified_publisher(&self) -> Option<&str> {
        self.verified_publisher.as_deref()
    }

    #[must_use]
    pub fn update_release(&self) -> Option<&str> {
        self.update_release.as_deref()
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryEntryCreate {
    source: Box<str>,
    translation: Box<str>,
}

impl DictionaryEntryCreate {
    #[must_use]
    pub fn new(source: impl Into<Box<str>>, translation: impl Into<Box<str>>) -> Self {
        Self {
            source: source.into(),
            translation: translation.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryMetadata {
    id: Box<str>,
    release_version: Box<str>,
    name: Box<str>,
    description: Box<str>,
    source_locale: Box<str>,
    target_locale: Box<str>,
    authors: Vec<Box<str>>,
    license: Option<Box<str>>,
    homepage: Option<Box<str>>,
    tags: Vec<Box<str>>,
}

impl DictionaryMetadata {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn release_version(&self) -> &str {
        &self.release_version
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
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
    pub fn authors(&self) -> &[Box<str>] {
        &self.authors
    }
    #[must_use]
    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }
    #[must_use]
    pub fn homepage(&self) -> Option<&str> {
        self.homepage.as_deref()
    }
    #[must_use]
    pub fn tags(&self) -> &[Box<str>] {
        &self.tags
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryCreate {
    metadata: DictionaryMetadata,
    entries: Vec<DictionaryEntryCreate>,
}

impl DictionaryCreate {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
    ) -> Self {
        Self {
            metadata: DictionaryMetadata {
                id: id.into(),
                release_version: "0.1.0".into(),
                name: name.into(),
                description: "".into(),
                source_locale: source_locale.into(),
                target_locale: target_locale.into(),
                authors: Vec::new(),
                license: None,
                homepage: None,
                tags: Vec::new(),
            },
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }

    #[must_use]
    pub fn with_release_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.metadata.release_version = version.into();
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_authors(mut self, authors: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.authors = authors.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_license(mut self, license: impl Into<Box<str>>) -> Self {
        self.metadata.license = Some(license.into());
        self
    }

    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<Box<str>>) -> Self {
        self.metadata.homepage = Some(homepage.into());
        self
    }

    #[must_use]
    pub fn with_entries(
        mut self,
        entries: impl IntoIterator<Item = DictionaryEntryCreate>,
    ) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DictionaryEdit {
    metadata: DictionaryMetadata,
    base_revision: u64,
    entries: Vec<DictionaryEntryCreate>,
}

impl DictionaryEdit {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        name: impl Into<Box<str>>,
        source_locale: impl Into<Box<str>>,
        target_locale: impl Into<Box<str>>,
        base_revision: u64,
    ) -> Self {
        let create = DictionaryCreate::new(id, name, source_locale, target_locale);
        Self {
            metadata: create.metadata,
            base_revision,
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<Box<str>>) -> Self {
        self.metadata.description = description.into();
        self
    }

    #[must_use]
    pub fn with_release_version(mut self, version: impl Into<Box<str>>) -> Self {
        self.metadata.release_version = version.into();
        self
    }

    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_authors(mut self, authors: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        self.metadata.authors = authors.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_license(mut self, license: impl Into<Box<str>>) -> Self {
        self.metadata.license = Some(license.into());
        self
    }

    #[must_use]
    pub fn with_homepage(mut self, homepage: impl Into<Box<str>>) -> Self {
        self.metadata.homepage = Some(homepage.into());
        self
    }

    #[must_use]
    pub fn with_entries(
        mut self,
        entries: impl IntoIterator<Item = DictionaryEntryCreate>,
    ) -> Self {
        self.entries = entries.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntryView {
    source: Box<str>,
    translation: Box<str>,
}

impl DictionaryEntryView {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn translation(&self) -> &str {
        &self.translation
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryView {
    metadata: DictionaryMetadata,
    revision: u64,
    entries: Vec<DictionaryEntryView>,
}

impl DictionaryView {
    #[must_use]
    pub const fn metadata(&self) -> &DictionaryMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn entries(&self) -> &[DictionaryEntryView] {
        &self.entries
    }
}
impl DesktopBackend {
    pub fn dictionary(&self, dictionary_id: &str) -> Result<&DictionaryView, BackendError> {
        self.dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))
    }
    pub fn dictionary_json(&self, dictionary_id: &str) -> Result<Vec<u8>, BackendError> {
        self.dictionary(dictionary_id)?;
        fs::read(dictionary_path(&self.root, dictionary_id)?)
            .map_err(|_| BackendError::Storage("read-dictionary"))
    }

    pub fn import_dictionary_file(
        &mut self,
        input_path: impl AsRef<Path>,
    ) -> Result<DictionaryView, BackendError> {
        let input_path = input_path.as_ref();
        if !input_path.is_absolute() {
            return Err(BackendError::InvalidInput("dictionary-import-path"));
        }
        let metadata = fs::metadata(input_path)
            .map_err(|_| BackendError::Storage("read-dictionary-import"))?;
        if !metadata.is_file() || metadata.len() > MAX_DICTIONARY_IMPORT_BYTES {
            return Err(BackendError::InvalidInput("dictionary-import-file"));
        }
        let source = fs::read_to_string(input_path)
            .map_err(|_| BackendError::InvalidArtifact("dictionary-import-json"))?;
        let artifact = dictionary_package::DictionaryPackage::decode_json(&source, None)
            .map_err(|_| BackendError::InvalidArtifact("dictionary-import-json"))?;
        if self.dictionaries.contains_key(artifact.id()) {
            return Err(BackendError::DuplicateDictionary(artifact.id().into()));
        }
        let serialized = artifact
            .encode_json()
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        write_atomic(&dictionary_path(&self.root, artifact.id())?, &serialized)?;
        let view = dictionary_view(&artifact);
        self.dictionaries.insert(artifact.id().into(), view.clone());
        self.refresh_dictionary_installations()?;
        Ok(view)
    }

    pub fn export_dictionary_file(
        &self,
        dictionary_id: &str,
        output_path: impl AsRef<Path>,
    ) -> Result<(), BackendError> {
        let output_path = output_path.as_ref();
        if !output_path.is_absolute() {
            return Err(BackendError::InvalidInput("dictionary-export-path"));
        }
        let source = String::from_utf8(self.dictionary_json(dictionary_id)?)
            .map_err(|_| BackendError::InvalidArtifact("dictionary-json"))?;
        write_atomic(output_path, &source)
    }

    pub fn create_dictionary(
        &mut self,
        create: DictionaryCreate,
    ) -> Result<DictionaryView, BackendError> {
        let dictionary_id = create.metadata.id.clone();
        if self.dictionaries.contains_key(&dictionary_id) {
            return Err(BackendError::DuplicateDictionary(dictionary_id));
        }
        let artifact = dictionary_artifact(create)?;
        let serialized = artifact
            .encode_json()
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        let path = dictionary_path(&self.root, artifact.id())?;
        write_atomic(&path, &serialized)?;
        let view = dictionary_view(&artifact);
        self.dictionaries.insert(artifact.id().into(), view.clone());
        self.refresh_dictionary_installations()?;
        Ok(view)
    }

    pub fn update_dictionary(
        &mut self,
        edit: DictionaryEdit,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(&edit.metadata.id)
            .ok_or_else(|| BackendError::UnknownDictionary(edit.metadata.id.clone()))?;
        if current.revision != edit.base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let artifact = dictionary_artifact_at_revision(
            DictionaryCreate {
                metadata: edit.metadata,
                entries: edit.entries,
            },
            current.revision + 1,
        )?;
        let view = dictionary_view(&artifact);
        for workflow_id in self.enabled_workflows.keys() {
            let workflow = &self.workflows[workflow_id];
            if workflow.targets.iter().any(|target| {
                target
                    .dictionary_ids
                    .iter()
                    .any(|dictionary_id| dictionary_id.as_ref() == artifact.id())
            }) {
                self.resolve_workflow_artifact_with_dictionary(workflow, Some(&view))?;
            }
        }
        let serialized = artifact
            .encode_json()
            .map_err(|_| BackendError::InvalidArtifact("serialize-dictionary"))?;
        let path = dictionary_path(&self.root, artifact.id())?;
        write_atomic(&path, &serialized)?;
        self.dictionaries.insert(artifact.id().into(), view.clone());
        self.refresh_dictionary_installations()?;
        Ok(view)
    }

    pub fn upsert_dictionary_entry(
        &mut self,
        dictionary_id: &str,
        entry: DictionaryEntryCreate,
        replacing_source: Option<&str>,
        base_revision: u64,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))?
            .clone();
        if current.revision != base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let existing_entries = current
            .entries
            .iter()
            .map(dictionary_entry_create)
            .collect::<Vec<_>>();
        let mut entries = existing_entries.clone();
        let next_source = entry.source.clone();
        let index = replacing_source.and_then(|source| {
            entries
                .iter()
                .position(|entry| entry.source.as_ref() == source)
        });
        if replacing_source.is_some() && index.is_none() {
            return Err(BackendError::InvalidInput("dictionary-entry"));
        }
        if entries
            .iter()
            .enumerate()
            .any(|(candidate_index, candidate)| {
                Some(candidate_index) != index && candidate.source == next_source
            })
        {
            return Err(BackendError::InvalidInput("dictionary-entry-duplicate"));
        }
        if let Some(index) = index {
            entries[index] = entry;
        } else {
            entries.push(entry);
        }
        self.update_dictionary(DictionaryEdit {
            metadata: current.metadata,
            base_revision: current.revision,
            entries,
        })
    }

    pub fn upsert_dictionary_entries(
        &mut self,
        dictionary_id: &str,
        updates: impl IntoIterator<Item = DictionaryEntryCreate>,
        base_revision: u64,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))?
            .clone();
        if current.revision != base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let updates = updates.into_iter().collect::<Vec<_>>();
        let sources = updates
            .iter()
            .map(|entry| entry.source.as_ref())
            .collect::<BTreeSet<_>>();
        if updates.is_empty()
            || sources.len() != updates.len()
            || updates
                .iter()
                .any(|entry| entry.source.trim().is_empty() || entry.translation.trim().is_empty())
        {
            return Err(BackendError::InvalidInput("dictionary-entry"));
        }
        let existing_entries = current
            .entries
            .iter()
            .map(dictionary_entry_create)
            .collect::<Vec<_>>();
        let mut entries = existing_entries.clone();
        for update in updates {
            if let Some(index) = entries
                .iter()
                .position(|entry| entry.source == update.source)
            {
                entries[index] = update;
            } else {
                entries.push(update);
            }
        }
        if entries == existing_entries {
            return Ok(current);
        }
        self.update_dictionary(DictionaryEdit {
            metadata: current.metadata,
            base_revision: current.revision,
            entries,
        })
    }

    pub fn delete_dictionary_entries(
        &mut self,
        dictionary_id: &str,
        sources: impl IntoIterator<Item = impl Into<Box<str>>>,
        base_revision: u64,
    ) -> Result<DictionaryView, BackendError> {
        let current = self
            .dictionaries
            .get(dictionary_id)
            .ok_or_else(|| BackendError::UnknownDictionary(dictionary_id.into()))?
            .clone();
        if current.revision != base_revision {
            return Err(BackendError::RevisionConflict {
                current: current.revision,
            });
        }
        let sources = sources.into_iter().map(Into::into).collect::<BTreeSet<_>>();
        let existing_keys = current
            .entries
            .iter()
            .map(|entry| entry.source.clone())
            .collect::<BTreeSet<_>>();
        if sources.is_empty() || !sources.is_subset(&existing_keys) {
            return Err(BackendError::InvalidInput("dictionary-entry"));
        }
        let entries = current
            .entries
            .iter()
            .filter(|entry| !sources.contains(&entry.source))
            .map(dictionary_entry_create)
            .collect::<Vec<_>>();
        self.update_dictionary(DictionaryEdit {
            metadata: current.metadata,
            base_revision: current.revision,
            entries,
        })
    }

    pub fn delete_dictionaries<'a>(
        &mut self,
        dictionary_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), BackendError> {
        let dictionary_ids = dictionary_ids.into_iter().collect::<BTreeSet<_>>();
        for dictionary_id in &dictionary_ids {
            if !self.dictionaries.contains_key(*dictionary_id) {
                return Err(BackendError::UnknownDictionary((*dictionary_id).into()));
            }
            let workflow_ids = self
                .workflows
                .values()
                .filter(|workflow| {
                    workflow.targets.iter().any(|target| {
                        target
                            .dictionary_ids
                            .iter()
                            .any(|candidate| candidate.as_ref() == *dictionary_id)
                    })
                })
                .map(|workflow| workflow.id.clone())
                .collect::<Vec<_>>();
            if !workflow_ids.is_empty() {
                return Err(BackendError::DictionaryReferenced {
                    dictionary_id: (*dictionary_id).into(),
                    workflow_ids,
                });
            }
        }
        for dictionary_id in dictionary_ids {
            let path = dictionary_path(&self.root, dictionary_id)?;
            fs::remove_file(path).map_err(|_| BackendError::Storage("remove-dictionary"))?;
            self.dictionaries.remove(dictionary_id);
        }
        self.refresh_dictionary_installations()?;
        Ok(())
    }

    pub fn reload_dictionaries(&mut self) -> Result<(), BackendError> {
        self.dictionaries = read_dictionaries(&self.root)?;
        self.refresh_dictionary_installations()
    }

    fn refresh_dictionary_installations(&mut self) -> Result<(), BackendError> {
        self.dictionary_installations = read_dictionary_installations(&self.root)?;
        Ok(())
    }
}

type DictionaryArtifact = dictionary_package::DictionaryPackage;

fn dictionary_artifact(create: DictionaryCreate) -> Result<DictionaryArtifact, BackendError> {
    dictionary_package::DictionaryPackage::create(package_dictionary_create(create))
        .map_err(|_| BackendError::InvalidArtifact("dictionary-contract"))
}

fn dictionary_artifact_at_revision(
    create: DictionaryCreate,
    revision: u64,
) -> Result<DictionaryArtifact, BackendError> {
    dictionary_package::DictionaryPackage::at_revision(package_dictionary_create(create), revision)
        .map_err(|_| BackendError::InvalidArtifact("dictionary-contract"))
}

fn package_dictionary_create(create: DictionaryCreate) -> dictionary_package::DictionaryCreate {
    let DictionaryCreate { metadata, entries } = create;
    let DictionaryMetadata {
        id,
        release_version,
        name,
        description,
        source_locale,
        target_locale,
        authors,
        license,
        homepage,
        tags,
    } = metadata;
    let mut packaged =
        dictionary_package::DictionaryCreate::new(id, name, source_locale, target_locale)
            .with_release_version(release_version)
            .with_description(description)
            .with_authors(authors)
            .with_tags(tags)
            .with_entries(entries.into_iter().map(package_dictionary_entry));
    if let Some(license) = license {
        packaged = packaged.with_license(license);
    }
    if let Some(homepage) = homepage {
        packaged = packaged.with_homepage(homepage);
    }
    packaged
}

fn package_dictionary_entry(
    entry: DictionaryEntryCreate,
) -> dictionary_package::DictionaryEntryCreate {
    dictionary_package::DictionaryEntryCreate::new(entry.source, entry.translation)
}

fn dictionary_view(artifact: &DictionaryArtifact) -> DictionaryView {
    let packaged = artifact.view();
    let metadata = packaged.metadata();
    DictionaryView {
        metadata: DictionaryMetadata {
            id: metadata.id().into(),
            release_version: metadata.release_version().into(),
            name: metadata.name().into(),
            description: metadata.description().into(),
            source_locale: metadata.source_locale().into(),
            target_locale: metadata.target_locale().into(),
            authors: metadata.authors().to_vec(),
            license: metadata.license().map(Into::into),
            homepage: metadata.homepage().map(Into::into),
            tags: metadata.tags().to_vec(),
        },
        revision: packaged.revision(),
        entries: packaged
            .entries()
            .iter()
            .map(|entry| DictionaryEntryView {
                source: entry.source().into(),
                translation: entry.translation().into(),
            })
            .collect(),
    }
}

fn dictionary_entry_create(entry: &DictionaryEntryView) -> DictionaryEntryCreate {
    DictionaryEntryCreate {
        source: entry.source.clone(),
        translation: entry.translation.clone(),
    }
}

pub(super) fn dictionary_definition(dictionary: &DictionaryView) -> WorkflowDictionary {
    let entries = dictionary
        .entries
        .iter()
        .map(|entry| WorkflowDictionaryEntry::new(entry.source.clone(), entry.translation.clone()));
    WorkflowDictionary::new(
        dictionary.id(),
        dictionary.metadata.target_locale.clone(),
        entries,
    )
}
pub(super) fn read_dictionaries(
    root: &Path,
) -> Result<BTreeMap<Box<str>, DictionaryView>, BackendError> {
    let directory = root.join("dictionaries");
    let mut paths = fs::read_dir(&directory)
        .map_err(|_| BackendError::Storage("read-dictionary-directory"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut dictionaries = BTreeMap::new();
    for path in paths {
        let source =
            fs::read_to_string(&path).map_err(|_| BackendError::Storage("dictionary-json"))?;
        let expected_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or(BackendError::InvalidArtifact("dictionary-json"))?;
        let artifact =
            dictionary_package::DictionaryPackage::decode_json(&source, Some(expected_id))
                .map_err(|_| BackendError::InvalidArtifact("dictionary-json"))?;
        let dictionary_id: Box<str> = artifact.id().into();
        if dictionaries
            .insert(dictionary_id.clone(), dictionary_view(&artifact))
            .is_some()
        {
            return Err(BackendError::DuplicateDictionary(dictionary_id));
        }
    }
    Ok(dictionaries)
}

pub(super) fn read_dictionary_installations(
    root: &Path,
) -> Result<BTreeMap<Box<str>, DictionaryInstallationSummaryView>, BackendError> {
    let mut store = FileDictionaryInstallStore::open(root)
        .map_err(|_| BackendError::Storage("open-dictionary-installations"))?;
    let summaries = store
        .installations()
        .map_err(|_| BackendError::Storage("read-dictionary-installations"))?
        .into_iter()
        .map(|view| {
            let source = view.source();
            let summary = DictionaryInstallationSummaryView {
                state: match view.state() {
                    DictionaryInstallationState::Verified => "verified",
                    DictionaryInstallationState::Modified => "modified",
                    DictionaryInstallationState::Missing => "missing",
                    DictionaryInstallationState::Unmanaged => "unmanaged",
                },
                installed_release: source.map(|source| source.release().release_version().into()),
                verified_publisher: source
                    .map(|source| source.publisher_identity().as_str().into()),
                update_release: None,
            };
            (Box::<str>::from(view.dictionary_id()), summary)
        })
        .collect::<BTreeMap<_, _>>();
    Ok(summaries)
}
