use crate::IsolatedWorkerHost;
use glyphshift_adapter_registry::{AdapterBinding, AdapterHostBinding};
use glyphshift_protocol::ControllerTransport;
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterDeactivation, AdapterHostPort, BoundAdapter, BoundFeature, HostActivation,
    HostDeactivation, HostFailure, HostGenerationReport, HostHealthReport, SessionId,
    TargetInstance, TargetInstanceId,
};
use glyphshift_target_process_host::TargetProcessHost;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ActivePlacements {
    pub(super) target_process: bool,
    pub(super) isolated_worker: bool,
}

pub(super) fn resolve_runtime_activation(
    target_process: Option<Result<(), HostFailure>>,
    isolated_worker: Option<Result<(), HostFailure>>,
    target_process_features: Vec<BoundFeature>,
    isolated_worker_features: Vec<BoundFeature>,
) -> Result<(ActivePlacements, HostActivation), HostFailure> {
    let mut acknowledged = Vec::new();
    let mut failed = Vec::new();
    let mut first_error = None;

    let target_process_active = match target_process {
        Some(Ok(())) => {
            acknowledged.extend(target_process_features);
            true
        }
        Some(Err(error)) => {
            failed.extend(target_process_features);
            first_error = Some(error);
            false
        }
        None => false,
    };
    let isolated_worker_active = match isolated_worker {
        Some(Ok(())) => {
            acknowledged.extend(isolated_worker_features);
            true
        }
        Some(Err(error)) => {
            failed.extend(isolated_worker_features);
            first_error.get_or_insert(error);
            false
        }
        None => false,
    };

    let active = ActivePlacements {
        target_process: target_process_active,
        isolated_worker: isolated_worker_active,
    };
    if !active.target_process && !active.isolated_worker {
        if let Some(error) = first_error {
            return Err(error);
        }
    }

    Ok((active, HostActivation::reported(acknowledged, failed)))
}

/// One Session-facing host that routes each binding to its declared placement.
pub struct HybridAdapterHost<T> {
    target_process: TargetProcessHost<T>,
    isolated_worker: Option<IsolatedWorkerHost>,
    active: BTreeMap<TargetInstanceId, ActivePlacements>,
}

impl<T> HybridAdapterHost<T> {
    #[must_use]
    pub fn new(target_process: TargetProcessHost<T>) -> Self {
        Self {
            target_process,
            isolated_worker: None,
            active: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_isolated_worker(mut self, isolated_worker: IsolatedWorkerHost) -> Self {
        self.isolated_worker = Some(isolated_worker);
        self
    }

    fn placements(bindings: &[AdapterBinding]) -> ActivePlacements {
        ActivePlacements {
            target_process: bindings
                .iter()
                .any(|binding| matches!(binding.host, AdapterHostBinding::TargetProcess { .. })),
            isolated_worker: bindings
                .iter()
                .any(|binding| matches!(binding.host, AdapterHostBinding::IsolatedWorker { .. })),
        }
    }

    fn features(bindings: &[AdapterBinding]) -> Vec<BoundFeature> {
        bindings
            .iter()
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature)
                })
            })
            .collect()
    }

    fn features_for(
        bindings: &[AdapterBinding],
        placement: fn(&AdapterHostBinding) -> bool,
    ) -> Vec<BoundFeature> {
        bindings
            .iter()
            .filter(|binding| placement(&binding.host))
            .flat_map(|binding| {
                binding.features.iter().copied().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, feature)
                })
            })
            .collect()
    }

    fn adapters_for(
        bindings: &[AdapterBinding],
        placement: fn(&AdapterHostBinding) -> bool,
    ) -> Vec<BoundAdapter> {
        bindings
            .iter()
            .filter(|binding| placement(&binding.host))
            .map(|binding| BoundAdapter::new(binding.adapter_id.clone(), binding.version))
            .collect()
    }
}

