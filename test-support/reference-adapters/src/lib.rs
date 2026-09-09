//! Executable reference implementations of each Adapter apply model.

use glyphshift_adapter_sdk::{
    authorize, ActivationGrant, AdapterError, DrawCommand, InlineTextInput, ObjectId,
    ObservationId, ProbeEvidence, SyntheticTarget,
};
use glyphshift_domain::{
    ApplyModel, Feature, FontDecision, Generation, Placement, RenderDecision, TextDecision,
};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineReferenceAdapter {
    active_features: Vec<Feature>,
}

impl InlineReferenceAdapter {
    #[must_use]
    pub fn probe(target: &SyntheticTarget) -> ProbeEvidence {
        ProbeEvidence::new(
            ApplyModel::InlineRender,
            Placement::TargetProcess,
            [Feature::TextReplace, Feature::FontSubstitute],
            target,
        )
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            active_features: authorize(requested, grant)?,
        })
    }

    pub fn invoke<R>(
        &mut self,
        input: InlineTextInput,
        decide: impl FnOnce(&str) -> Result<RenderDecision, AdapterError>,
        original: impl FnOnce(DrawCommand) -> R,
    ) -> R {
        let original_command = input.original().clone();
        let command = input
            .decode()
            .ok()
            .and_then(|text| catch_unwind(AssertUnwindSafe(|| decide(&text))).ok())
            .and_then(Result::ok)
            .map_or_else(
                || original_command.clone(),
                |decision| apply_decision(&original_command, &decision, &self.active_features),
            );
        original(command)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RetainedObject {
    original: DrawCommand,
    current: DrawCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedReferenceAdapter {
    active_features: Vec<Feature>,
    objects: BTreeMap<ObjectId, RetainedObject>,
}

impl RetainedReferenceAdapter {
    #[must_use]
    pub fn probe(target: &SyntheticTarget) -> ProbeEvidence {
        ProbeEvidence::new(
            ApplyModel::RetainedObject,
            Placement::TargetProcess,
            [Feature::TextReplace, Feature::FontSubstitute],
            target,
        )
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            active_features: authorize(requested, grant)?,
            objects: BTreeMap::new(),
        })
    }

    pub fn create(
        &mut self,
        object_id: ObjectId,
        original: DrawCommand,
        decision: RenderDecision,
    ) -> Result<(), AdapterError> {
        if self.objects.contains_key(&object_id) {
            return Err(AdapterError::ObjectAlreadyExists);
        }
        let current = apply_decision(&original, &decision, &self.active_features);
        self.objects
            .insert(object_id, RetainedObject { original, current });
        Ok(())
    }

    pub fn apply(
        &mut self,
        object_id: &ObjectId,
        decision: RenderDecision,
    ) -> Result<(), AdapterError> {
        let object = self
            .objects
            .get_mut(object_id)
            .ok_or(AdapterError::ObjectNotFound)?;
        object.current = apply_decision(&object.original, &decision, &self.active_features);
        Ok(())
    }

    pub fn destroy(&mut self, object_id: &ObjectId) -> Result<(), AdapterError> {
        self.objects
            .remove(object_id)
            .map(|_| ())
            .ok_or(AdapterError::ObjectNotFound)
    }

    #[must_use]
    pub fn object(&self, object_id: &ObjectId) -> Option<&DrawCommand> {
        self.objects.get(object_id).map(|object| &object.current)
    }

    pub fn deactivate(&mut self) -> Vec<(ObjectId, DrawCommand)> {
        std::mem::take(&mut self.objects)
            .into_iter()
            .map(|(object_id, object)| (object_id, object.original))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalReferenceAdapter {
    active_features: Vec<Feature>,
    connected: bool,
    degraded: bool,
    next_observation_id: u64,
    objects: BTreeMap<ObjectId, DrawCommand>,
    observations: BTreeMap<ObservationId, ObjectId>,
    applied_generations: BTreeMap<ObservationId, Generation>,
}

impl ExternalReferenceAdapter {
    #[must_use]
    pub fn probe(target: &SyntheticTarget) -> ProbeEvidence {
        ProbeEvidence::new(
            ApplyModel::ExternalProtocol,
            Placement::IsolatedWorker,
            [Feature::TextReplace, Feature::FontSubstitute],
            target,
        )
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            active_features: authorize(requested, grant)?,
            connected: true,
            degraded: false,
            next_observation_id: 0,
            objects: BTreeMap::new(),
            observations: BTreeMap::new(),
            applied_generations: BTreeMap::new(),
        })
    }

    pub fn observe(&mut self, object_id: ObjectId, original: DrawCommand) -> ObservationId {
        self.observations
            .retain(|_, observed_object| observed_object != &object_id);
        self.objects.insert(object_id.clone(), original);
        self.next_observation_id += 1;
        let observation_id = ObservationId::new(self.next_observation_id);
        self.observations.insert(observation_id, object_id);
        observation_id
    }

    pub fn apply(
        &mut self,
        observation_id: ObservationId,
        generation: Generation,
        decision: RenderDecision,
    ) -> Result<Generation, AdapterError> {
        if !self.connected {
            return Err(AdapterError::Disconnected);
        }
        let object_id = self
            .observations
            .get(&observation_id)
            .ok_or(AdapterError::ObservationExpired)?;
        let object = self
            .objects
            .get_mut(object_id)
            .ok_or(AdapterError::ObservationExpired)?;
        *object = apply_decision(object, &decision, &self.active_features);
        self.applied_generations.insert(observation_id, generation);
        Ok(generation)
    }

    pub fn invalidate(&mut self, object_id: &ObjectId) {
        self.objects.remove(object_id);
        self.observations.retain(|observation_id, observed_object| {
            if observed_object == object_id {
                self.applied_generations.remove(observation_id);
                false
            } else {
                true
            }
        });
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.degraded = true;
    }

    #[must_use]
    pub const fn is_degraded(&self) -> bool {
        self.degraded
    }

    #[must_use]
    pub fn object(&self, object_id: &ObjectId) -> Option<&DrawCommand> {
        self.objects.get(object_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObserveReferenceAdapter {
    active_features: Vec<Feature>,
    observation_count: usize,
}

impl ObserveReferenceAdapter {
    #[must_use]
    pub fn probe(target: &SyntheticTarget) -> ProbeEvidence {
        ProbeEvidence::new(
            ApplyModel::ObserveOnly,
            Placement::IsolatedWorker,
            [Feature::TextObserve],
            target,
        )
    }

    pub fn activate(
        requested: impl IntoIterator<Item = Feature>,
        grant: &ActivationGrant,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            active_features: authorize(requested, grant)?,
            observation_count: 0,
        })
    }

    pub fn observe(&mut self, _text: &str) {
        self.observation_count = self.observation_count.saturating_add(1);
    }

    #[must_use]
    pub const fn observation_count(&self) -> usize {
        self.observation_count
    }

    #[must_use]
    pub fn active_features(&self) -> &[Feature] {
        &self.active_features
    }
}

fn apply_decision(
    original: &DrawCommand,
    decision: &RenderDecision,
    active_features: &[Feature],
) -> DrawCommand {
    let text = match &decision.text {
        TextDecision::Replace(text) if active_features.contains(&Feature::TextReplace) => {
            text.as_ref()
        }
        TextDecision::Keep | TextDecision::Replace(_) => original.text(),
    };
    let font = match &decision.font {
        FontDecision::Substitute(font) if active_features.contains(&Feature::FontSubstitute) => {
            font.as_ref()
        }
        FontDecision::Keep | FontDecision::Substitute(_) | FontDecision::Scaled { .. } => original.font(),
    };
    DrawCommand::new(text, font)
}
