//! Target-process composition of verified bindings and immutable decision inputs.

use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding};
use glyphshift_decision::{DecisionEngine, DecisionResult, DecisionState, DecisionTrace};
use glyphshift_domain::{
    AdapterId, FontDecision, Generation, RenderDecision, RouteProgram, TextDecision,
    TextObservation,
};
use glyphshift_runtime_contract::{RuntimePublication, RuntimePublicationIdentity};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

const MAX_DECISION_TRACES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeDecisionTrace {
    adapter_id: Box<str>,
    source_text: Box<str>,
    decision: RenderDecision,
    trace: DecisionTrace,
    publication_identity: RuntimePublicationIdentity,
}

impl RuntimeDecisionTrace {
    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    #[must_use]
    pub const fn decision(&self) -> &RenderDecision {
        &self.decision
    }

    #[must_use]
    pub const fn trace(&self) -> DecisionTrace {
        self.trace
    }

    #[must_use]
    pub const fn publication_identity(&self) -> RuntimePublicationIdentity {
        self.publication_identity
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeDecisionTraceBatch {
    records: Vec<RuntimeDecisionTrace>,
    dropped: u64,
}

impl RuntimeDecisionTraceBatch {
    #[must_use]
    pub fn records(&self) -> &[RuntimeDecisionTrace] {
        &self.records
    }

    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeKernelError {
    InvalidPublication,
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
    publication_identity: RuntimePublicationIdentity,
    decision_engine: DecisionEngine,
    decision_state: Mutex<DecisionState>,
    decision_tracing: AtomicBool,
    decision_traces: Mutex<VecDeque<RuntimeDecisionTrace>>,
    dropped_decision_traces: AtomicU64,
}

impl RuntimeKernel {
    pub fn from_publication(
        bindings: impl IntoIterator<Item = AdapterBinding>,
        publication: RuntimePublication,
    ) -> Result<Self, RuntimeKernelError> {
        let publication_identity = publication
            .identity()
            .map_err(|_| RuntimeKernelError::InvalidPublication)?;
        let (route, snapshot, font_policy) = publication.into_parts();
        Self::activate_with_identity(bindings, route, snapshot, font_policy, publication_identity)
    }

    pub fn activate(
        bindings: impl IntoIterator<Item = AdapterBinding>,
        route: RouteProgram,
        snapshot: TranslationSnapshot,
        font_policy: FontPolicy,
    ) -> Result<Self, RuntimeKernelError> {
        let publication_identity =
            RuntimePublication::new(route.clone(), snapshot.clone(), font_policy.clone())
                .identity()
                .map_err(|_| RuntimeKernelError::InvalidPublication)?;
        Self::activate_with_identity(bindings, route, snapshot, font_policy, publication_identity)
    }

    fn activate_with_identity(
        bindings: impl IntoIterator<Item = AdapterBinding>,
        route: RouteProgram,
        snapshot: TranslationSnapshot,
        font_policy: FontPolicy,
        publication_identity: RuntimePublicationIdentity,
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
            publication_identity,
            decision_engine: DecisionEngine::new(),
            decision_state: Mutex::new(DecisionState::new()),
            decision_tracing: AtomicBool::new(false),
            decision_traces: Mutex::new(VecDeque::with_capacity(MAX_DECISION_TRACES)),
            dropped_decision_traces: AtomicU64::new(0),
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
        let result = self.decision_engine.decide(
            observation,
            &self.route,
            &self.snapshot,
            &self.font_policy,
            &mut state,
        );
        self.record_decision_trace(observation, &result);
        result.into_decision()
    }

    /// Enables or disables bounded decision tracing without changing decision behaviour.
    pub fn set_decision_tracing(&self, enabled: bool) {
        self.decision_tracing.store(enabled, Ordering::Release);
        if let Ok(mut traces) = self.decision_traces.lock() {
            traces.clear();
        }
        self.dropped_decision_traces.store(0, Ordering::Relaxed);
    }

    /// Drains the current diagnostic window. This is an off-hot-path operation.
    #[must_use]
    pub fn drain_decision_traces(&self) -> RuntimeDecisionTraceBatch {
        let records = self
            .decision_traces
            .lock()
            .map(|mut traces| traces.drain(..).collect())
            .unwrap_or_default();
        RuntimeDecisionTraceBatch {
            records,
            dropped: self.dropped_decision_traces.swap(0, Ordering::Relaxed),
        }
    }

    fn record_decision_trace(&self, observation: &TextObservation, result: &DecisionResult) {
        if !self.decision_tracing.load(Ordering::Acquire) {
            return;
        }
        let record = RuntimeDecisionTrace {
            adapter_id: observation.adapter_id().into(),
            source_text: observation.source_text().into(),
            decision: result.decision().clone(),
            trace: result.trace(),
            publication_identity: self.publication_identity,
        };
        let Ok(mut traces) = self.decision_traces.try_lock() else {
            self.dropped_decision_traces.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if traces.len() == MAX_DECISION_TRACES {
            traces.pop_front();
            self.dropped_decision_traces.fetch_add(1, Ordering::Relaxed);
        }
        traces.push_back(record);
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
        let publication_identity =
            RuntimePublication::new(self.route.clone(), snapshot.clone(), font_policy.clone())
                .identity()
                .map_err(|_| RuntimeKernelError::InvalidPublication)?;
        self.snapshot = snapshot;
        self.font_policy = font_policy;
        self.publication_identity = publication_identity;
        Ok(incoming)
    }

    pub fn apply_publication(
        &mut self,
        publication: RuntimePublication,
    ) -> Result<Generation, RuntimeKernelError> {
        let publication_identity = publication
            .identity()
            .map_err(|_| RuntimeKernelError::InvalidPublication)?;
        let (route, snapshot, font_policy) = publication.into_parts();
        let current = self.snapshot.generation();
        let incoming = snapshot.generation();
        if incoming <= current {
            return Err(RuntimeKernelError::StaleGeneration { current, incoming });
        }
        self.route = route;
        self.snapshot = snapshot;
        self.font_policy = font_policy;
        self.publication_identity = publication_identity;
        Ok(incoming)
    }

    #[must_use]
    pub fn bindings(&self) -> &[AdapterBinding] {
        &self.bindings
    }
}