impl<T: ControllerTransport + Send + 'static> AdapterHostPort for HybridAdapterHost<T> {
    fn activate(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let placements = Self::placements(bindings);
        if placements.target_process || self.active.contains_key(target.id()) {
            return Err(HostFailure::HandshakeRejected);
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .activate(target, bindings)?;
        }
        self.active.insert(target.id().clone(), placements);
        Ok(HostActivation::connected(Self::features(bindings)))
    }

    fn activate_runtime(
        &mut self,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        let placements = Self::placements(bindings);
        if self.active.contains_key(target.id()) {
            return Err(HostFailure::Unavailable);
        }
        let target_process = placements.target_process.then(|| {
            self.target_process
                .activate_runtime(target, bindings, publication)
                .map(|_| ())
        });
        let isolated_worker = placements.isolated_worker.then(|| {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)
                .and_then(|host| host.activate_runtime(target, bindings, publication))
                .map(|_| ())
        });
        let (active, activation) = resolve_runtime_activation(
            target_process,
            isolated_worker,
            Self::features_for(bindings, |host| {
                matches!(host, AdapterHostBinding::TargetProcess { .. })
            }),
            Self::features_for(bindings, |host| {
                matches!(host, AdapterHostBinding::IsolatedWorker { .. })
            }),
        )?;
        self.active.insert(target.id().clone(), active);
        Ok(activation)
    }

    fn update(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        generation: glyphshift_domain::Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if placements.target_process {
            return Err(HostFailure::Unavailable);
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .update(session_id, target, bindings, generation)?;
        }
        Ok(HostGenerationReport::reported(
            None,
            Self::features(bindings)
                .into_iter()
                .map(|feature| (feature, generation)),
        ))
    }

    fn update_runtime(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostGenerationReport, HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if placements.target_process {
            self.target_process
                .update_runtime(session_id, target, bindings, publication)?;
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .update_runtime(session_id, target, bindings, publication)?;
        }
        let isolated = placements
            .isolated_worker
            .then(|| {
                bindings
                    .iter()
                    .filter(|binding| {
                        matches!(binding.host, AdapterHostBinding::IsolatedWorker { .. })
                    })
                    .flat_map(|binding| {
                        binding.features.iter().copied().map(|feature| {
                            (
                                BoundFeature::new(
                                    binding.adapter_id.clone(),
                                    binding.version,
                                    feature,
                                ),
                                publication.generation(),
                            )
                        })
                    })
            })
            .into_iter()
            .flatten();
        Ok(HostGenerationReport::reported(
            placements
                .target_process
                .then_some(publication.generation()),
            isolated,
        ))
    }

    fn control_capture(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        paused: bool,
    ) -> Result<(), HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if paused {
            if placements.target_process {
                self.target_process
                    .control_capture(session_id, target, true)?;
            }
            if placements.isolated_worker
                && self
                    .isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .control_capture(session_id, target, true)
                    .is_err()
            {
                if placements.target_process {
                    let _ = self
                        .target_process
                        .control_capture(session_id, target, false);
                }
                return Err(HostFailure::Unavailable);
            }
        } else {
            if placements.isolated_worker {
                self.isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .control_capture(session_id, target, false)?;
            }
            if placements.target_process
                && self
                    .target_process
                    .control_capture(session_id, target, false)
                    .is_err()
            {
                if placements.isolated_worker {
                    let _ = self
                        .isolated_worker
                        .as_mut()
                        .and_then(|host| host.control_capture(session_id, target, true).ok());
                }
                return Err(HostFailure::Unavailable);
            }
        }
        Ok(())
    }

    fn control_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        enabled: bool,
    ) -> Result<(), HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        if !placements.target_process {
            return Err(HostFailure::Unavailable);
        }
        self.target_process
            .control_runtime_diagnostics(session_id, target, enabled)
    }

    fn query_runtime_diagnostics(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
    ) -> Result<glyphshift_session::RuntimeTraceBatch, HostFailure> {
        self.target_process
            .query_runtime_diagnostics(session_id, target)
    }

    fn health(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        let placements = self
            .active
            .get(target.id())
            .copied()
            .ok_or(HostFailure::Unavailable)?;
        let mut report = HostHealthReport::healthy();
        if placements.target_process {
            report = report.merge(self.target_process.health(session_id, target, bindings)?);
        }
        if placements.isolated_worker {
            report = report.merge(
                self.isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .health(session_id, target, bindings)?,
            );
        }
        Ok(report)
    }

    fn deactivate(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
        plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        let placements = self
            .active
            .remove(target.id())
            .ok_or(HostFailure::Unavailable)?;
        let mut report = HostDeactivation::pending();
        if placements.target_process {
            report = report.merge(
                self.target_process
                    .deactivate(session_id, target, bindings, plan)
                    .unwrap_or_else(|_| {
                        HostDeactivation::reported(
                            [],
                            Self::adapters_for(bindings, |host| {
                                matches!(host, AdapterHostBinding::TargetProcess { .. })
                            }),
                        )
                    }),
            );
        }
        if placements.isolated_worker {
            report = report.merge(
                self.isolated_worker
                    .as_mut()
                    .ok_or(HostFailure::Unavailable)?
                    .deactivate(session_id, target, bindings, plan)
                    .unwrap_or_else(|_| {
                        HostDeactivation::reported(
                            [],
                            Self::adapters_for(bindings, |host| {
                                matches!(host, AdapterHostBinding::IsolatedWorker { .. })
                            }),
                        )
                    }),
            );
        }
        Ok(report)
    }

    fn release(
        &mut self,
        session_id: SessionId,
        target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        let placements = self.active.remove(target.id()).unwrap_or(ActivePlacements {
            target_process: true,
            isolated_worker: self.isolated_worker.is_some(),
        });
        if placements.target_process {
            self.target_process.release(session_id, target, bindings)?;
        }
        if placements.isolated_worker {
            self.isolated_worker
                .as_mut()
                .ok_or(HostFailure::Unavailable)?
                .release(session_id, target, bindings)?;
        }
        Ok(())
    }
}
