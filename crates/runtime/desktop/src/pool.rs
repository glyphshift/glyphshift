use super::*;

pub(super) trait RuntimeFactory: Send {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError>;
}

impl RuntimeFactory for RuntimeBundle {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
        RuntimeBundle::discover(self, application_id, spec)
            .map(|runtime| Box::new(runtime) as Box<dyn ManagedRuntime>)
    }
}

/// Path- and process-private desktop state for one registered application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopRuntimeStatus {
    application_id: Box<str>,
    targets: Vec<RuntimeTarget>,
    active_target_id: Option<u64>,
    supported_features: BTreeSet<Feature>,
    requested_features: BTreeSet<Feature>,
    active_features: BTreeSet<Feature>,
    applied_generation: Option<Generation>,
}

impl DesktopRuntimeStatus {
    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    pub fn targets(&self) -> impl Iterator<Item = &RuntimeTarget> {
        self.targets.iter()
    }

    #[must_use]
    pub fn active_target_id(&self) -> Option<u64> {
        self.active_target_id
    }

    #[must_use]
    pub fn supports(&self, feature: Feature) -> bool {
        self.supported_features.contains(&feature)
    }

    #[must_use]
    pub fn is_feature_active(&self, feature: Feature) -> bool {
        self.active_features.contains(&feature)
    }

    #[must_use]
    pub fn is_feature_requested(&self, feature: Feature) -> bool {
        self.requested_features.contains(&feature)
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active_target_id.is_some() && !self.active_features.is_empty()
    }

    #[must_use]
    pub const fn applied_generation(&self) -> Option<Generation> {
        self.applied_generation
    }

