//! Pure workflow composition for runtime intents.

use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workflow {
    id: Box<str>,
    targets: Vec<WorkflowTarget>,
}

impl Workflow {
    #[must_use]
    pub fn new(id: impl Into<Box<str>>, targets: impl IntoIterator<Item = WorkflowTarget>) -> Self {
        Self {
            id: id.into(),
            targets: targets.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterPlan {
    adapter_ids: Vec<Box<str>>,
}

impl AdapterPlan {
    #[must_use]
    pub fn parallel(adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>) -> Self {
        Self {
            adapter_ids: adapter_ids.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowTarget {
    software_id: Box<str>,
    adapter_plan: AdapterPlan,
    dictionary_ids: Vec<Box<str>>,
    font_bindings: Vec<FontProfileBinding>,
}

impl WorkflowTarget {
    #[must_use]
    pub fn new(
        software_id: impl Into<Box<str>>,
        adapter_plan: AdapterPlan,
        dictionary_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            adapter_plan,
            dictionary_ids: dictionary_ids.into_iter().map(Into::into).collect(),
            font_bindings: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_font_bindings(
        mut self,
        bindings: impl IntoIterator<Item = FontProfileBinding>,
    ) -> Self {
        self.font_bindings = bindings.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareInput {
    id: Box<str>,
    locale: Box<str>,
    generation: Generation,
    route: RouteProgram,
    locations: BTreeSet<Box<str>>,
}

impl SoftwareInput {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        locale: impl Into<Box<str>>,
        generation: Generation,
        route: RouteProgram,
        locations: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            id: id.into(),
            locale: locale.into(),
            generation,
            route,
            locations: locations.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dictionary {
    id: Box<str>,
    locale: Box<str>,
    entries: Vec<DictionaryEntry>,
}

impl Dictionary {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        locale: impl Into<Box<str>>,
        entries: impl IntoIterator<Item = DictionaryEntry>,
    ) -> Self {
        Self {
            id: id.into(),
            locale: locale.into(),
            entries: entries.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryEntry {
    location: Box<str>,
    context: Option<EntryContext>,
    source: Box<str>,
    text: EntryTextBehavior,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EntryTextBehavior {
    Keep,
    Replace(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EntryContext {
    kind: Box<str>,
    key: Box<str>,
}

impl DictionaryEntry {
    #[must_use]
    pub fn replace(
        location: impl Into<Box<str>>,
        source: impl Into<Box<str>>,
        translation: impl Into<Box<str>>,
    ) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            text: EntryTextBehavior::Replace(translation.into()),
        }
    }

    #[must_use]
    pub fn keep(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            text: EntryTextBehavior::Keep,
        }
    }

    #[must_use]
    pub fn with_context(mut self, kind: impl Into<Box<str>>, key: impl Into<Box<str>>) -> Self {
        self.context = Some(EntryContext {
            kind: kind.into(),
            key: key.into(),
        });
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontProfile {
    id: Box<str>,
    families: Vec<Box<str>>,
}

impl FontProfile {
    #[must_use]
    pub fn new(
        id: impl Into<Box<str>>,
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            id: id.into(),
            families: families.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FontScope {
    All,
    Locations(BTreeSet<Box<str>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontProfileBinding {
    font_profile_id: Box<str>,
    scope: FontScope,
}

impl FontProfileBinding {
    #[must_use]
    pub fn all(font_profile_id: impl Into<Box<str>>) -> Self {
        Self {
            font_profile_id: font_profile_id.into(),
            scope: FontScope::All,
        }
    }

    #[must_use]
    pub fn locations(
        font_profile_id: impl Into<Box<str>>,
        locations: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            font_profile_id: font_profile_id.into(),
            scope: FontScope::Locations(locations.into_iter().map(Into::into).collect()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterInput {
    id: Box<str>,
    features: BTreeSet<Feature>,
}

impl AdapterInput {
    #[must_use]
    pub fn new(id: impl Into<Box<str>>, features: impl IntoIterator<Item = Feature>) -> Self {
        Self {
            id: id.into(),
            features: features.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionEnvironment {
    adapters: BTreeMap<Box<str>, AdapterInput>,
    font_families: BTreeSet<Box<str>>,
}

impl CompositionEnvironment {
    #[must_use]
    pub fn new(
        adapters: impl IntoIterator<Item = AdapterInput>,
        font_families: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            adapters: adapters
                .into_iter()
                .map(|adapter| (adapter.id.clone(), adapter))
                .collect(),
            font_families: font_families.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledWorkflow {
    id: Box<str>,
    targets: Vec<CompiledTarget>,
    diagnostics: Vec<CompositionDiagnostic>,
}

impl CompiledWorkflow {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn targets(&self) -> &[CompiledTarget] {
        &self.targets
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompositionDiagnostic {
    RuleConflict {
        software_id: Box<str>,
        location: Box<str>,
        context_kind: Option<Box<str>>,
        context_key: Option<Box<str>>,
        source: Box<str>,
        winning_dictionary_id: Box<str>,
        shadowed_dictionary_id: Box<str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledTarget {
    software_id: Box<str>,
    adapter_ids: Vec<Box<str>>,
    route: RouteProgram,
    snapshot: TranslationSnapshot,
    font_policy: FontPolicy,
    requested_features: Vec<Feature>,
}

impl CompiledTarget {
    #[must_use]
    pub fn software_id(&self) -> &str {
        &self.software_id
    }

    #[must_use]
    pub fn adapter_ids(&self) -> &[Box<str>] {
        &self.adapter_ids
    }

    #[must_use]
    pub fn generation(&self) -> Generation {
        self.snapshot.generation()
    }

    #[must_use]
    pub const fn route(&self) -> &RouteProgram {
        &self.route
    }

    #[must_use]
    pub fn snapshot(&self) -> &TranslationSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub const fn font_policy(&self) -> &FontPolicy {
        &self.font_policy
    }

    #[must_use]
    pub fn requested_features(&self) -> &[Feature] {
        &self.requested_features
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    EmptyAdapterPlan {
        software_id: Box<str>,
    },
    DuplicateAdapter {
        software_id: Box<str>,
        adapter_id: Box<str>,
    },
    UnknownSoftware(Box<str>),
    UnknownDictionary(Box<str>),
    UnknownFontProfile(Box<str>),
    UnknownAdapter(Box<str>),
    LocaleMismatch {
        software_id: Box<str>,
        dictionary_id: Box<str>,
    },
    UnknownLocation {
        software_id: Box<str>,
        asset_id: Box<str>,
        location: Box<str>,
    },
    FontUnavailable {
        font_profile_id: Box<str>,
    },
    EmptyFontScope {
        software_id: Box<str>,
        font_profile_id: Box<str>,
    },
    FontScopeConflict {
        software_id: Box<str>,
        location: Box<str>,
    },
    FeatureUnavailable {
        software_id: Box<str>,
        feature: Feature,
    },
    NoEffectiveRules {
        software_id: Box<str>,
    },
}

pub fn resolve(
    workflow: &Workflow,
    software_inputs: &[SoftwareInput],
    dictionaries: &[Dictionary],
    font_profiles: &[FontProfile],
    environment: &CompositionEnvironment,
) -> Result<CompiledWorkflow, ResolveError> {
    let software_by_id = software_inputs
        .iter()
        .map(|software| (software.id.as_ref(), software))
        .collect::<BTreeMap<&str, &SoftwareInput>>();
    let dictionaries_by_id = dictionaries
        .iter()
        .map(|dictionary| (dictionary.id.as_ref(), dictionary))
        .collect::<BTreeMap<&str, &Dictionary>>();
    let font_profiles_by_id = font_profiles
        .iter()
        .map(|profile| (profile.id.as_ref(), profile))
        .collect::<BTreeMap<&str, &FontProfile>>();
    let mut compiled_targets = Vec::with_capacity(workflow.targets.len());
    let mut diagnostics = Vec::new();

    for target in &workflow.targets {
        if target.adapter_plan.adapter_ids.is_empty() {
            return Err(ResolveError::EmptyAdapterPlan {
                software_id: target.software_id.clone(),
            });
        }
        let mut unique_adapter_ids = BTreeSet::new();
        if let Some(adapter_id) = target
            .adapter_plan
            .adapter_ids
            .iter()
            .find(|adapter_id| !unique_adapter_ids.insert((*adapter_id).clone()))
        {
            return Err(ResolveError::DuplicateAdapter {
                software_id: target.software_id.clone(),
                adapter_id: adapter_id.clone(),
            });
        }
        let software = software_by_id
            .get(target.software_id.as_ref())
            .copied()
            .ok_or_else(|| ResolveError::UnknownSoftware(target.software_id.clone()))?;
        let selected_adapters = target
            .adapter_plan
            .adapter_ids
            .iter()
            .map(|adapter_id| {
                environment
                    .adapters
                    .get(adapter_id)
                    .ok_or_else(|| ResolveError::UnknownAdapter(adapter_id.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut snapshot = TranslationSnapshot::empty(software.generation);
        let mut font_policy = FontPolicy::empty();
        let mut has_text_replacement = false;
        let mut has_font_substitution = false;
        let mut winning_dictionaries =
            BTreeMap::<(Box<str>, Option<EntryContext>, Box<str>), Box<str>>::new();

        for dictionary_id in &target.dictionary_ids {
            let dictionary = dictionaries_by_id
                .get(dictionary_id.as_ref())
                .copied()
                .ok_or_else(|| ResolveError::UnknownDictionary(dictionary_id.clone()))?;
            if dictionary.locale != software.locale {
                return Err(ResolveError::LocaleMismatch {
                    software_id: software.id.clone(),
                    dictionary_id: dictionary.id.clone(),
                });
            }
            for entry in &dictionary.entries {
                if !software.locations.contains(&entry.location) {
                    return Err(ResolveError::UnknownLocation {
                        software_id: software.id.clone(),
                        asset_id: dictionary.id.clone(),
                        location: entry.location.clone(),
                    });
                }
                let key = (
                    entry.location.clone(),
                    entry.context.clone(),
                    entry.source.clone(),
                );
                if let Some(winner) = winning_dictionaries.get(&key) {
                    diagnostics.push(CompositionDiagnostic::RuleConflict {
                        software_id: software.id.clone(),
                        location: entry.location.clone(),
                        context_kind: entry.context.as_ref().map(|context| context.kind.clone()),
                        context_key: entry.context.as_ref().map(|context| context.key.clone()),
                        source: entry.source.clone(),
                        winning_dictionary_id: winner.clone(),
                        shadowed_dictionary_id: dictionary.id.clone(),
                    });
                    continue;
                }
                winning_dictionaries.insert(key, dictionary.id.clone());
                if let EntryTextBehavior::Replace(translation) = &entry.text {
                    snapshot = match &entry.context {
                        None => snapshot.with_entry(
                            entry.location.clone(),
                            entry.source.clone(),
                            translation.clone(),
                        ),
                        Some(context) => snapshot.with_context_entry(
                            entry.location.clone(),
                            context.kind.clone(),
                            context.key.clone(),
                            entry.source.clone(),
                            translation.clone(),
                        ),
                    };
                    has_text_replacement = true;
                }
            }
        }

        let mut font_locations = BTreeSet::new();
        for binding in &target.font_bindings {
            if matches!(&binding.scope, FontScope::Locations(locations) if locations.is_empty()) {
                return Err(ResolveError::EmptyFontScope {
                    software_id: software.id.clone(),
                    font_profile_id: binding.font_profile_id.clone(),
                });
            }
            let profile = font_profiles_by_id
                .get(binding.font_profile_id.as_ref())
                .copied()
                .ok_or_else(|| ResolveError::UnknownFontProfile(binding.font_profile_id.clone()))?;
            let family = profile
                .families
                .iter()
                .find(|family| environment.font_families.contains(*family))
                .cloned()
                .ok_or_else(|| ResolveError::FontUnavailable {
                    font_profile_id: profile.id.clone(),
                })?;
            let locations = match &binding.scope {
                FontScope::All => software.locations.clone(),
                FontScope::Locations(locations) => locations.clone(),
            };
            for location in locations {
                if !software.locations.contains(&location) {
                    return Err(ResolveError::UnknownLocation {
                        software_id: software.id.clone(),
                        asset_id: profile.id.clone(),
                        location,
                    });
                }
                if !font_locations.insert(location.clone()) {
                    return Err(ResolveError::FontScopeConflict {
                        software_id: software.id.clone(),
                        location,
                    });
                }
                font_policy = font_policy.with_location(location, family.clone());
                has_font_substitution = true;
            }
        }

        let mut requested_features = Vec::with_capacity(2);
        if has_text_replacement {
            requested_features.push(Feature::TextReplace);
        }
        if has_font_substitution {
            requested_features.push(Feature::FontSubstitute);
        }
        if requested_features.is_empty() {
            return Err(ResolveError::NoEffectiveRules {
                software_id: software.id.clone(),
            });
        }
        for feature in &requested_features {
            if !selected_adapters
                .iter()
                .any(|adapter| adapter.features.contains(feature))
            {
                return Err(ResolveError::FeatureUnavailable {
                    software_id: software.id.clone(),
                    feature: *feature,
                });
            }
        }

        compiled_targets.push(CompiledTarget {
            software_id: software.id.clone(),
            adapter_ids: target.adapter_plan.adapter_ids.clone(),
            route: software.route.clone(),
            snapshot,
            font_policy,
            requested_features,
        });
    }

    Ok(CompiledWorkflow {
        id: workflow.id.clone(),
        targets: compiled_targets,
        diagnostics,
    })
}
