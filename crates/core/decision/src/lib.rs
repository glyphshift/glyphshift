//! Pure Route, lookup, and render-decision semantics.

use glyphshift_domain::{
    FontDecision, RenderDecision, RouteOperator, RouteProgram, TextDecision, TextObservation,
};
use glyphshift_translation::{
    FontPolicy, FontPolicyDigest, FontRule, SnapshotDigest, TranslationSnapshot,
};
use std::collections::BTreeMap;

const MAX_ADAPTER_ID_BYTES: usize = 256;
const MAX_SOURCE_TEXT_BYTES: usize = 16_384;
const MAX_SURFACE_TOKEN_BYTES: usize = 256;
const MAX_CONTEXT_FIELD_BYTES: usize = 256;
const MAX_TRANSLATION_CONTEXT_FIELD_BYTES: usize = 4 * 1024;
const MAX_ROUTE_STATE_ENTRIES: u16 = 1_024;
const MAX_ROUTE_STEPS: u16 = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionDiagnostic {
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionTraceStatus {
    NoMatch,
    Matched,
    ContextRecorded,
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextTrace {
    Unmatched,
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontTrace {
    Unmatched,
    Protected,
    Substituted,
}

/// Allocation-free explanation of one decision and the immutable inputs that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionTrace {
    status: DecisionTraceStatus,
    text: TextTrace,
    font: FontTrace,
    generation: glyphshift_domain::Generation,
    translation_digest: SnapshotDigest,
    font_policy_digest: FontPolicyDigest,
}

impl DecisionTrace {
    fn new(
        status: DecisionTraceStatus,
        text: TextTrace,
        font: FontTrace,
        snapshot: &TranslationSnapshot,
        font_policy: &FontPolicy,
    ) -> Self {
        Self {
            status,
            text,
            font,
            generation: snapshot.generation(),
            translation_digest: snapshot.digest(),
            font_policy_digest: font_policy.digest(),
        }
    }

    #[must_use]
    pub const fn status(self) -> DecisionTraceStatus {
        self.status
    }

    #[must_use]
    pub const fn text(self) -> TextTrace {
        self.text
    }

    #[must_use]
    pub const fn font(self) -> FontTrace {
        self.font
    }

    #[must_use]
    pub const fn generation(self) -> glyphshift_domain::Generation {
        self.generation
    }

    #[must_use]
    pub const fn translation_digest(self) -> SnapshotDigest {
        self.translation_digest
    }

    #[must_use]
    pub const fn font_policy_digest(self) -> FontPolicyDigest {
        self.font_policy_digest
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionResult {
    decision: RenderDecision,
    diagnostics: Vec<DecisionDiagnostic>,
    trace: DecisionTrace,
}

impl DecisionResult {
    #[must_use]
    pub const fn decision(&self) -> &RenderDecision {
        &self.decision
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[DecisionDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub const fn trace(&self) -> DecisionTrace {
        self.trace
    }

    #[must_use]
    pub fn into_decision(self) -> RenderDecision {
        self.decision
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionState {
    contexts: BTreeMap<(Box<str>, Box<str>), Box<str>>,
}

impl DecisionState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            contexts: BTreeMap::new(),
        }
    }

    fn context_key(&self, surface: &str, kind: &str) -> Option<&str> {
        self.contexts
            .get(&(surface.into(), kind.into()))
            .map(AsRef::as_ref)
    }

    fn set_context(
        &mut self,
        surface: &str,
        kind: &str,
        key: &str,
        max_entries: usize,
    ) -> Result<(), DecisionDiagnostic> {
        let state_key = (surface.into(), kind.into());
        if !self.contexts.contains_key(&state_key) && self.contexts.len() >= max_entries {
            return Err(DecisionDiagnostic::StateLimitExceeded);
        }
        self.contexts.insert(state_key, key.into());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DecisionEngine;

impl DecisionEngine {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn decide(
        &self,
        observation: &TextObservation,
        route: &RouteProgram,
        snapshot: &TranslationSnapshot,
        font_policy: &FontPolicy,
        state: &mut DecisionState,
    ) -> DecisionResult {
        let mut result = self.decide_exact(observation, route, snapshot, font_policy, state);
        if matches!(result.trace.status, DecisionTraceStatus::NoMatch | DecisionTraceStatus::Matched)
            && observation
                .translation_context()
                .is_none_or(|context| context.plural_n().is_none())
        {
            for (location, rules) in snapshot.dictionary_rules() {
                if rules.matching_rule_index(observation.source_text()).is_none() { continue; }
                let text = rules.replace(observation.source_text(), |source|
                    snapshot.lookup_for_adapter(location, observation.adapter_id(), source));
                result.decision.text = text.map_or(TextDecision::Keep, TextDecision::Replace);
                result.trace.text = if matches!(result.decision.text, TextDecision::Keep) { TextTrace::Unmatched } else { TextTrace::Replaced };
                result.trace.status = DecisionTraceStatus::Matched;
                break;
            }
        }
        result
    }

    fn decide_exact(
        &self,
        observation: &TextObservation,
        route: &RouteProgram,
        snapshot: &TranslationSnapshot,
        font_policy: &FontPolicy,
        state: &mut DecisionState,
    ) -> DecisionResult {
        let pass = || RenderDecision {
            text: TextDecision::Keep,
            font: FontDecision::Keep,
            generation: snapshot.generation(),
        };
        let result = |decision, status, text, font, diagnostics| DecisionResult {
            decision,
            diagnostics,
            trace: DecisionTrace::new(status, text, font, snapshot, font_policy),
        };
        if !valid_observation(observation) {
            return result(
                pass(),
                DecisionTraceStatus::InvalidObservation,
                TextTrace::Unmatched,
                FontTrace::Unmatched,
                vec![DecisionDiagnostic::InvalidObservation],
            );
        }
        let limits = route.limits();
        if limits.max_state_entries() == 0
            || limits.max_state_entries() > MAX_ROUTE_STATE_ENTRIES
            || limits.max_steps() == 0
            || limits.max_steps() > MAX_ROUTE_STEPS
        {
            return result(
                pass(),
                DecisionTraceStatus::InvalidRouteProgram,
                TextTrace::Unmatched,
                FontTrace::Unmatched,
                vec![DecisionDiagnostic::InvalidRouteProgram],
            );
        }

        let mut steps = 0_u16;
        for operator in route.operators() {
            if !consume_step(&mut steps, limits.max_steps()) {
                return result(
                    pass(),
                    DecisionTraceStatus::ExecutionLimitExceeded,
                    TextTrace::Unmatched,
                    FontTrace::Unmatched,
                    vec![DecisionDiagnostic::ExecutionLimitExceeded],
                );
            }
            match operator {
                RouteOperator::Direct { location } => {
                    if let Some(matched) = decide_at(
                        location,
                        observation,
                        snapshot,
                        font_policy,
                    ) {
                        return result(
                            matched.decision,
                            DecisionTraceStatus::Matched,
                            matched.text,
                            matched.font,
                            Vec::new(),
                        );
                    }
                }
                RouteOperator::Fallback { locations } => {
                    for location in locations {
                        if !consume_step(&mut steps, limits.max_steps()) {
                            return result(
                                pass(),
                                DecisionTraceStatus::ExecutionLimitExceeded,
                                TextTrace::Unmatched,
                                FontTrace::Unmatched,
                                vec![DecisionDiagnostic::ExecutionLimitExceeded],
                            );
                        }
                        if let Some(matched) = decide_at(
                            location,
                            observation,
                            snapshot,
                            font_policy,
                        ) {
                            return result(
                                matched.decision,
                                DecisionTraceStatus::Matched,
                                matched.text,
                                matched.font,
                                Vec::new(),
                            );
                        }
                    }
                }
                RouteOperator::ContextualHeading {
                    location,
                    context_kind,
                } => {
                    if let Some(heading) = observation.context_heading() {
                        if heading.kind() == context_kind.as_ref() {
                            if let Err(diagnostic) = state.set_context(
                                observation.surface_token(),
                                context_kind,
                                heading.key(),
                                usize::from(limits.max_state_entries()),
                            ) {
                                return result(
                                    pass(),
                                    DecisionTraceStatus::StateLimitExceeded,
                                    TextTrace::Unmatched,
                                    FontTrace::Unmatched,
                                    vec![diagnostic],
                                );
                            }
                            return result(
                                pass(),
                                DecisionTraceStatus::ContextRecorded,
                                TextTrace::Unmatched,
                                FontTrace::Unmatched,
                                Vec::new(),
                            );
                        }
                        continue;
                    }
                    let Some(context_key) =
                        state.context_key(observation.surface_token(), context_kind)
                    else {
                        continue;
                    };
                    if let Some(matched) = decide_at_context(
                        location,
                        context_kind,
                        context_key,
                        observation.adapter_id(),
                        observation.source_text(),
                        snapshot,
                        font_policy,
                    ) {
                        return result(
                            matched.decision,
                            DecisionTraceStatus::Matched,
                            matched.text,
                            matched.font,
                            Vec::new(),
                        );
                    }
                }
                RouteOperator::Unknown { .. }
                | RouteOperator::NativeCode
                | RouteOperator::Script
                | RouteOperator::Io => {
                    return result(
                        pass(),
                        DecisionTraceStatus::InvalidRouteProgram,
                        TextTrace::Unmatched,
                        FontTrace::Unmatched,
                        vec![DecisionDiagnostic::InvalidRouteProgram],
                    );
                }
            }
        }
        result(
            pass(),
            DecisionTraceStatus::NoMatch,
            TextTrace::Unmatched,
            FontTrace::Unmatched,
            Vec::new(),
        )
    }
}

fn valid_observation(observation: &TextObservation) -> bool {
    if observation.adapter_id().is_empty()
        || observation.adapter_id().len() > MAX_ADAPTER_ID_BYTES
        || observation.source_text().len() > MAX_SOURCE_TEXT_BYTES
        || observation.surface_token().is_empty()
        || observation.surface_token().len() > MAX_SURFACE_TOKEN_BYTES
    {
        return false;
    }
    let heading_valid = observation.context_heading().is_none_or(|context| {
        !context.kind().is_empty()
            && context.kind().len() <= MAX_CONTEXT_FIELD_BYTES
            && !context.key().is_empty()
            && context.key().len() <= MAX_CONTEXT_FIELD_BYTES
            && !context.label().is_empty()
            && context.label().len() <= MAX_CONTEXT_FIELD_BYTES
    });
    let translation_context_valid = observation.translation_context().is_none_or(|context| {
        context.context().is_none_or(|value| {
            value.len() <= MAX_TRANSLATION_CONTEXT_FIELD_BYTES && !value.contains('\0')
        }) && context.disambiguation().is_none_or(|value| {
            value.len() <= MAX_TRANSLATION_CONTEXT_FIELD_BYTES && !value.contains('\0')
        }) && context.plural_n().is_none_or(|n| n >= 0)
    });
    heading_valid && translation_context_valid
}

fn consume_step(steps: &mut u16, max_steps: u16) -> bool {
    *steps = steps.saturating_add(1);
    *steps <= max_steps
}

fn decide_at(
    location: &str,
    observation: &TextObservation,
    snapshot: &TranslationSnapshot,
    font_policy: &FontPolicy,
) -> Option<MatchedDecision> {
    let text = observation
        .translation_context()
        .is_none_or(|context| context.plural_n().is_none())
        .then(|| {
            snapshot.lookup_for_adapter(
                location,
                observation.adapter_id(),
                observation.source_text(),
            )
        })
        .flatten();
    let font = font_policy.lookup_entry_for_adapter(
        location,
        observation.adapter_id(),
        observation.source_text(),
    );
    decision_from_parts(text, font, snapshot)
}

fn decide_at_context(
    location: &str,
    context_kind: &str,
    context_key: &str,
    adapter_id: &str,
    source: &str,
    snapshot: &TranslationSnapshot,
    font_policy: &FontPolicy,
) -> Option<MatchedDecision> {
    let text = snapshot.lookup_context_for_adapter(
        location,
        context_kind,
        context_key,
        adapter_id,
        source,
    );
    let font = font_policy.lookup_context_entry_for_adapter(
        location,
        context_kind,
        context_key,
        adapter_id,
        source,
    );
    decision_from_parts(text, font, snapshot)
}

struct MatchedDecision {
    decision: RenderDecision,
    text: TextTrace,
    font: FontTrace,
}

fn decision_from_parts(
    text: Option<std::sync::Arc<str>>,
    font: Option<FontRule>,
    snapshot: &TranslationSnapshot,
) -> Option<MatchedDecision> {
    if text.is_none() && font.is_none() {
        return None;
    }
    let text_trace = if text.is_some() {
        TextTrace::Replaced
    } else {
        TextTrace::Unmatched
    };
    let font_trace = match &font {
        None => FontTrace::Unmatched,
        Some(FontRule::Unchanged) => FontTrace::Protected,
        Some(FontRule::Substitute(_)) | Some(FontRule::Scaled { .. }) => FontTrace::Substituted,
    };
    Some(MatchedDecision {
        decision: RenderDecision {
            text: text.map_or(TextDecision::Keep, TextDecision::Replace),
            font: match font {
                None | Some(FontRule::Unchanged) => FontDecision::Keep,
                Some(FontRule::Scaled { family, percent }) => {
                    FontDecision::Scaled { family, percent }
                }
                Some(FontRule::Substitute(family)) => FontDecision::Substitute(family),
            },
            generation: snapshot.generation(),
        },
        text: text_trace,
        font: font_trace,
    })
}
