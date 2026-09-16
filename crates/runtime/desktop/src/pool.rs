use super::*;

pub(super) trait RuntimeFactory: Send {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError>;

    fn supports_acquisition_adapter(&self, _adapter_id: &str) -> bool {
        false
    }
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

    fn supports_acquisition_adapter(&self, adapter_id: &str) -> bool {
        self.acquisition_worker_ids()
            .iter()
            .any(|candidate| candidate.as_ref() == adapter_id)
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
    active_target_count: usize,
    failed_target_count: usize,
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
    pub const fn active_target_count(&self) -> usize {
        self.active_target_count
    }

    #[must_use]
    pub const fn failed_target_count(&self) -> usize {
        self.failed_target_count
    }

    #[must_use]
    pub const fn is_partially_active(&self) -> bool {
        self.active_target_count > 0 && self.failed_target_count > 0
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
            active_target_count: 0,
            failed_target_count: 0,
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
    workflow_collections: BTreeMap<Box<str>, BTreeMap<Box<str>, CaptureConfiguration>>,
    active_collections: BTreeMap<Box<str>, CaptureConfiguration>,
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
            workflow_collections: BTreeMap::new(),
            active_collections: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn supports_acquisition_adapter(&self, adapter_id: &str) -> bool {
        self.factory.supports_acquisition_adapter(adapter_id)
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

    /// Runs one bounded acquisition through an isolated, short-lived Runtime.
    ///
    /// The Runtime is deliberately not inserted into `sessions`: acquisition must not borrow,
    /// replace, stop, or otherwise mutate a Workflow/Capture Controller session owned by the Pool.
    pub fn acquire_point(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: u64,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        self.acquire(
            application_id,
            spec,
            target_id,
            adapter_id,
            InteractiveSelection::Point(point),
            cancellation,
        )
    }

    fn acquire(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: u64,
        adapter_id: &str,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        if cancellation.is_cancelled() {
            return Err(DesktopAcquisitionError::Cancelled);
        }
        let mut runtime = self
            .factory
            .discover(application_id.into(), spec)
            .map_err(map_runtime_acquisition_error)?;
        runtime.acquire(target_id, adapter_id, selection, cancellation)
    }

    /// Acquires from the Pool's stable primary target selection without exposing target identity
    /// to product interaction state.
    pub fn acquire_primary_point(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        adapter_id: &str,
        point: DesktopPoint,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        self.acquire_primary(
            application_id,
            spec,
            adapter_id,
            InteractiveSelection::Point(point),
            cancellation,
        )
    }

    /// Acquires a bounded visual region from the Pool's stable primary target selection.
    pub fn acquire_primary_region(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        adapter_id: &str,
        region: DesktopRect,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        self.acquire_primary(
            application_id,
            spec,
            adapter_id,
            InteractiveSelection::Region(region),
            cancellation,
        )
    }

    fn acquire_primary(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        adapter_id: &str,
        selection: InteractiveSelection,
        cancellation: &DesktopAcquisitionCancellation,
    ) -> Result<AcquisitionResult, DesktopAcquisitionError> {
        if cancellation.is_cancelled() {
            return Err(DesktopAcquisitionError::Cancelled);
        }
        let mut runtime = self
            .factory
            .discover(application_id.into(), spec)
            .map_err(map_runtime_acquisition_error)?;
        let target_id = runtime
            .targets()
            .first()
            .map(RuntimeTarget::id)
            .ok_or(DesktopAcquisitionError::UnknownTarget)?;
        runtime.acquire(target_id, adapter_id, selection, cancellation)
    }

    pub fn set_features(
        &mut self,
        application_id: impl Into<Box<str>>,
        spec: &DesktopRuntimeSpec,
        target_id: Option<u64>,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        self.set_features_with_collection(application_id.into(), spec, target_id, requested_features.into_iter().collect(), None)
    }

    fn set_features_with_collection(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
        target_id: Option<u64>,
        mut requested_features: BTreeSet<Feature>,
        collection: Option<CaptureConfiguration>,
    ) -> Result<DesktopRuntimeStatus, DesktopRuntimeError> {
        if collection.is_some() { requested_features.insert(Feature::TextObserve); }
        if requested_features.is_empty() {
            return self.stop_application(application_id);
        }

        let expired = if let Some(runtime) = self.sessions.get_mut(application_id.as_ref()) {
            !runtime.refresh_liveness()?
        } else { false };
        if expired {
            self.sessions.remove(application_id.as_ref());
            self.active_collections.remove(application_id.as_ref());
            self.requested_features.remove(application_id.as_ref());
        }

        let must_replace = self
            .sessions
            .get(application_id.as_ref())
            .is_some_and(|runtime| {
                runtime.is_active() && (runtime.active_features() != requested_features
                    || self.active_collections.get(application_id.as_ref()) != collection.as_ref())
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
        let Some(target_id) = target_id.or_else(|| runtime.targets().first().map(RuntimeTarget::id)) else {
            return Err(DesktopRuntimeError::UnknownTarget);
        };
        if let Some(configuration) = collection {
            let target_ids = runtime.targets().iter().map(RuntimeTarget::id).collect::<Vec<_>>();
            if let Err(error) = runtime.start_capture(&target_ids, &requested_features, configuration.clone()) {
                // Failed activation may consume the controller connection. Retry from discovery.
                if !runtime.is_active() {
                    self.sessions.remove(application_id.as_ref());
                    self.active_collections.remove(application_id.as_ref());
                    self.requested_features.remove(application_id.as_ref());
                }
                return Err(error);
            }
            self.active_collections.insert(application_id.clone(), configuration);
        } else {
            if let Err(error) = runtime.start(target_id, &requested_features) {
                if !runtime.is_active() {
                    self.sessions.remove(application_id.as_ref());
                    self.active_collections.remove(application_id.as_ref());
                    self.requested_features.remove(application_id.as_ref());
                }
                return Err(error);
            }
            self.active_collections.remove(application_id.as_ref());
        }
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

    /// Collection shares the workflow session and never acquires a second target owner.
    pub fn control_workflow_collection(&mut self, workflow_id: &str, software_id: &str, paused: bool) -> Result<(), DesktopRuntimeError> {
        if !self.workflow_targets.get(workflow_id).is_some_and(|targets| targets.contains(software_id))
            || !self.active_collections.contains_key(software_id) { return Err(DesktopRuntimeError::InvalidState); }
        self.sessions.get_mut(software_id).ok_or(DesktopRuntimeError::InvalidState)?.control_capture(paused)
    }

    pub fn configure_workflow_collection(
        &mut self,
        workflow_id: impl Into<Box<str>>,
        collections: BTreeMap<Box<str>, CaptureConfiguration>,
    ) {
        self.workflow_collections.insert(workflow_id.into(), collections);
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
            return Err(DesktopRuntimeError::TargetInUse(
                TargetExecutionOwner::Capture,
            ));
        }
        if self.workflow_targets.iter().any(|(workflow_id, owned)| {
            workflow_id.as_ref() != intent.workflow_id()
                && owned
                    .iter()
                    .any(|software_id| target_ids.contains(software_id))
        }) {
            return Err(DesktopRuntimeError::TargetInUse(
                TargetExecutionOwner::Workflow,
            ));
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
            let collection = self.workflow_collections.get(intent.workflow_id())
                .and_then(|collections| collections.get(target.software_id())).cloned();
            match self.set_features_with_collection(
                target.software_id().into(),
                target.runtime_spec(),
                None,
                requested_features,
                collection,
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

    pub fn workflow_owns_target(&self, workflow_id: &str, software_id: &str) -> bool {
        self.workflow_targets.get(workflow_id).is_some_and(|targets| targets.contains(software_id))
    }

    pub fn stop_workflow(
        &mut self,
        workflow_id: &str,
    ) -> Result<WorkflowReconcileReport, DesktopRuntimeError> {
        let target_ids = self
            .workflow_targets
            .get(workflow_id).cloned()
            .unwrap_or_default();
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
        if errors.is_empty() { self.workflow_targets.remove(workflow_id); }
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
        let previous_collection = self.active_collections.remove(application_id.as_ref());
        let Some(mut runtime) = self.sessions.remove(application_id.as_ref()) else {
            self.requested_features.remove(application_id.as_ref());
            return Ok(DesktopRuntimeStatus::inactive(application_id));
        };
        if runtime.is_active() {
            if let Err(error) = runtime.stop() {
                if let Some(configuration) = previous_collection { self.active_collections.insert(application_id.clone(), configuration); }
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
        if self.capture_targets.contains(application_id.as_ref()) {
            return Err(DesktopRuntimeError::TargetInUse(
                TargetExecutionOwner::Capture,
            ));
        }
        if self
            .workflow_targets
            .values()
            .any(|targets| targets.contains(application_id.as_ref()))
        {
            return Err(DesktopRuntimeError::TargetInUse(
                TargetExecutionOwner::Workflow,
            ));
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
            self.discard_session(application_id.as_ref());
            return Err(DesktopRuntimeError::UnknownTarget);
        }
        let mut requested_features = BTreeSet::from([Feature::TextObserve]);
        if status.supports(Feature::TextReplace) {
            requested_features.insert(Feature::TextReplace);
        }
        if let Err(error) = runtime.start_capture(&target_ids, &requested_features, capture) {
            self.discard_session(application_id.as_ref());
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

    fn discard_session(&mut self, application_id: &str) {
        if let Some(mut runtime) = self.sessions.remove(application_id) {
            runtime.abandon();
        }
        self.requested_features.remove(application_id);
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

    pub fn abandon_capture(&mut self, application_id: &str) {
        self.capture_targets.remove(application_id);
        self.discard_session(application_id);
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
        // Polling must preserve live translation and capture sessions. Reconcile only
        // applies changed configuration/publications or starts an inactive session.
        self.reconcile_workflow(intent)
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
        active_target_count: runtime.active_target_count(),
        failed_target_count: runtime.failed_target_count(),
        applied_generation: runtime.applied_generation(),
    }
}

const fn map_runtime_acquisition_error(error: DesktopRuntimeError) -> DesktopAcquisitionError {
    match error {
        DesktopRuntimeError::UnknownTarget => DesktopAcquisitionError::UnknownTarget,
        DesktopRuntimeError::AcquisitionWorkerUnavailable => {
            DesktopAcquisitionError::WorkerUnavailable
        }
        DesktopRuntimeError::TargetInUse(_) | DesktopRuntimeError::InvalidState => {
            DesktopAcquisitionError::InvalidState
        }
        DesktopRuntimeError::ControllerUnavailable
        | DesktopRuntimeError::ControllerRejected
        | DesktopRuntimeError::ProtocolRejected
        | DesktopRuntimeError::SessionRejected
        | DesktopRuntimeError::ActivationRejected(_)
        | DesktopRuntimeError::BundleUnavailable
        | DesktopRuntimeError::InvalidManifest
        | DesktopRuntimeError::InvalidArtifactPath
        | DesktopRuntimeError::InvalidArtifactHash
        | DesktopRuntimeError::ArtifactHashMismatch
        | DesktopRuntimeError::AdapterAbiMismatch
        | DesktopRuntimeError::AdapterInspectionFailed
        | DesktopRuntimeError::AdapterRegistryRejected => {
            DesktopAcquisitionError::ControllerUnavailable
        }
    }
}
