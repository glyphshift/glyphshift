//! Pure workflow composition for runtime intents.

use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_translation::{FontPolicy, FontRule, TranslationSnapshot};
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
pub struct WorkflowTarget {
    software_id: Box<str>,
    dictionary_ids: Vec<Box<str>>,
}

impl WorkflowTarget {
    #[must_use]
    pub fn new(
        software_id: impl Into<Box<str>>,
        dictionary_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        Self {
            software_id: software_id.into(),
            dictionary_ids: dictionary_ids.into_iter().map(Into::into).collect(),
        }
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
    adapter_ids: BTreeSet<Box<str>>,
    default_font: DefaultFontBehavior,
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
            adapter_ids: BTreeSet::new(),
            default_font: DefaultFontBehavior::Unchanged,
            entries: entries.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn with_default_font(mut self, default_font: DefaultFontBehavior) -> Self {
        self.default_font = default_font;
        self
    }

    #[must_use]
    pub fn for_adapters(
        mut self,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.adapter_ids = adapter_ids.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DefaultFontBehavior {
    #[default]
    Unchanged,
    Substitute(Box<str>),
}

impl DefaultFontBehavior {
    #[must_use]
    pub fn substitute(family: impl Into<Box<str>>) -> Self {
        Self::Substitute(family.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryEntry {
    location: Box<str>,
    context: Option<EntryContext>,
    source: Box<str>,
    text: EntryTextBehavior,
    adapter_ids: BTreeSet<Box<str>>,
    font: EntryFontBehavior,
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
            adapter_ids: BTreeSet::new(),
            font: EntryFontBehavior::Inherit,
        }
    }

    #[must_use]
    pub fn keep(location: impl Into<Box<str>>, source: impl Into<Box<str>>) -> Self {
        Self {
            location: location.into(),
            context: None,
            source: source.into(),
            text: EntryTextBehavior::Keep,
            adapter_ids: BTreeSet::new(),
            font: EntryFontBehavior::Inherit,
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

    #[must_use]
    pub fn for_adapters(
        mut self,
        adapter_ids: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Self {
        self.adapter_ids = adapter_ids.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_font(mut self, font: EntryFontBehavior) -> Self {
        self.font = font;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EntryFontBehavior {
    #[default]
    Inherit,
    Unchanged,
    Substitute(Box<str>),
}

impl EntryFontBehavior {
    #[must_use]
    pub fn substitute(family: impl Into<Box<str>>) -> Self {
        Self::Substitute(family.into())
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
        adapter_ids: BTreeSet<Box<str>>,
        winning_dictionary_id: Box<str>,
        shadowed_dictionary_id: Box<str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledTarget {
    software_id: Box<str>,
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
    EmptyTarget {
        software_id: Box<str>,
    },
    UnknownSoftware(Box<str>),
    UnknownDictionary(Box<str>),
    LocaleMismatch {
        software_id: Box<str>,
        dictionary_id: Box<str>,
    },
    UnknownLocation {
        software_id: Box<str>,
        dictionary_id: Box<str>,
        location: Box<str>,
    },
    NoEffectiveRules {
        software_id: Box<str>,
    },
}

pub fn resolve(
    workflow: &Workflow,
    software_inputs: &[SoftwareInput],
    dictionaries: &[Dictionary],
) -> Result<CompiledWorkflow, ResolveError> {
    let software_by_id = software_inputs
        .iter()
        .map(|software| (software.id.as_ref(), software))
        .collect::<BTreeMap<&str, &SoftwareInput>>();
    let dictionaries_by_id = dictionaries
        .iter()
        .map(|dictionary| (dictionary.id.as_ref(), dictionary))
        .collect::<BTreeMap<&str, &Dictionary>>();
    let mut compiled_targets = Vec::with_capacity(workflow.targets.len());
    let mut diagnostics = Vec::new();

    for target in &workflow.targets {
        if target.dictionary_ids.is_empty() {
            return Err(ResolveError::EmptyTarget {
                software_id: target.software_id.clone(),
            });
        }
        let software = software_by_id
            .get(target.software_id.as_ref())
            .copied()
            .ok_or_else(|| ResolveError::UnknownSoftware(target.software_id.clone()))?;
        let mut snapshot = TranslationSnapshot::empty(software.generation);
        let mut font_policy = FontPolicy::empty();
        let mut has_text_replacement = false;
        let mut has_font_substitution = false;
        let mut default_font_applied_globally = false;
        let mut default_font_adapters_applied = BTreeSet::<Box<str>>::new();
        let mut winning_dictionaries = BTreeMap::<
            (Box<str>, Option<EntryContext>, Box<str>, BTreeSet<Box<str>>),
            Box<str>,
        >::new();

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
            if let DefaultFontBehavior::Substitute(family) = &dictionary.default_font {
                if dictionary.adapter_ids.is_empty() && !default_font_applied_globally {
                    for location in &software.locations {
                        font_policy = font_policy.with_location(location.clone(), family.clone());
                    }
                    has_font_substitution = true;
                    default_font_applied_globally = true;
                } else if !default_font_applied_globally {
                    let unapplied_adapter_ids = dictionary
                        .adapter_ids
                        .difference(&default_font_adapters_applied)
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    if !unapplied_adapter_ids.is_empty() {
                        for location in &software.locations {
                            font_policy = font_policy.with_location_for_adapters(
                                location.clone(),
                                family.clone(),
                                unapplied_adapter_ids.clone(),
                            );
                        }
                        default_font_adapters_applied.extend(unapplied_adapter_ids);
                        has_font_substitution = true;
                    }
                }
            }
            for entry in &dictionary.entries {
                if !software.locations.contains(&entry.location) {
                    return Err(ResolveError::UnknownLocation {
                        software_id: software.id.clone(),
                        dictionary_id: dictionary.id.clone(),
                        location: entry.location.clone(),
                    });
                }
                let adapter_ids = if dictionary.adapter_ids.is_empty() {
                    &entry.adapter_ids
                } else {
                    &dictionary.adapter_ids
                };
                let key = (
                    entry.location.clone(),
                    entry.context.clone(),
                    entry.source.clone(),
                    adapter_ids.clone(),
                );
                if let Some(winning_dictionary_id) = winning_dictionaries.get(&key) {
                    diagnostics.push(CompositionDiagnostic::RuleConflict {
                        software_id: software.id.clone(),
                        location: entry.location.clone(),
                        context_kind: entry.context.as_ref().map(|context| context.kind.clone()),
                        context_key: entry.context.as_ref().map(|context| context.key.clone()),
                        source: entry.source.clone(),
                        adapter_ids: adapter_ids.clone(),
                        winning_dictionary_id: winning_dictionary_id.clone(),
                        shadowed_dictionary_id: dictionary.id.clone(),
                    });
                    continue;
                }
                winning_dictionaries.insert(key, dictionary.id.clone());
                if let EntryTextBehavior::Replace(translation) = &entry.text {
                    snapshot = match &entry.context {
                        None => snapshot.with_entry_for_adapters(
                            entry.location.clone(),
                            entry.source.clone(),
                            translation.clone(),
                            adapter_ids.clone(),
                        ),
                        Some(context) => snapshot.with_context_entry_for_adapters(
                            entry.location.clone(),
                            context.kind.clone(),
                            context.key.clone(),
                            entry.source.clone(),
                            translation.clone(),
                            adapter_ids.clone(),
                        ),
                    };
                    has_text_replacement = true;
                }
                match &entry.font {
                    EntryFontBehavior::Inherit => {}
                    EntryFontBehavior::Unchanged => {
                        font_policy =
                            apply_font_rule(font_policy, entry, adapter_ids, FontRule::Unchanged);
                    }
                    EntryFontBehavior::Substitute(family) => {
                        font_policy = apply_font_rule(
                            font_policy,
                            entry,
                            adapter_ids,
                            FontRule::Substitute(family.clone().into()),
                        );
                        has_font_substitution = true;
                    }
                }
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

        compiled_targets.push(CompiledTarget {
            software_id: software.id.clone(),
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

fn apply_font_rule(
    policy: FontPolicy,
    entry: &DictionaryEntry,
    adapter_ids: &BTreeSet<Box<str>>,
    rule: FontRule,
) -> FontPolicy {
    match &entry.context {
        None => policy.with_entry_for_adapters(
            entry.location.clone(),
            entry.source.clone(),
            rule,
            adapter_ids.clone(),
        ),
        Some(context) => policy.with_context_entry_for_adapters(
            entry.location.clone(),
            context.kind.clone(),
            context.key.clone(),
            entry.source.clone(),
            rule,
            adapter_ids.clone(),
        ),
    }
}
