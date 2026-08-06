use crate::*;
use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, AdapterRegistry, AdapterRequirement,
    AdapterVersionRequirement, RegistryError,
};
use glyphshift_domain::{ApplyModel, Feature, Generation};
use glyphshift_runtime_contract::RuntimePublication;
use std::collections::{BTreeMap, BTreeSet};

struct SessionRecord {
    target: TargetInstance,
    bindings: Vec<AdapterBinding>,
    controller_loss_policy: ControllerLossPolicy,
    status: SessionStatus,
}

pub struct SessionManager {
    registry: AdapterRegistry,
    controller: Box<dyn ControllerRecipePort>,
    host: Box<dyn AdapterHostPort>,
    target_lifecycle: Box<dyn TargetLifecyclePort>,
    last_session_id: u64,
    sessions: BTreeMap<SessionId, SessionRecord>,
}

impl SessionManager {
    #[must_use]
    pub fn new(
        registry: AdapterRegistry,
        controller: impl ControllerRecipePort + 'static,
        host: impl AdapterHostPort + 'static,
        target_lifecycle: impl TargetLifecyclePort + 'static,
    ) -> Self {
        Self {
            registry,
            controller: Box::new(controller),
            host: Box::new(host),
            target_lifecycle: Box::new(target_lifecycle),
            last_session_id: 0,
            sessions: BTreeMap::new(),
        }
    }

