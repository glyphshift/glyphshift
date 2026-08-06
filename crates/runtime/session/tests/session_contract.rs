use glyphshift_adapter_registry::{
    AdapterBinding, AdapterDescriptor, AdapterPackage, AdapterPackageSet, AdapterRegistry,
    AdapterRequirement, AdapterTrustPolicy, AdapterVersion, AdapterVersionRequirement,
    ArtifactHash, PackageArtifactId, SignerId,
};
use glyphshift_domain::{
    AdapterId, ApplyModel, Feature, Generation, Placement, RouteProgram, TargetFacts,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterDeactivation, AdapterHostPort, BoundAdapter, BoundFeature, ControllerFailure,
    ControllerHealth, ControllerLossPolicy, ControllerRecipePort, DeactivationMode, FeaturePhase,
    GenerationPhase, HostActivation, HostDeactivation, HostFailure, HostGenerationReport,
    HostHealthReport, SessionDiagnostic, SessionError, SessionId, SessionManager, SessionRecipe,
    TargetHealth, TargetInstance, TargetInstanceId, TargetLifecyclePort,
};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

const TRUSTED_SIGNER: &str = "example.signer.trusted";

#[derive(Clone)]
struct StaticRecipePort {
    recipe: SessionRecipe,
}

impl ControllerRecipePort for StaticRecipePort {
    fn prepare(
        &mut self,
        _target: &TargetInstance,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure> {
        Ok(self.recipe.clone())
    }
}

struct QueueRecipePort {
    recipes: VecDeque<SessionRecipe>,
}

impl ControllerRecipePort for QueueRecipePort {
    fn prepare(
        &mut self,
        _target: &TargetInstance,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure> {
        self.recipes
            .pop_front()
            .ok_or(ControllerFailure::Unavailable)
    }
}

struct HealthRecipePort {
    recipe: SessionRecipe,
    health: ControllerHealth,
}

impl ControllerRecipePort for HealthRecipePort {
    fn prepare(
        &mut self,
        _target: &TargetInstance,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<SessionRecipe, ControllerFailure> {
        Ok(self.recipe.clone())
    }

    fn health(&mut self, _target: &TargetInstance) -> ControllerHealth {
        self.health
    }
}

struct RunningTargets;

impl TargetLifecyclePort for RunningTargets {
    fn health(&mut self, _target: &TargetInstance) -> TargetHealth {
        TargetHealth::Running
    }
}

struct OneExitedTarget {
    exited_target: TargetInstanceId,
}

impl TargetLifecyclePort for OneExitedTarget {
    fn health(&mut self, target: &TargetInstance) -> TargetHealth {
        if target.id() == &self.exited_target {
            TargetHealth::Exited
        } else {
            TargetHealth::Running
        }
    }
}

struct AckAllHost;

struct PublicationRecordingHost {
    generations: Arc<Mutex<Vec<Generation>>>,
}

impl AdapterHostPort for PublicationRecordingHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        panic!("runtime-aware activation must not discard the publication")
    }

    fn activate_runtime(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        self.generations
            .lock()
            .expect("publication log should remain available")
            .push(publication.generation());
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }

    fn update_runtime(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostGenerationReport, HostFailure> {
        self.generations
            .lock()
            .expect("publication log should remain available")
            .push(publication.generation());
        Ok(HostGenerationReport::target_runtime(
            publication.generation(),
        ))
    }
}

impl AdapterHostPort for AckAllHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }
}

struct ConnectedWithoutAck;

impl AdapterHostPort for ConnectedWithoutAck {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        Ok(HostActivation::connected(std::iter::empty()))
    }
}

struct AdapterHealthHost {
    failed_adapters: Vec<BoundAdapter>,
}

impl AdapterHostPort for AdapterHealthHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }

    fn health(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        Ok(HostHealthReport::failed(self.failed_adapters.clone()))
    }
}

struct DeactivationHost {
    expected: Vec<AdapterDeactivation>,
    completed: Vec<BoundAdapter>,
}

struct ReleaseRecordingHost {
    released_sessions: Arc<Mutex<Vec<SessionId>>>,
}

struct PerSessionReportingHost {
    failed_adapter: BoundAdapter,
}

impl AdapterHostPort for PerSessionReportingHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }

    fn update(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        generation: Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        Ok(HostGenerationReport::target_runtime(generation))
    }

    fn health(
        &mut self,
        session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        if session_id == SessionId::new(1) {
            Ok(HostHealthReport::reported(
                [self.failed_adapter.clone()],
                [SessionDiagnostic::new("first-target-host-failure")],
            ))
        } else {
            Ok(HostHealthReport::reported(
                std::iter::empty(),
                [SessionDiagnostic::new("second-target-healthy")],
            ))
        }
    }
}

