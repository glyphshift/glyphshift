use super::*;

pub trait ControllerTransport {
    fn handshake(
        &mut self,
        expected_extension: &ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure>;

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn launch(
        &mut self,
        _installation: &ControllerInstallationToken,
    ) -> Result<ControllerLaunchAck, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn prepare(
        &mut self,
        _target: &ControllerTargetToken,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<ControllerRecipe, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn authorize_worker_target(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<ControllerWorkerTargetGrant, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn activate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
        _deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn update_runtime(
        &mut self,
        _target: &ControllerTargetToken,
        _publication_json: &str,
        _generation: u64,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn control_capture(
        &mut self,
        _target: &ControllerTargetToken,
        _paused: bool,
    ) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn control_runtime_diagnostics(
        &mut self,
        _target: &ControllerTargetToken,
        _enabled: bool,
    ) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn query_runtime_diagnostics(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<ControllerRuntimeTraceBatch, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn query_observations(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<CaptureObservationBatch, TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn deactivate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn cancel(&mut self, _operation: &ControllerOperation) -> Result<(), TransportFailure> {
        Err(TransportFailure::MalformedMessage)
    }

    fn terminate(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerHealth {
    Available,
    Degraded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerProtocolError {
    NonceReplay(ControllerNonce),
    VersionMismatch,
    NonceMismatch,
    ExtensionMismatch,
    UnknownInstallation(InstallationId),
    UnknownTarget(OpaqueTargetId),
    InvalidRecipe(RecipeViolation),
    Transport(TransportFailure),
}

#[derive(Debug, Default)]
pub struct NonceLedger {
    accepted: BTreeSet<ControllerNonce>,
}

impl NonceLedger {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            accepted: BTreeSet::new(),
        }
    }
}

#[derive(Debug)]
pub struct ControllerConnection<T> {
    transport: T,
    extension_id: ExtensionId,
    health: ControllerHealth,
    installation_tokens: BTreeMap<InstallationId, ControllerInstallationToken>,
    target_tokens: BTreeMap<OpaqueTargetId, ControllerTargetToken>,
    authorized_requirements: Vec<AdapterRequirement>,
    cancelled_inventory: bool,
    cancelled_targets: BTreeSet<OpaqueTargetId>,
}

impl<T: ControllerTransport> ControllerConnection<T> {
    pub fn connect(
        mut transport: T,
        extension_id: ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
        ledger: &mut NonceLedger,
    ) -> Result<Self, ControllerProtocolError> {
        if ledger.accepted.contains(&nonce) {
            transport.terminate();
            return Err(ControllerProtocolError::NonceReplay(nonce));
        }
        let hello = transport
            .handshake(&extension_id, version, nonce)
            .map_err(ControllerProtocolError::Transport)?;
        let rejection = if hello.version != version {
            Some(ControllerProtocolError::VersionMismatch)
        } else if hello.nonce != nonce {
            Some(ControllerProtocolError::NonceMismatch)
        } else if hello.extension_id != extension_id {
            Some(ControllerProtocolError::ExtensionMismatch)
        } else {
            None
        };
        if let Some(rejection) = rejection {
            transport.terminate();
            return Err(rejection);
        }
        ledger.accepted.insert(nonce);
        Ok(Self {
            transport,
            extension_id,
            health: ControllerHealth::Available,
            installation_tokens: BTreeMap::new(),
            target_tokens: BTreeMap::new(),
            authorized_requirements: Vec::new(),
            cancelled_inventory: false,
            cancelled_targets: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    #[must_use]
    pub const fn health(&self) -> ControllerHealth {
        self.health
    }

    pub fn inventory(&mut self) -> Result<InventoryView, ControllerProtocolError> {
        if std::mem::take(&mut self.cancelled_inventory) {
            return Err(ControllerProtocolError::Transport(
                TransportFailure::Cancelled,
            ));
        }
        let inventory = match self.transport.inventory() {
            Ok(inventory) => inventory,
            Err(failure) => return Err(self.handle_transport_failure(failure)),
        };
        self.installation_tokens.clear();
        self.target_tokens.clear();
        let installations = inventory
            .installations
            .into_iter()
            .enumerate()
            .map(|(index, installation)| {
                let id = InstallationId(index as u64 + 1);
                self.installation_tokens
                    .insert(id, installation.token.clone());
                DiscoveredInstallation {
                    id,
                    display_name: installation.display_name,
                }
            })
            .collect();
        let targets = inventory
            .targets
            .into_iter()
            .enumerate()
            .map(|(index, target)| {
                let id = OpaqueTargetId(index as u64 + 1);
                self.target_tokens.insert(id, target.token.clone());
                DiscoveredTarget {
                    id,
                    display_name: target.display_name,
                    facts: target.facts,
                }
            })
            .collect();
        Ok(InventoryView {
            installations,
            targets,
        })
    }

    pub fn launch(
        &mut self,
        installation_id: InstallationId,
    ) -> Result<LaunchReceipt, ControllerProtocolError> {
        let token = self
            .installation_tokens
            .get(&installation_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownInstallation(
                installation_id,
            ))?;
        if let Err(failure) = self.transport.launch(&token) {
            return Err(self.handle_transport_failure(failure));
        }
        Ok(LaunchReceipt {
            installation_id,
            state: LaunchState::Requested,
        })
    }

    pub fn authorize_capabilities(
        &mut self,
        requirements: impl IntoIterator<Item = AdapterRequirement>,
    ) {
        self.authorized_requirements = requirements.into_iter().collect();
    }

    pub fn prepare(
        &mut self,
        target_id: OpaqueTargetId,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<PreparedRecipe, ControllerProtocolError> {
        if self.cancelled_targets.remove(&target_id) {
            return Err(ControllerProtocolError::Transport(
                TransportFailure::Cancelled,
            ));
        }
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        let requested_features: BTreeSet<_> = requested_features.into_iter().collect();
        let recipe = match self.transport.prepare(&target, &requested_features) {
            Ok(recipe) => recipe,
            Err(failure) => return Err(self.handle_transport_failure(failure)),
        };
        let mut requirements = Vec::new();
        for directive in recipe.directives {
            let requirement = match directive {
                RecipeDirective::Adapter(requirement) => requirement,
                RecipeDirective::FunctionAddress(_) => {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::FunctionAddress,
                    ));
                }
                RecipeDirective::HookCode => {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::HookCode,
                    ));
                }
                RecipeDirective::Script => {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::Script,
                    ));
                }
            };
            let authorized = self.authorized_requirements.iter().find(|authorized| {
                authorized.adapter_id() == requirement.adapter_id()
                    && authorized.version_requirement() == requirement.version_requirement()
            });
            let Some(authorized) = authorized else {
                return Err(ControllerProtocolError::InvalidRecipe(
                    RecipeViolation::UnknownAdapter(requirement.adapter_id().clone()),
                ));
            };
            for feature in requirement.features() {
                if !authorized
                    .features()
                    .any(|authorized_feature| authorized_feature == feature)
                {
                    return Err(ControllerProtocolError::InvalidRecipe(
                        RecipeViolation::UnauthorizedFeature {
                            adapter_id: requirement.adapter_id().clone(),
                            feature,
                        },
                    ));
                }
            }
            requirements.push(requirement);
        }
        Ok(PreparedRecipe {
            requirements,
            controller_loss_policy: recipe.controller_loss_policy,
        })
    }

    pub fn activate_runtime(
        &mut self,
        target_id: OpaqueTargetId,
        deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .activate_runtime(&target, deployment)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn authorize_worker_target(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<ControllerWorkerTargetGrant, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .authorize_worker_target(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn update_runtime(
        &mut self,
        target_id: OpaqueTargetId,
        publication_json: &str,
        generation: u64,
    ) -> Result<ControllerRuntimeAck, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .update_runtime(&target, publication_json, generation)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn control_capture(
        &mut self,
        target_id: OpaqueTargetId,
        paused: bool,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .control_capture(&target, paused)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn control_runtime_diagnostics(
        &mut self,
        target_id: OpaqueTargetId,
        enabled: bool,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .control_runtime_diagnostics(&target, enabled)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn query_runtime_diagnostics(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<ControllerRuntimeTraceBatch, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .query_runtime_diagnostics(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn query_observations(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<CaptureObservationBatch, ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .query_observations(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn deactivate_runtime(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        self.transport
            .deactivate_runtime(&target)
            .map_err(|failure| self.handle_transport_failure(failure))
    }

    pub fn cancel_inventory(&mut self) -> Result<(), ControllerProtocolError> {
        if let Err(failure) = self.transport.cancel(&ControllerOperation::Inventory) {
            return Err(self.handle_transport_failure(failure));
        }
        self.cancelled_inventory = true;
        Ok(())
    }

    pub fn cancel_prepare(
        &mut self,
        target_id: OpaqueTargetId,
    ) -> Result<(), ControllerProtocolError> {
        let target = self
            .target_tokens
            .get(&target_id)
            .cloned()
            .ok_or(ControllerProtocolError::UnknownTarget(target_id))?;
        if let Err(failure) = self.transport.cancel(&ControllerOperation::Prepare(target)) {
            return Err(self.handle_transport_failure(failure));
        }
        self.cancelled_targets.insert(target_id);
        Ok(())
    }

    fn handle_transport_failure(&mut self, failure: TransportFailure) -> ControllerProtocolError {
        if !matches!(
            failure,
            TransportFailure::Cancelled | TransportFailure::Rejected(_)
        ) {
            self.health = ControllerHealth::Degraded;
            self.transport.terminate();
        }
        ControllerProtocolError::Transport(failure)
    }

    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }
}