    pub fn start(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<SessionStatus, SessionError> {
        self.start_inner(target, requested_features, None)
    }

    pub fn start_with_runtime(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, SessionError> {
        self.start_inner(target, requested_features, Some(publication))
    }

    fn start_inner(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        publication: Option<&RuntimePublication>,
    ) -> Result<SessionStatus, SessionError> {
        let requested_features: BTreeSet<_> = requested_features.into_iter().collect();
        let recipe = self
            .controller
            .prepare(&target, &requested_features)
            .map_err(SessionError::Controller)?;

        let bindings: Vec<_> = recipe
            .requirements
            .iter()
            .map(|requirement| {
                self.registry
                    .resolve(requirement, target.facts())
                    .map_err(|error| {
                        if matches!(
                            error,
                            RegistryError::AdapterNotFound(_)
                                | RegistryError::AdapterVersionNotFound { .. }
                        ) {
                            let AdapterVersionRequirement::Exact(version) =
                                requirement.version_requirement();
                            SessionError::FeaturesUnavailable(
                                requirement
                                    .features()
                                    .map(|feature| {
                                        BoundFeature::new(
                                            requirement.adapter_id().clone(),
                                            version,
                                            feature,
                                        )
                                    })
                                    .collect(),
                            )
                        } else {
                            SessionError::Adapter(error)
                        }
                    })
            })
            .collect::<Result<_, _>>()?;

        for requested_feature in &requested_features {
            if !bindings
                .iter()
                .any(|binding| binding.features.contains(requested_feature))
            {
                return Err(SessionError::RequestedFeatureUnavailable(
                    *requested_feature,
                ));
            }
        }

        let activation = if let Some(publication) = publication {
            self.host.activate_runtime(&target, &bindings, publication)
        } else {
            self.host.activate(&target, &bindings)
        }
        .map_err(SessionError::Host)?;
        if let Some(conflicting_feature) = activation
            .acknowledged_features
            .intersection(&activation.failed_features)
            .next()
        {
            return Err(SessionError::ConflictingHostReport(
                conflicting_feature.clone(),
            ));
        }
        let features: BTreeMap<BoundFeature, FeaturePhase> = bindings
            .iter()
            .flat_map(|binding| {
                binding.features.iter().map(|feature| {
                    let bound_feature = BoundFeature::from_binding(binding, *feature);
                    let phase = if activation.acknowledged_features.contains(&bound_feature) {
                        FeaturePhase::Active
                    } else if activation.failed_features.contains(&bound_feature) {
                        FeaturePhase::Failed
                    } else {
                        FeaturePhase::Starting
                    };
                    (bound_feature, phase)
                })
            })
            .collect();
        let next_session_id = self
            .last_session_id
            .checked_add(1)
            .ok_or(SessionError::SessionIdExhausted)?;
        let status = SessionStatus {
            session_id: SessionId::new(next_session_id),
            target_instance_id: target.id().clone(),
            diagnostics: Vec::new(),
            generations: features
                .keys()
                .cloned()
                .map(|feature| (feature, GenerationPhase::Unreported))
                .collect(),
            features,
        };

        self.last_session_id = next_session_id;
        self.sessions.insert(
            status.session_id,
            SessionRecord {
                target,
                bindings,
                controller_loss_policy: recipe.controller_loss_policy,
                status: status.clone(),
            },
        );

        Ok(status)
    }

    pub fn update(
        &mut self,
        session_id: SessionId,
        generation: Generation,
    ) -> Result<SessionStatus, SessionError> {
        self.update_inner(session_id, generation, None)
    }

    pub fn update_with_runtime(
        &mut self,
        session_id: SessionId,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, SessionError> {
        self.update_inner(session_id, publication.generation(), Some(publication))
    }

    pub fn control_capture(
        &mut self,
        session_id: SessionId,
        paused: bool,
    ) -> Result<(), SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        self.host
            .control_capture(session_id, &target, paused)
            .map_err(SessionError::Host)
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
        enabled: bool,
    ) -> Result<(), SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        self.host
            .control_runtime_diagnostics(session_id, &target, enabled)
            .map_err(SessionError::Host)
    }

    pub fn query_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
    ) -> Result<RuntimeTraceBatch, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        self.host
            .query_runtime_diagnostics(session_id, &target)
            .map_err(SessionError::Host)
    }

    fn update_inner(
        &mut self,
        session_id: SessionId,
        generation: Generation,
        publication: Option<&RuntimePublication>,
    ) -> Result<SessionStatus, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        let bindings = record.bindings.clone();
        let report = if let Some(publication) = publication {
            self.host
                .update_runtime(session_id, &target, &bindings, publication)
        } else {
            self.host.update(session_id, &target, &bindings, generation)
        }
        .map_err(SessionError::Host)?;
        let record = self
            .sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;

        for binding in &record.bindings {
            for feature in &binding.features {
                let bound_feature = BoundFeature::from_binding(binding, *feature);
                let previous_applied = record
                    .status
                    .generations
                    .get(&bound_feature)
                    .copied()
                    .unwrap_or(GenerationPhase::Unreported)
                    .applied();
                let acknowledged = match &binding.host {
                    AdapterHostBinding::TargetProcess { .. } => report.target_runtime_ack,
                    AdapterHostBinding::IsolatedWorker { .. } => {
                        report.isolated_feature_acks.get(&bound_feature).copied()
                    }
                };
                let phase = match acknowledged {
                    Some(acknowledged) if acknowledged == generation => {
                        GenerationPhase::Applied(generation)
                    }
                    Some(acknowledged) => GenerationPhase::Mismatch {
                        desired: generation,
                        acknowledged,
                        applied: previous_applied,
                    },
                    None => GenerationPhase::Updating {
                        desired: generation,
                        applied: previous_applied,
                    },
                };
                record.status.generations.insert(bound_feature, phase);
            }
        }

        Ok(record.status.clone())
    }

    pub fn stop(&mut self, session_id: SessionId) -> Result<SessionStatus, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        let bindings = record.bindings.clone();
        let plan: Vec<_> = bindings
            .iter()
            .map(|binding| {
                let mode = match binding.apply_model {
                    ApplyModel::InlineRender => DeactivationMode::PassThrough,
                    ApplyModel::RetainedObject => DeactivationMode::RestoreOriginal,
                    ApplyModel::ExternalProtocol => DeactivationMode::StopWriteback,
                    ApplyModel::ObserveOnly => DeactivationMode::StopObserving,
                };
                AdapterDeactivation::new(
                    BoundAdapter::new(binding.adapter_id.clone(), binding.version),
                    mode,
                )
            })
            .collect();
        let report = self
            .host
            .deactivate(session_id, &target, &bindings, &plan)
            .map_err(SessionError::Host)?;
        if let Some(conflicting_adapter) = report
            .completed_adapters
            .intersection(&report.failed_adapters)
            .next()
        {
            return Err(SessionError::ConflictingHostDeactivation(
                conflicting_adapter.clone(),
            ));
        }
        let record = self
            .sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;

        for (bound_feature, phase) in &mut record.status.features {
            let adapter =
                BoundAdapter::new(bound_feature.adapter_id.clone(), bound_feature.version);
            *phase = if report.completed_adapters.contains(&adapter) {
                FeaturePhase::Ready
            } else if report.failed_adapters.contains(&adapter) {
                FeaturePhase::Failed
            } else {
                FeaturePhase::Degraded
            };
        }

        Ok(record.status.clone())
    }

    pub fn status(&mut self, session_id: SessionId) -> Result<SessionStatus, SessionError> {
        let record = self
            .sessions
            .get(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;
        let target = record.target.clone();
        let bindings = record.bindings.clone();
        let controller_loss_policy = record.controller_loss_policy;
        if self.target_lifecycle.health(&target) == TargetHealth::Exited {
            let release = self.host.release(session_id, &target, &bindings);
            self.sessions.remove(&session_id);
            return match release {
                Ok(()) => Err(SessionError::TargetExited(session_id)),
                Err(failure) => Err(SessionError::TargetReleaseFailed {
                    session_id,
                    failure,
                }),
            };
        }
        let controller_health = self.controller.health(&target);
        let host_health = self
            .host
            .health(session_id, &target, &bindings)
            .map_err(SessionError::Host)?;
        let invalidated_adapters: BTreeSet<_> = bindings
            .iter()
            .filter_map(|binding| {
                let requirement = AdapterRequirement::new(
                    binding.adapter_id.clone(),
                    AdapterVersionRequirement::Exact(binding.version),
                    binding.features.iter().copied(),
                );
                self.registry
                    .resolve(&requirement, target.facts())
                    .err()
                    .map(|_| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            })
            .collect();
        let record = self
            .sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound(session_id))?;

        match (controller_health, controller_loss_policy) {
            (ControllerHealth::Lost, ControllerLossPolicy::Continue)
            | (ControllerHealth::Available, _) => {}
            (ControllerHealth::Lost, ControllerLossPolicy::Degrade) => {
                for phase in record.status.features.values_mut() {
                    if *phase != FeaturePhase::Failed {
                        *phase = FeaturePhase::Degraded;
                    }
                }
            }
        }
        for (bound_feature, phase) in &mut record.status.features {
            let adapter =
                BoundAdapter::new(bound_feature.adapter_id.clone(), bound_feature.version);
            if (host_health.failed_adapters.contains(&adapter)
                || invalidated_adapters.contains(&adapter))
                && *phase != FeaturePhase::Failed
            {
                *phase = FeaturePhase::Degraded;
            }
        }
        record.status.diagnostics = host_health.diagnostics;

        Ok(record.status.clone())
    }
}
