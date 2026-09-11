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
    font_policy: Option<TargetFontPolicy>,
    collect_text: bool,
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
            font_policy: None,
            collect_text: false,
        }
    }

    #[must_use]
    pub fn with_collection(mut self, enabled: bool) -> Self {
        self.collect_text = enabled;
        self
    }

    #[must_use]
    pub fn with_font_policy(mut self, policy: TargetFontPolicy) -> Self {
        self.font_policy = Some(policy);
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
    text_rules: glyphshift_translation::RegexTranslationRules,
}

impl Dictionary {
    pub fn with_text_rules(mut self, rules: glyphshift_translation::RegexTranslationRules) -> Self { self.text_rules = rules; self }
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
            text_rules: glyphshift_translation::RegexTranslationRules::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryEntry {
    source: Box<str>,
    translation: Box<str>,
}

impl DictionaryEntry {
    #[must_use]
    pub fn new(source: impl Into<Box<str>>, translation: impl Into<Box<str>>) -> Self {
        Self {
            source: source.into(),
            translation: translation.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontCoverage {
    DictionaryMatches,
    AllObservations,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetFontPolicy {
    families: Vec<Box<str>>,
    coverage: FontCoverage,
    scale_percent: u16,
    dictionary_overrides: BTreeMap<Box<str>, (Vec<Box<str>>, Option<u16>)>,
}

impl TargetFontPolicy {
    #[must_use]
    pub fn with_dictionary_override(mut self, id: impl Into<Box<str>>, families: impl IntoIterator<Item = impl Into<Box<str>>>, percent: Option<u16>) -> Self {
        self.dictionary_overrides.insert(id.into(), (families.into_iter().map(Into::into).collect(), percent));
        self
    }

    #[must_use]
    pub fn with_scale_percent(mut self, percent: u16) -> Self {
        self.scale_percent = percent;
        self
    }
    #[must_use]
    pub fn new(
        families: impl IntoIterator<Item = impl Into<Box<str>>>,
        coverage: FontCoverage,
    ) -> Self {
        Self {
            families: families.into_iter().map(Into::into).collect(),
            coverage,
            scale_percent: 100,
            dictionary_overrides: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn families(&self) -> &[Box<str>] {
        &self.families
    }

    #[must_use]
    pub const fn coverage(&self) -> FontCoverage {
        self.coverage
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
    EntryConflict {
        software_id: Box<str>,
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
    InvalidFontScale {
        software_id: Box<str>,
    },
    EmptyAdapterPlan {
        software_id: Box<str>,
    },
    DuplicateAdapter {
        software_id: Box<str>,
        adapter_id: Box<str>,
    },
    UnknownSoftware(Box<str>),
    UnknownDictionary(Box<str>),
    UnknownAdapter(Box<str>),
    LocaleMismatch {
        software_id: Box<str>,
        dictionary_id: Box<str>,
    },
    FontUnavailable {
        software_id: Box<str>,
    },
    EmptyFontFamilies {
        software_id: Box<str>,
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
        let mut winning_dictionaries = BTreeMap::<Box<str>, Box<str>>::new();

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
            let rule_location = format!("@dictionary/{}", dictionary.id);
            if !dictionary.text_rules.rules().is_empty() {
                snapshot = snapshot.with_dictionary_rules(rule_location.clone(), dictionary.text_rules.clone());
                has_text_replacement = true;
                for entry in &dictionary.entries {
                    snapshot = snapshot.with_entry(rule_location.clone(), entry.source.clone(), entry.translation.clone());
                }
            }
            for entry in &dictionary.entries {
                if let Some(winner) = winning_dictionaries.get(&entry.source) {
                    diagnostics.push(CompositionDiagnostic::EntryConflict {
                        software_id: software.id.clone(),
                        source: entry.source.clone(),
                        winning_dictionary_id: winner.clone(),
                        shadowed_dictionary_id: dictionary.id.clone(),
                    });
                    continue;
                }
                winning_dictionaries.insert(entry.source.clone(), dictionary.id.clone());
                for location in &software.locations {
                    snapshot = snapshot.with_entry(
                        location.clone(),
                        entry.source.clone(),
                        entry.translation.clone(),
                    );
                }
                has_text_replacement = true;
            }
        }

        let mut has_font_substitution = false;
        let mut has_font_scaling = false;
        if let Some(policy) = &target.font_policy {
            let select_family = |families: &[Box<str>]| -> Result<Option<Box<str>>, ResolveError> {
                if families.is_empty() {
                    return Ok(None);
                }
                families
                    .iter()
                    .find(|family| environment.font_families.contains(*family))
                    .cloned()
                    .map(Some)
                    .ok_or_else(|| ResolveError::FontUnavailable {
                        software_id: software.id.clone(),
                    })
            };
            let default_family = select_family(&policy.families)?;
            let mut add_rule = |source: Option<&Box<str>>,
                                family: Option<Box<str>>,
                                percent: u16|
             -> Result<(), ResolveError> {
                if !(50..=200).contains(&percent) {
                    return Err(ResolveError::InvalidFontScale {
                        software_id: software.id.clone(),
                    });
                }
                for (needed, feature) in [
                    (family.is_some(), Feature::FontSubstitute),
                    (percent != 100, Feature::FontScale),
                ] {
                    if needed
                        && !selected_adapters
                            .iter()
                            .any(|adapter| adapter.features.contains(&feature))
                    {
                        return Err(ResolveError::FeatureUnavailable {
                            software_id: software.id.clone(),
                            feature,
                        });
                    }
                }
                for adapter in &selected_adapters {
                    let scoped_family = if adapter.features.contains(&Feature::FontSubstitute) {
                        family.clone()
                    } else {
                        None
                    };
                    let scoped_percent = if adapter.features.contains(&Feature::FontScale) {
                        percent
                    } else {
                        100
                    };
                    if !adapter.features.contains(&Feature::FontSubstitute)
                        && !adapter.features.contains(&Feature::FontScale)
                    {
                        continue;
                    }
                    has_font_substitution |= scoped_family.is_some();
                    has_font_scaling |= scoped_percent != 100;
                    let rule = if scoped_percent != 100 {
                        FontRule::Scaled {
                            family: scoped_family.map(Into::into),
                            percent: scoped_percent,
                        }
                    } else {
                        scoped_family.map_or(FontRule::Unchanged, |family| {
                            FontRule::Substitute(family.into())
                        })
                    };
                    for location in &software.locations {
                        font_policy = if let Some(source) = source {
                            std::mem::take(&mut font_policy).with_entry_for_adapter(
                                location.clone(),
                                source.clone(),
                                adapter.id.clone(),
                                rule.clone(),
                            )
                        } else {
                            std::mem::take(&mut font_policy).with_location_rule_for_adapters(
                                location.clone(),
                                rule.clone(),
                                [adapter.id.clone()],
                            )
                        };
                    }
                }
                Ok(())
            };
            if policy.coverage == FontCoverage::AllObservations {
                add_rule(None, default_family.clone(), policy.scale_percent)?;
            }
            for (source, dictionary_id) in &winning_dictionaries {
                let (family, percent) = if let Some((families, scale)) = policy.dictionary_overrides.get(dictionary_id) {
                    (if families.is_empty() { default_family.clone() } else { select_family(families)? }, scale.unwrap_or(policy.scale_percent))
                } else { (default_family.clone(), policy.scale_percent) };
                add_rule(Some(source), family, percent)?;
            }
        }

        let mut requested_features = Vec::with_capacity(3);
        if target.collect_text {
            requested_features.push(Feature::TextObserve);
        }
        if has_text_replacement {
            requested_features.push(Feature::TextReplace);
        }
        if has_font_scaling {
            requested_features.push(Feature::FontScale);
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
