//! Host-independent service composition root.

use glyphshift_adapter_registry::{AdapterPackageSet, AdapterRegistry, RegistryError};
use glyphshift_domain::{Feature, Generation, RegistryRevision};
use glyphshift_extension::{
    ExtensionPackageKind, ExtensionPackageSet, ExtensionRegistry, ExtensionRegistryError,
    ExtensionRequirement,
};
use glyphshift_protocol::{PreparedRecipe, RecipeControllerLossPolicy};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterHostPort, ControllerFailure, ControllerLossPolicy, ControllerRecipePort, SessionError,
    SessionId, SessionManager, SessionRecipe, SessionStatus, TargetInstance, TargetInstanceId,
    TargetLifecyclePort,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct ServiceRecipePort {
    recipes: Arc<Mutex<BTreeMap<TargetInstanceId, SessionRecipe>>>,
}

impl ControllerRecipePort for ServiceRecipePort {
    fn prepare(
        &mut self,
        target: &TargetInstance,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure> {
        self.recipes
            .lock()
            .map_err(|_| ControllerFailure::Unavailable)?
            .remove(target.id())
            .ok_or(ControllerFailure::InvalidRecipe)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    Extension(ExtensionRegistryError),
    Adapter(RegistryError),
    Session(SessionError),
    ExtensionNotSoftware,
    RecipeQueueUnavailable,
}

pub struct GlyphshiftService {
    extensions: ExtensionRegistry,
    adapters: AdapterRegistry,
    recipes: Arc<Mutex<BTreeMap<TargetInstanceId, SessionRecipe>>>,
    sessions: SessionManager,
}

impl GlyphshiftService {
    #[must_use]
    pub fn new(
        extensions: ExtensionRegistry,
        adapters: AdapterRegistry,
        host: impl AdapterHostPort + 'static,
        target_lifecycle: impl TargetLifecyclePort + 'static,
    ) -> Self {
        let recipes = Arc::new(Mutex::new(BTreeMap::new()));
        let recipe_port = ServiceRecipePort {
            recipes: Arc::clone(&recipes),
        };
        let sessions = SessionManager::new(adapters.clone(), recipe_port, host, target_lifecycle);
        Self {
            extensions,
            adapters,
            recipes,
            sessions,
        }
    }

    pub fn reload_adapters(
        &mut self,
        packages: AdapterPackageSet,
    ) -> Result<RegistryRevision, ServiceError> {
        self.adapters
            .reload(packages)
            .map_err(ServiceError::Adapter)
    }

    pub fn reload_extensions(
        &mut self,
        packages: ExtensionPackageSet,
    ) -> Result<u64, ServiceError> {
        self.extensions
            .reload(packages)
            .map_err(ServiceError::Extension)
    }

    pub fn start_extension_session(
        &mut self,
        extension: &ExtensionRequirement,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<SessionStatus, ServiceError> {
        let resolved = self
            .extensions
            .resolve(extension)
            .map_err(ServiceError::Extension)?;
        if resolved.kind() != ExtensionPackageKind::Software {
            return Err(ServiceError::ExtensionNotSoftware);
        }
        let recipe = SessionRecipe::new(
            resolved.capability_requirements().iter().cloned(),
            ControllerLossPolicy::Continue,
        );
        self.start_with_recipe(target, requested_features, recipe)
    }

    pub fn start_extension_session_with_runtime(
        &mut self,
        extension: &ExtensionRequirement,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, ServiceError> {
        let resolved = self
            .extensions
            .resolve(extension)
            .map_err(ServiceError::Extension)?;
        if resolved.kind() != ExtensionPackageKind::Software {
            return Err(ServiceError::ExtensionNotSoftware);
        }
        let recipe = SessionRecipe::new(
            resolved.capability_requirements().iter().cloned(),
            ControllerLossPolicy::Continue,
        );
        self.start_with_recipe_and_runtime(target, requested_features, recipe, publication)
    }

    pub fn start_prepared_session(
        &mut self,
        prepared: &PreparedRecipe,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
    ) -> Result<SessionStatus, ServiceError> {
        let controller_loss_policy = match prepared.controller_loss_policy() {
            RecipeControllerLossPolicy::Continue => ControllerLossPolicy::Continue,
            RecipeControllerLossPolicy::Degrade => ControllerLossPolicy::Degrade,
        };
        let recipe = SessionRecipe::new(
            prepared.requirements().iter().cloned(),
            controller_loss_policy,
        );
        self.start_with_recipe(target, requested_features, recipe)
    }

    pub fn update(
        &mut self,
        session_id: SessionId,
        generation: Generation,
    ) -> Result<SessionStatus, ServiceError> {
        self.sessions
            .update(session_id, generation)
            .map_err(ServiceError::Session)
    }

    pub fn update_with_runtime(
        &mut self,
        session_id: SessionId,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, ServiceError> {
        self.sessions
            .update_with_runtime(session_id, publication)
            .map_err(ServiceError::Session)
    }

    pub fn stop(&mut self, session_id: SessionId) -> Result<SessionStatus, ServiceError> {
        self.sessions
            .stop(session_id)
            .map_err(ServiceError::Session)
    }

    pub fn status(&mut self, session_id: SessionId) -> Result<SessionStatus, ServiceError> {
        self.sessions
            .status(session_id)
            .map_err(ServiceError::Session)
    }

    fn start_with_recipe(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        recipe: SessionRecipe,
    ) -> Result<SessionStatus, ServiceError> {
        self.recipes
            .lock()
            .map_err(|_| ServiceError::RecipeQueueUnavailable)?
            .insert(target.id().clone(), recipe);
        self.sessions
            .start(target, requested_features)
            .map_err(ServiceError::Session)
    }

    fn start_with_recipe_and_runtime(
        &mut self,
        target: TargetInstance,
        requested_features: impl IntoIterator<Item = Feature>,
        recipe: SessionRecipe,
        publication: &RuntimePublication,
    ) -> Result<SessionStatus, ServiceError> {
        self.recipes
            .lock()
            .map_err(|_| ServiceError::RecipeQueueUnavailable)?
            .insert(target.id().clone(), recipe);
        self.sessions
            .start_with_runtime(target, requested_features, publication)
            .map_err(ServiceError::Session)
    }
}