    fn inactive(application_id: Box<str>) -> Self {
        Self {
            application_id,
            targets: Vec::new(),
            active_target_id: None,
            supported_features: BTreeSet::new(),
            requested_features: BTreeSet::new(),
            active_features: BTreeSet::new(),
            applied_generation: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowReconcileReport {
    workflow_id: Box<str>,
    statuses: Vec<DesktopRuntimeStatus>,
    errors: BTreeMap<Box<str>, DesktopRuntimeError>,
}

impl WorkflowReconcileReport {
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    #[must_use]
    pub fn statuses(&self) -> &[DesktopRuntimeStatus] {
        &self.statuses
    }

    #[must_use]
    pub const fn errors(&self) -> &BTreeMap<Box<str>, DesktopRuntimeError> {
        &self.errors
    }
}

/// Owns independent Runtime sessions for every enabled application.
///
/// The desktop shell expresses desired feature state per application. This module owns discovery,
/// target selection, session replacement, publication, and isolated shutdown.
pub struct DesktopRuntimePool {
    factory: Box<dyn RuntimeFactory>,
    sessions: BTreeMap<Box<str>, Box<dyn ManagedRuntime>>,
    requested_features: BTreeMap<Box<str>, BTreeSet<Feature>>,
    workflow_targets: BTreeMap<Box<str>, BTreeSet<Box<str>>>,
    capture_targets: BTreeSet<Box<str>>,
}

impl DesktopRuntimePool {
    #[must_use]
    pub fn new(bundle: RuntimeBundle) -> Self {
        Self::with_factory(Box::new(bundle))
    }

    pub(super) fn with_factory(factory: Box<dyn RuntimeFactory>) -> Self {
        Self {
            factory,
            sessions: BTreeMap::new(),
            requested_features: BTreeMap::new(),
            workflow_targets: BTreeMap::new(),
            capture_targets: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn status(&self, application_id: &str) -> Option<DesktopRuntimeStatus> {
        self.sessions.get(application_id).map(|runtime| {
            runtime_status(
                runtime.as_ref(),
                self.requested_features
                    .get(application_id)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
    }

    pub fn statuses(&self) -> impl Iterator<Item = DesktopRuntimeStatus> + '_ {
        self.sessions.iter().map(|(application_id, runtime)| {
            runtime_status(
                runtime.as_ref(),
                self.requested_features
                    .get(application_id)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
    }

    pub fn discover(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        if !self.sessions.contains_key(application_id.as_ref()) {
            let runtime = self.factory.discover(application_id.clone(), spec)?;
            self.sessions.insert(application_id.clone(), runtime);
        }
        self.status(&application_id)
            .ok_or(DesktopRuntimeError::InvalidState)
    }

    pub fn set_features(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: Option<u64>,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        let requested_features = requested_features.into_iter().collect::<BTreeSet<_>>();
        if requested_features.is_empty() {
            return self.stop_application(application_id);
        }

        let must_replace = self
            .sessions
            .get(application_id.as_ref())
            .is_some_and(|runtime| {
                runtime.is_active() && runtime.active_features() != requested_features
            });
        if must_replace {
            let mut runtime = self
                .sessions
                .remove(application_id.as_ref())
                .ok_or(DesktopRuntimeError::InvalidState)?;
            if let Err(error) = runtime.stop() {
                self.sessions.insert(application_id.clone(), runtime);
                return Err(error);
            }
        }

        self.discover(application_id.clone(), spec)?;
        let runtime = self
            .sessions
            .get_mut(application_id.as_ref())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if runtime.is_active() {
            if runtime
                .applied_generation()
                .is_none_or(|generation| spec.publication().generation() > generation)
            {
                runtime.publish(spec.publication().clone())?;
            }
            self.requested_features
                .insert(application_id.clone(), requested_features.clone());
            return Ok(runtime_status(runtime.as_ref(), requested_features));
        }
        let target_id = target_id
            .or_else(|| runtime.targets().first().map(RuntimeTarget::id))
            .ok_or(DesktopRuntimeError::UnknownTarget)?;
        runtime.start(target_id, &requested_features)?;
        self.requested_features
            .insert(application_id.clone(), runtime.active_features());
        Ok(runtime_status(
            runtime.as_ref(),
            self.requested_features
                .get(application_id.as_ref())
                .cloned()
                .unwrap_or_default(),
        ))
    }

    pub fn reconcile_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let target_ids = intent
            .targets()
            .iter()
            .map(|target| Box::<str>::from(target.software_id()))
            .collect::<BTreeSet<_>>();
        if target_ids
            .iter()
            .any(|software_id| self.capture_targets.contains(software_id))
        {
            return Err(DesktopRuntimeError::InvalidState);
        }
        if self.workflow_targets.iter().any(|(workflow_id, owned)| {
            workflow_id.as_ref() != intent.workflow_id()
                && owned
                    .iter()
                    .any(|software_id| target_ids.contains(software_id))
        }) {
            return Err(DesktopRuntimeError::InvalidState);
        }

        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        let previous_targets = self
            .workflow_targets
            .get(intent.workflow_id())
            .cloned()
            .unwrap_or_default();
        for software_id in previous_targets.difference(&target_ids) {
            match self.stop_application(software_id.clone()) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(software_id.clone(), error);
                }
            }
        }
        for target in intent.targets() {
            let requested_features = target
                .requested_features()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            self.requested_features
                .insert(target.software_id().into(), requested_features.clone());
            match self.set_features(
                target.software_id(),
                target.runtime_spec(),
                None,
                requested_features,
            ) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(target.software_id().into(), error);
                }
            }
        }
        self.workflow_targets
            .insert(intent.workflow_id().into(), target_ids);
        Ok(WorkflowReconcileReport {
            workflow_id: intent.workflow_id().into(),
            statuses,
            errors,
        })
    }

    pub fn stop_workflow(
        &mut self,
        workflow_id: &str,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let target_ids = self
            .workflow_targets
            .remove(workflow_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        for software_id in target_ids {
            match self.stop_application(software_id.clone()) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(software_id, error);
                }
            }
        }
        Ok(WorkflowReconcileReport {
            workflow_id: workflow_id.into(),
            statuses,
            errors,
        })
    }

    pub fn replace_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let replacement_targets = intent
            .targets()
            .iter()
            .map(|target| target.software_id())
            .collect::<BTreeSet<_>>();
        let replaced_workflow_ids = self
            .workflow_targets
            .iter()
            .filter(|(workflow_id, targets)| {
                workflow_id.as_ref() != intent.workflow_id()
                    && targets
                        .iter()
                        .any(|software_id| replacement_targets.contains(software_id.as_ref()))
            })
            .map(|(workflow_id, _)| workflow_id.clone())
            .collect::<Vec<_>>();
        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        for workflow_id in replaced_workflow_ids {
            let stopped = self.stop_workflow(&workflow_id)?;
            statuses.extend(stopped.statuses);
            errors.extend(stopped.errors);
        }
        let reconciled = self.reconcile_workflow(intent)?;
        statuses.extend(reconciled.statuses);
        errors.extend(reconciled.errors);
        Ok(WorkflowReconcileReport {
            workflow_id: intent.workflow_id().into(),
            statuses,
            errors,
        })
    }

    fn stop_application(
        &mut self,
        application_id: Box<str>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let Some(mut runtime) = self.sessions.remove(application_id.as_ref()) else {
            self.requested_features.remove(application_id.as_ref());
            return Ok(DesktopRuntimeStatus::inactive(application_id));
        };
        if runtime.is_active() {
            if let Err(error) = runtime.stop() {
                self.sessions.insert(application_id, runtime);
                return Err(error);
            }
        }
        self.requested_features.remove(application_id.as_ref());
        Ok(runtime_status(runtime.as_ref(), BTreeSet::new()))
    }

    pub fn start_capture(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: Option<u64>,
        capture: CaptureConfiguration,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        if self.capture_targets.contains(application_id.as_ref())
            || self
                .workflow_targets
                .values()
                .any(|targets| targets.contains(application_id.as_ref()))
        {
            return Err(DesktopRuntimeError::InvalidState);
        }
        let status = self.discover(application_id.clone(), spec)?;
        if !status.supports(Feature::TextObserve) || status.is_active() {
            return Err(DesktopRuntimeError::SessionRejected);
        }
        let runtime = self
            .sessions
            .get_mut(application_id.as_ref())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        let target_ids = if let Some(target_id) = target_id {
            vec![target_id]
        } else {
            runtime
                .targets()
                .into_iter()
                .map(|target| target.id())
                .collect::<Vec<_>>()
        };
        if target_ids.is_empty() {
            return Err(DesktopRuntimeError::UnknownTarget);
        }
        let mut requested_features = BTreeSet::from([Feature::TextObserve]);
        if status.supports(Feature::TextReplace) {
            requested_features.insert(Feature::TextReplace);
        }
        if let Err(error) = runtime.start_capture(&target_ids, &requested_features, capture) {
            if let Some(mut failed) = self.sessions.remove(application_id.as_ref()) {
                failed.abandon();
            }
            self.requested_features.remove(application_id.as_ref());
            return Err(error);
        }
        let runtime = self
            .sessions
            .get(application_id.as_ref())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        self.requested_features
            .insert(application_id.clone(), requested_features.clone());
        self.capture_targets.insert(application_id.clone());
        Ok(runtime_status(runtime.as_ref(), requested_features))
    }

    pub fn stop_capture(
        &mut self,
        application_id: impl Into<Box<str>>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        if !self.capture_targets.remove(application_id.as_ref()) {
            return Err(DesktopRuntimeError::InvalidState);
        }
        self.stop_application(application_id)
    }

    pub fn control_capture(
        &mut self,
        application_id: &str,
        paused: bool,
    ) -> Result<(), DesktopRuntimeError> {
        if !self.capture_targets.contains(application_id) {
            return Err(DesktopRuntimeError::InvalidState);
        }
        self.sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?
            .control_capture(paused)
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        application_id: &str,
        enabled: bool,
    ) -> Result<(), DesktopRuntimeError> {
        let runtime = self
            .sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if !runtime.is_active() {
            return Err(DesktopRuntimeError::InvalidState);
        }
        runtime.control_runtime_diagnostics(enabled)
    }

    pub fn query_runtime_diagnostics(
        &mut self,
        application_id: &str,
    ) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        let runtime = self
            .sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if !runtime.is_active() {
            return Err(DesktopRuntimeError::InvalidState);
        }
        runtime.query_runtime_diagnostics()
    }

    pub fn refresh(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        let application_id = application_id.into();
        let requested_features = self
            .requested_features
            .get(application_id.as_ref())
            .cloned()
            .unwrap_or_default();
        if let Some(mut runtime) = self.sessions.remove(application_id.as_ref()) {
            if runtime.is_active() && runtime.stop().is_err() {
                runtime.abandon();
            }
        }

        let mut runtime = self.factory.discover(application_id.clone(), spec)?;
        if !requested_features.is_empty() {
            let target_id = runtime.targets().first().map(RuntimeTarget::id);
            if let Some(target_id) = target_id {
                runtime.start(target_id, &requested_features)?;
            }
        }
        let status = runtime_status(runtime.as_ref(), requested_features);
        self.sessions.insert(application_id, runtime);
        Ok(status)
    }

    pub fn refresh_workflow(
        &mut self,
        intent: &EffectiveWorkflowIntent,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let owned = self
            .workflow_targets
            .get(intent.workflow_id())
            .ok_or(DesktopRuntimeError::InvalidState)?;
        if intent
            .targets()
            .iter()
            .any(|target| !owned.contains(target.software_id()))
        {
            return Err(DesktopRuntimeError::InvalidState);
        }
        let mut statuses = Vec::new();
        let mut errors = BTreeMap::new();
        for target in intent.targets() {
            match self.refresh(target.software_id(), target.runtime_spec()) {
                Ok(status) => statuses.push(status),
                Err(error) => {
                    errors.insert(target.software_id().into(), error);
                }
            }
        }
        Ok(WorkflowReconcileReport {
            workflow_id: intent.workflow_id().into(),
            statuses,
            errors,
        })
    }

    pub fn publish(
        &mut self,
        application_id: &str,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        let runtime = self
            .sessions
            .get_mut(application_id)
            .ok_or(DesktopRuntimeError::InvalidState)?;
        runtime.publish(publication)
    }

    pub fn remove(&mut self, application_id: &str) -> Result<(), DesktopRuntimeError> {
        let Some(mut runtime) = self.sessions.remove(application_id) else {
            self.requested_features.remove(application_id);
            return Ok(());
        };
        if runtime.is_active() {
            if let Err(error) = runtime.stop() {
                self.sessions.insert(application_id.into(), runtime);
                return Err(error);
            }
        }
        self.requested_features.remove(application_id);
        self.capture_targets.remove(application_id);
        Ok(())
    }
}

fn runtime_status(
    runtime: &dyn ManagedRuntime,
    requested_features: BTreeSet<Feature>,
) -> DesktopRuntimeStatus {
    DesktopRuntimeStatus {
        application_id: runtime.application_id().into(),
        targets: runtime.targets(),
        active_target_id: runtime.active_target_id(),
        supported_features: runtime.supported_features(),
        requested_features,
        active_features: runtime.active_features(),
        applied_generation: runtime.applied_generation(),
    }
}
