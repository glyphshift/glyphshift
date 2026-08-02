//! Pure Route, lookup, and render-decision semantics.

use glyphshift_domain::{
    FontDecision, RenderDecision, RouteOperator, RouteProgram, TextDecision, TextObservation,
};
use glyphshift_translation::{FontPolicy, FontRule, TranslationSnapshot};
use std::collections::BTreeMap;

const MAX_ADAPTER_ID_BYTES: usize = 256;
const MAX_SOURCE_TEXT_BYTES: usize = 16_384;
const MAX_SURFACE_TOKEN_BYTES: usize = 256;
const MAX_CONTEXT_FIELD_BYTES: usize = 256;
const MAX_ROUTE_STATE_ENTRIES: u16 = 1_024;
const MAX_ROUTE_STEPS: u16 = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionDiagnostic {
    InvalidObservation,
    InvalidRouteProgram,
    ExecutionLimitExceeded,
    StateLimitExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionResult {
    decision: RenderDecision,
    diagnostics: Vec<DecisionDiagnostic>,
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
        let pass = || RenderDecision {
            text: TextDecision::Keep,
            font: FontDecision::Keep,
            generation: snapshot.generation(),
        };
        if !valid_observation(observation) {
            return DecisionResult {
                decision: pass(),
                diagnostics: vec![DecisionDiagnostic::InvalidObservation],
            };
        }
        let limits = route.limits();
        if limits.max_state_entries() == 0
            || limits.max_state_entries() > MAX_ROUTE_STATE_ENTRIES
            || limits.max_steps() == 0
            || limits.max_steps() > MAX_ROUTE_STEPS
        {
            return DecisionResult {
                decision: pass(),
                diagnostics: vec![DecisionDiagnostic::InvalidRouteProgram],
            };
        }

        let mut steps = 0_u16;
        for operator in route.operators() {
            if !consume_step(&mut steps, limits.max_steps()) {
                return execution_limited(pass());
            }
            match operator {
                RouteOperator::Direct { location } => {
                    if let Some(decision) = decide_at(
                        location,
                        observation.adapter_id(),
                        observation.source_text(),
                        snapshot,
                        font_policy,
                    ) {
                        return successful(decision);
                    }
                }
                RouteOperator::Fallback { locations } => {
                    for location in locations {
                        if !consume_step(&mut steps, limits.max_steps()) {
                            return execution_limited(pass());
                        }
                        if let Some(decision) = decide_at(
                            location,
                            observation.adapter_id(),
                            observation.source_text(),
                            snapshot,
                            font_policy,
                        ) {
                            return successful(decision);
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
                                return DecisionResult {
                                    decision: pass(),
                                    diagnostics: vec![diagnostic],
                                };
                            }
                            return successful(pass());
                        }
                        continue;
                    }
                    let Some(context_key) =
                        state.context_key(observation.surface_token(), context_kind)
                    else {
                        continue;
                    };
                    if let Some(decision) = decide_at_context(
                        location,
                        context_kind,
                        context_key,
                        observation.adapter_id(),
                        observation.source_text(),
                        snapshot,
                        font_policy,
                    ) {
                        return successful(decision);
                    }
                }
                RouteOperator::Unknown { .. }
                | RouteOperator::NativeCode
                | RouteOperator::Script
                | RouteOperator::Io => {
                    return DecisionResult {
                        decision: pass(),
                        diagnostics: vec![DecisionDiagnostic::InvalidRouteProgram],
                    };
                }
            }
        }
        successful(pass())
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
    observation.context_heading().is_none_or(|context| {
        !context.kind().is_empty()
            && context.kind().len() <= MAX_CONTEXT_FIELD_BYTES
            && !context.key().is_empty()
            && context.key().len() <= MAX_CONTEXT_FIELD_BYTES
            && !context.label().is_empty()
            && context.label().len() <= MAX_CONTEXT_FIELD_BYTES
    })
}

fn consume_step(steps: &mut u16, max_steps: u16) -> bool {
    *steps = steps.saturating_add(1);
    *steps <= max_steps
}

fn decide_at(
    location: &str,
    adapter_id: &str,
    source: &str,
    snapshot: &TranslationSnapshot,
    font_policy: &FontPolicy,
) -> Option<RenderDecision> {
    let text = snapshot.lookup_for_adapter(location, adapter_id, source);
    let font = font_policy.lookup_entry_for_adapter(location, adapter_id, source);
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
) -> Option<RenderDecision> {
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

fn decision_from_parts(
    text: Option<std::sync::Arc<str>>,
    font: Option<FontRule>,
    snapshot: &TranslationSnapshot,
) -> Option<RenderDecision> {
    if text.is_none() && font.is_none() {
        return None;
    }
    Some(RenderDecision {
        text: text.map_or(TextDecision::Keep, TextDecision::Replace),
        font: match font {
            None | Some(FontRule::Unchanged) => FontDecision::Keep,
            Some(FontRule::Substitute(family)) => FontDecision::Substitute(family),
        },
        generation: snapshot.generation(),
    })
}

fn successful(decision: RenderDecision) -> DecisionResult {
    DecisionResult {
        decision,
        diagnostics: Vec::new(),
    }
}

fn execution_limited(decision: RenderDecision) -> DecisionResult {
    DecisionResult {
        decision,
        diagnostics: vec![DecisionDiagnostic::ExecutionLimitExceeded],
    }
}