impl AdapterHostPort for ReleaseRecordingHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }

    fn release(
        &mut self,
        session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        self.released_sessions
            .lock()
            .expect("release log lock should remain available")
            .push(session_id);
        Ok(())
    }
}

impl AdapterHostPort for DeactivationHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }

    fn deactivate(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        assert_eq!(plan, self.expected);
        Ok(HostDeactivation::completed(self.completed.clone()))
    }
}

struct ReportHost {
    acknowledged: Vec<BoundFeature>,
    failed: Vec<BoundFeature>,
}

impl AdapterHostPort for ReportHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        Ok(HostActivation::reported(
            self.acknowledged.clone(),
            self.failed.clone(),
        ))
    }
}

struct UpdateReportingHost {
    reports: VecDeque<HostGenerationReport>,
}

impl AdapterHostPort for UpdateReportingHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let acknowledgements = bindings.iter().flat_map(|binding| {
            binding.features.iter().map(|feature| {
                BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
            })
        });
        Ok(HostActivation::connected(acknowledgements))
    }

    fn update(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        _generation: Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        self.reports.pop_front().ok_or(HostFailure::Unavailable)
    }
}

fn registry_fixture() -> (
    AdapterRegistry,
    AdapterRequirement,
    AdapterId,
    AdapterVersion,
) {
    let adapter_id = AdapterId::new("example.synthetic.session");
    let version = AdapterVersion::new(1, 0, 0);
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        version,
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let hash = ArtifactHash::sha256([0x81; 32]);
    let package = AdapterPackage::new(
        descriptor,
        PackageArtifactId::new("synthetic.session.library"),
        SignerId::new(TRUSTED_SIGNER),
        hash,
        hash,
    );
    let policy = AdapterTrustPolicy::new([SignerId::new(TRUSTED_SIGNER)], [adapter_id.clone()]);
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("fixture package should load");
    let requirement = AdapterRequirement::new(
        adapter_id.clone(),
        AdapterVersionRequirement::Exact(version),
        [Feature::TextReplace],
    );

    (registry, requirement, adapter_id, version)
}
fn adapter_fixture<const N: usize>(
    adapter_id: &str,
    apply_model: ApplyModel,
    placement: Placement,
    features: [Feature; N],
    hash_byte: u8,
) -> (AdapterPackage, AdapterRequirement, AdapterId) {
    let adapter_id = AdapterId::new(adapter_id);
    let version = AdapterVersion::new(1, 0, 0);
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        version,
        apply_model,
        placement,
        features,
    );
    let hash = ArtifactHash::sha256([hash_byte; 32]);
    let package = AdapterPackage::new(
        descriptor,
        PackageArtifactId::new("synthetic.session.artifact"),
        SignerId::new(TRUSTED_SIGNER),
        hash,
        hash,
    );
    let requirement = AdapterRequirement::new(
        adapter_id.clone(),
        AdapterVersionRequirement::Exact(version),
        features,
    );

    (package, requirement, adapter_id)
}

fn isolated_session(reports: impl IntoIterator<Item = HostGenerationReport>) -> SessionManager {
    let (package, requirement, adapter_id) = adapter_fixture(
        "example.synthetic.worker-generation",
        ApplyModel::ExternalProtocol,
        Placement::IsolatedWorker,
        [Feature::TextReplace],
        0x96,
    );
    let policy = AdapterTrustPolicy::new([SignerId::new(TRUSTED_SIGNER)], [adapter_id.clone()]);
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("worker package should load");
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
    };
    SessionManager::new(
        registry,
        recipe_port,
        UpdateReportingHost {
            reports: reports.into_iter().collect(),
        },
        RunningTargets,
    )
}

fn worker_bound_feature() -> BoundFeature {
    BoundFeature::new(
        AdapterId::new("example.synthetic.worker-generation"),
        AdapterVersion::new(1, 0, 0),
        Feature::TextReplace,
    )
}

#[path = "session_contract/activation.rs"]
mod activation;
#[path = "session_contract/generation.rs"]
mod generation;
#[path = "session_contract/health.rs"]
mod health;
#[path = "session_contract/isolation.rs"]
mod isolation;
#[path = "session_contract/publication.rs"]
mod publication;
#[path = "session_contract/stop.rs"]
mod stop;
