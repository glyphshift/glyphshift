//! Target-process composition of verified bindings and immutable decision inputs.

use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding};
use glyphshift_decision::{DecisionEngine, DecisionState};
use glyphshift_domain::{
    AdapterId, FontDecision, Generation, RenderDecision, RouteProgram, TextDecision,
    TextObservation,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeKernelError {
    UnsupportedBinding(AdapterId),
    StaleGeneration {
        current: Generation,
        incoming: Generation,
    },
}

#[derive(Debug)]
pub struct RuntimeKernel {
    bindings: Vec<AdapterBinding>,
    route: RouteProgram,
    snapshot: TranslationSnapshot,
    font_policy: FontPolicy,
    decision_engine: DecisionEngine,
    decision_state: Mutex<DecisionState>,
}

impl RuntimeKernel {
    pub fn from_publication(
        bindings: impl IntoIterator<Item = AdapterBinding>,
        publication: RuntimePublication,
    ) -> Result<Self, RuntimeKernelError> {
        let (route, snapshot, font_policy) = publication.into_parts();
        Self::activate(bindings, route, snapshot, font_policy)
    }

    pub fn activate(
        bindings: impl IntoIterator<Item = AdapterBinding>,
        route: RouteProgram,
        snapshot: TranslationSnapshot,
        font_policy: FontPolicy,
    ) -> Result<Self, RuntimeKernelError> {
        let bindings: Vec<_> = bindings.into_iter().collect();
        if let Some(binding) = bindings
            .iter()
            .find(|binding| !matches!(binding.host, AdapterHostBinding::TargetProcess { .. }))
        {
            return Err(RuntimeKernelError::UnsupportedBinding(
                binding.adapter_id.clone(),
            ));
        }
        Ok(Self {
            bindings,
            route,
            snapshot,
            font_policy,
            decision_engine: DecisionEngine::new(),
            decision_state: Mutex::new(DecisionState::new()),
        })
    }

    #[must_use]
    pub fn decide(&self, observation: &TextObservation) -> RenderDecision {
        let Ok(mut state) = self.decision_state.lock() else {
            return RenderDecision {
                text: TextDecision::Keep,
                font: FontDecision::Keep,
                generation: self.snapshot.generation(),
            };
        };
        self.decision_engine
            .decide(
                observation,
                &self.route,
                &self.snapshot,
                &self.font_policy,
                &mut state,
            )
            .into_decision()
    }

    pub fn update(
        &mut self,
        snapshot: TranslationSnapshot,
        font_policy: FontPolicy,
    ) -> Result<Generation, RuntimeKernelError> {
        let current = self.snapshot.generation();
        let incoming = snapshot.generation();
        if incoming <= current {
            return Err(RuntimeKernelError::StaleGeneration { current, incoming });
        }
        self.snapshot = snapshot;
        self.font_policy = font_policy;
        Ok(incoming)
    }

    pub fn apply_publication(
        &mut self,
        publication: RuntimePublication,
    ) -> Result<Generation, RuntimeKernelError> {
        let (route, snapshot, font_policy) = publication.into_parts();
        self.route = route;
        self.update(snapshot, font_policy)
    }

    #[must_use]
    pub fn bindings(&self) -> &[AdapterBinding] {
        &self.bindings
    }
}
