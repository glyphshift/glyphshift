//! Immutable decision publications shared by orchestration and runtime hosts.

use glyphshift_domain::{Generation, RouteLimits, RouteOperator, RouteProgram};
use glyphshift_translation::{FontPolicy, FontRule, TranslationSnapshot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const RUNTIME_SCHEMA: &str = "glyphshift.runtime/3";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeWireError {
    InvalidJson,
    UnsupportedSchema,
    ExecutableRoute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimePublicationIdentity([u8; 32]);

impl RuntimePublicationIdentity {
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Serialize, Deserialize)]
struct WirePublication {
    schema: Box<str>,
    generation: u64,
    route: WireRoute,
    translations: Vec<WireTranslation>,
    fonts: Vec<WireFont>,
}

#[derive(Serialize, Deserialize)]
struct WireRoute {
    max_state_entries: u16,
    max_steps: u16,
    operators: Vec<WireRouteOperator>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireRouteOperator {
    Direct {
        location: Box<str>,
    },
    Fallback {
        locations: Vec<Box<str>>,
    },
    ContextualHeading {
        location: Box<str>,
        context_kind: Box<str>,
    },
}

#[derive(Serialize, Deserialize)]
struct WireTranslation {
    location: Box<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_kind: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_key: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    adapter_ids: Vec<Box<str>>,
    source: Box<str>,
    translation: Box<str>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireFont {
    AdapterEntry {
        location: Box<str>,
        source: Box<str>,
        adapter: Box<str>,
        rule: WireFontRule,
    },
    Style {
        location: Box<str>,
        rule: WireFontRule,
        adapter_ids: Vec<Box<str>>,
    },
    Default {
        location: Box<str>,
        family: Box<str>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        adapter_ids: Vec<Box<str>>,
    },
    Entry {
        location: Box<str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_kind: Option<Box<str>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        context_key: Option<Box<str>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        adapter_ids: Vec<Box<str>>,
        source: Box<str>,
        rule: WireFontRule,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireFontRule {
    Scaled {
        family: Option<Box<str>>,
        percent: u16,
    },
    Unchanged,
    Substitute {
        family: Box<str>,
    },
}

/// Complete decision input for one runtime generation.
///
/// A generation number alone is not deployable. Hosts must receive the route, translations, and
/// font policy together and acknowledge only after the complete publication is applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePublication {
    route: RouteProgram,
    snapshot: TranslationSnapshot,
    font_policy: FontPolicy,
}

impl RuntimePublication {
    #[must_use]
    pub const fn new(
        route: RouteProgram,
        snapshot: TranslationSnapshot,
        font_policy: FontPolicy,
    ) -> Self {
        Self {
            route,
            snapshot,
            font_policy,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.snapshot.generation()
    }

    #[must_use]
    pub const fn route(&self) -> &RouteProgram {
        &self.route
    }

    #[must_use]
    pub const fn snapshot(&self) -> &TranslationSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub const fn font_policy(&self) -> &FontPolicy {
        &self.font_policy
    }

    /// Returns a deterministic identity for the complete encoded decision input.
    pub fn identity(&self) -> Result<RuntimePublicationIdentity, RuntimeWireError> {
        let encoded = self.encode_json()?;
        Ok(RuntimePublicationIdentity(
            Sha256::digest(encoded.as_bytes()).into(),
        ))
    }

    #[must_use]
    pub fn into_parts(self) -> (RouteProgram, TranslationSnapshot, FontPolicy) {
        (self.route, self.snapshot, self.font_policy)
    }

    pub fn encode_json(&self) -> Result<String, RuntimeWireError> {
        let operators = self
            .route
            .operators()
            .iter()
            .map(|operator| match operator {
                RouteOperator::Direct { location } => Ok(WireRouteOperator::Direct {
                    location: location.clone(),
                }),
                RouteOperator::Fallback { locations } => Ok(WireRouteOperator::Fallback {
                    locations: locations.clone(),
                }),
                RouteOperator::ContextualHeading {
                    location,
                    context_kind,
                } => Ok(WireRouteOperator::ContextualHeading {
                    location: location.clone(),
                    context_kind: context_kind.clone(),
                }),
                RouteOperator::Unknown { .. }
                | RouteOperator::NativeCode
                | RouteOperator::Script
                | RouteOperator::Io => Err(RuntimeWireError::ExecutableRoute),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut translations = Vec::new();
        self.snapshot
            .visit_entries_with_adapters(|location, source, translation, adapter_ids| {
                translations.push(WireTranslation {
                    location: location.into(),
                    context_kind: None,
                    context_key: None,
                    adapter_ids: adapter_ids.iter().cloned().collect(),
                    source: source.into(),
                    translation: translation.into(),
                });
            });
        self.snapshot.visit_context_entries_with_adapters(
            |location, context_kind, context_key, source, translation, adapter_ids| {
                translations.push(WireTranslation {
                    location: location.into(),
                    context_kind: Some(context_kind.into()),
                    context_key: Some(context_key.into()),
                    adapter_ids: adapter_ids.iter().cloned().collect(),
                    source: source.into(),
                    translation: translation.into(),
                });
            },
        );
        let mut fonts = Vec::new();
        self.font_policy
            .visit_adapter_entries(|location, source, adapter, rule| {
                fonts.push(WireFont::AdapterEntry {
                    location: location.into(),
                    source: source.into(),
                    adapter: adapter.into(),
                    rule: encode_font_rule(rule),
                });
            });
        self.font_policy
            .visit_location_rules_with_adapters(|location, rule, adapter_ids| {
                if let FontRule::Substitute(family) = rule {
                    fonts.push(WireFont::Default {
                        location: location.into(),
                        family: family.as_ref().into(),
                        adapter_ids: adapter_ids.iter().cloned().collect(),
                    });
                } else {
                    fonts.push(WireFont::Style {
                        location: location.into(),
                        rule: encode_font_rule(rule),
                        adapter_ids: adapter_ids.iter().cloned().collect(),
                    });
                }
            });
        self.font_policy
            .visit_entries_with_adapters(|location, source, rule, adapter_ids| {
                fonts.push(WireFont::Entry {
                    location: location.into(),
                    context_kind: None,
                    context_key: None,
                    adapter_ids: adapter_ids.iter().cloned().collect(),
                    source: source.into(),
                    rule: encode_font_rule(rule),
                });
            });
        self.font_policy.visit_context_entries_with_adapters(
            |location, context_kind, context_key, source, rule, adapter_ids| {
                fonts.push(WireFont::Entry {
                    location: location.into(),
                    context_kind: Some(context_kind.into()),
                    context_key: Some(context_key.into()),
                    adapter_ids: adapter_ids.iter().cloned().collect(),
                    source: source.into(),
                    rule: encode_font_rule(rule),
                });
            },
        );
        let limits = self.route.limits();
        serde_json::to_string(&WirePublication {
            schema: RUNTIME_SCHEMA.into(),
            generation: self.generation().value(),
            route: WireRoute {
                max_state_entries: limits.max_state_entries(),
                max_steps: limits.max_steps(),
                operators,
            },
            translations,
            fonts,
        })
        .map_err(|_| RuntimeWireError::InvalidJson)
    }

    pub fn decode_json(json: &str) -> Result<Self, RuntimeWireError> {
        let wire: WirePublication =
            serde_json::from_str(json).map_err(|_| RuntimeWireError::InvalidJson)?;
        if wire.schema.as_ref() != RUNTIME_SCHEMA {
            return Err(RuntimeWireError::UnsupportedSchema);
        }
        let route = RouteProgram::new(
            wire.route
                .operators
                .into_iter()
                .map(|operator| match operator {
                    WireRouteOperator::Direct { location } => RouteOperator::direct(location),
                    WireRouteOperator::Fallback { locations } => RouteOperator::fallback(locations),
                    WireRouteOperator::ContextualHeading {
                        location,
                        context_kind,
                    } => RouteOperator::contextual_heading(location, context_kind),
                }),
            RouteLimits::new(wire.route.max_state_entries, wire.route.max_steps),
        );
        let mut snapshot = TranslationSnapshot::empty(Generation::new(wire.generation));
        for entry in wire.translations {
            snapshot = match (entry.context_kind, entry.context_key) {
                (None, None) => snapshot.with_entry_for_adapters(
                    entry.location,
                    entry.source,
                    entry.translation,
                    entry.adapter_ids,
                ),
                (Some(kind), Some(key)) => snapshot.with_context_entry_for_adapters(
                    entry.location,
                    kind,
                    key,
                    entry.source,
                    entry.translation,
                    entry.adapter_ids,
                ),
                _ => return Err(RuntimeWireError::InvalidJson),
            };
        }
        let mut font_policy = FontPolicy::empty();
        for font in wire.fonts {
            font_policy = match font {
                WireFont::Default {
                    location,
                    family,
                    adapter_ids,
                } => font_policy.with_location_for_adapters(location, family, adapter_ids),
                WireFont::Entry {
                    location,
                    context_kind: None,
                    context_key: None,
                    adapter_ids,
                    source,
                    rule,
                } => font_policy.with_entry_for_adapters(
                    location,
                    source,
                    decode_font_rule(rule)?,
                    adapter_ids,
                ),
                WireFont::Entry {
                    location,
                    context_kind: Some(context_kind),
                    context_key: Some(context_key),
                    adapter_ids,
                    source,
                    rule,
                } => font_policy.with_context_entry_for_adapters(
                    location,
                    context_kind,
                    context_key,
                    source,
                    decode_font_rule(rule)?,
                    adapter_ids,
                ),
                WireFont::AdapterEntry {
                    location,
                    source,
                    adapter,
                    rule,
                } => font_policy.with_entry_for_adapter(
                    location,
                    source,
                    adapter,
                    decode_font_rule(rule)?,
                ),
                WireFont::Style {
                    location,
                    rule,
                    adapter_ids,
                } => font_policy.with_location_rule_for_adapters(
                    location,
                    decode_font_rule(rule)?,
                    adapter_ids,
                ),
                WireFont::Entry { .. } => return Err(RuntimeWireError::InvalidJson),
            };
        }
        Ok(Self::new(route, snapshot, font_policy))
    }
}

fn encode_font_rule(rule: &FontRule) -> WireFontRule {
    match rule {
        FontRule::Scaled { family, percent } => WireFontRule::Scaled {
            family: family.as_ref().map(|f| f.as_ref().into()),
            percent: *percent,
        },
        FontRule::Unchanged => WireFontRule::Unchanged,
        FontRule::Substitute(family) => WireFontRule::Substitute {
            family: family.as_ref().into(),
        },
    }
}

fn decode_font_rule(rule: WireFontRule) -> Result<FontRule, RuntimeWireError> {
    Ok(match rule {
        WireFontRule::Scaled { family, percent } => {
            if !(50..=200).contains(&percent)
                || family.as_ref().is_some_and(|f| {
                    f.trim().is_empty() || f.contains('\0') || f.chars().count() > 128
                })
            {
                return Err(RuntimeWireError::InvalidJson);
            }
            FontRule::Scaled {
                family: family.map(Into::into),
                percent,
            }
        }
        WireFontRule::Unchanged => FontRule::Unchanged,
        WireFontRule::Substitute { family } => FontRule::Substitute(family.into()),
    })
}
