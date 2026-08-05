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

#[test]
fn ses_016_delivers_complete_runtime_publications_to_the_host() {
    let (registry, requirement, _, _) = registry_fixture();
    let generations = Arc::new(Mutex::new(Vec::new()));
    let mut manager = SessionManager::new(
        registry,
        StaticRecipePort {
            recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
        },
        PublicationRecordingHost {
            generations: Arc::clone(&generations),
        },
        RunningTargets,
    );
    let first = RuntimePublication::new(
        RouteProgram::direct("main-ui"),
        TranslationSnapshot::empty(Generation::new(1)).with_entry("main-ui", "File", "文件"),
        FontPolicy::empty(),
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("opaque-target"),
        TargetFacts::new("windows", "x86_64"),
    );

    let status = manager
        .start_with_runtime(target, [Feature::TextReplace], &first)
        .expect("a complete runtime publication should reach activation");

    let second = RuntimePublication::new(
        RouteProgram::direct("main-ui"),
        TranslationSnapshot::empty(Generation::new(2)).with_entry("main-ui", "File", "档案"),
        FontPolicy::empty(),
    );
    manager
        .update_with_runtime(status.session_id(), &second)
        .expect("the next complete runtime publication should reach update");

    assert_eq!(
        generations
            .lock()
            .expect("publication log should remain available")
            .as_slice(),
        &[Generation::new(1), Generation::new(2)]
    );
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

#[test]
fn ses_001_marks_a_bound_feature_active_only_after_host_ack() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
    };
    let mut manager = SessionManager::new(registry, recipe_port, AckAllHost, RunningTargets);
    let target_id = TargetInstanceId::new("target-instance-1");
    let target = TargetInstance::new(target_id.clone(), TargetFacts::new("windows", "x86_64"));

    let status = manager
        .start(target, [Feature::TextReplace])
        .expect("fully acknowledged session should start");

    assert_eq!(status.target_instance_id(), &target_id);
    assert_eq!(
        status.phase(&BoundFeature::new(
            adapter_id,
            version,
            Feature::TextReplace,
        )),
        Some(FeaturePhase::Active)
    );
}

#[test]
fn ses_002_keeps_a_connected_but_unacknowledged_feature_starting() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
    };
    let mut manager =
        SessionManager::new(registry, recipe_port, ConnectedWithoutAck, RunningTargets);
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-2"),
        TargetFacts::new("windows", "x86_64"),
    );

    let status = manager
        .start(target, [Feature::TextReplace])
        .expect("connected session should exist before feature ACK");

    assert_eq!(
        status.phase(&BoundFeature::new(
            adapter_id,
            version,
            Feature::TextReplace,
        )),
        Some(FeaturePhase::Starting)
    );
}

#[test]
fn ses_003_observe_only_activity_does_not_promote_writeback_features() {
    let (observe_package, observe_requirement, observe_id) = adapter_fixture(
        "example.synthetic.observe-session",
        ApplyModel::ObserveOnly,
        Placement::IsolatedWorker,
        [Feature::TextObserve],
        0x91,
    );
    let (write_package, write_requirement, write_id) = adapter_fixture(
        "example.synthetic.write-session",
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
        0x92,
    );
    let version = AdapterVersion::new(1, 0, 0);
    let policy = AdapterTrustPolicy::new(
        [SignerId::new(TRUSTED_SIGNER)],
        [observe_id.clone(), write_id.clone()],
    );
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([observe_package, write_package]))
        .expect("both packages should load");
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new(
            [observe_requirement, write_requirement],
            ControllerLossPolicy::Continue,
        ),
    };
    let observe_feature = BoundFeature::new(observe_id, version, Feature::TextObserve);
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        ReportHost {
            acknowledged: vec![observe_feature.clone()],
            failed: Vec::new(),
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-3"),
        TargetFacts::new("windows", "x86_64"),
    );

    let status = manager
        .start(target, [Feature::TextReplace])
        .expect("observe activity must not block the writeback session");

    assert_eq!(status.phase(&observe_feature), Some(FeaturePhase::Active));
    assert_eq!(
        status.phase(&BoundFeature::new(write_id, version, Feature::TextReplace,)),
        Some(FeaturePhase::Starting)
    );
}

#[test]
fn ses_004_preserves_each_adapter_feature_phase_independently() {
    let (text_package, text_requirement, text_id) = adapter_fixture(
        "example.synthetic.text-surface",
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
        0x93,
    );
    let (combined_package, combined_requirement, combined_id) = adapter_fixture(
        "example.synthetic.combined-surface",
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace, Feature::FontSubstitute],
        0x94,
    );
    let version = AdapterVersion::new(1, 0, 0);
    let text_active = BoundFeature::new(text_id.clone(), version, Feature::TextReplace);
    let combined_text_failed =
        BoundFeature::new(combined_id.clone(), version, Feature::TextReplace);
    let combined_font_active =
        BoundFeature::new(combined_id.clone(), version, Feature::FontSubstitute);
    let policy = AdapterTrustPolicy::new([SignerId::new(TRUSTED_SIGNER)], [text_id, combined_id]);
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([text_package, combined_package]))
        .expect("both packages should load");
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new(
            [text_requirement, combined_requirement],
            ControllerLossPolicy::Continue,
        ),
    };
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        ReportHost {
            acknowledged: vec![text_active.clone(), combined_font_active.clone()],
            failed: vec![combined_text_failed.clone()],
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-4"),
        TargetFacts::new("windows", "x86_64"),
    );

    let status = manager
        .start(target, [Feature::TextReplace, Feature::FontSubstitute])
        .expect("partial adapter outcomes should produce a session status");

    assert_eq!(status.phase(&text_active), Some(FeaturePhase::Active));
    assert_eq!(
        status.phase(&combined_text_failed),
        Some(FeaturePhase::Failed)
    );
    assert_eq!(
        status.phase(&combined_font_active),
        Some(FeaturePhase::Active)
    );
    assert_eq!(
        status.active_features().collect::<BTreeSet<_>>(),
        BTreeSet::from([Feature::TextReplace, Feature::FontSubstitute])
    );
}

#[test]
fn ses_005_reports_unavailable_without_creating_a_fake_session() {
    let (registry, installed_requirement, installed_id, version) = registry_fixture();
    let missing_id = AdapterId::new("example.synthetic.missing-session");
    let missing_requirement = AdapterRequirement::new(
        missing_id.clone(),
        AdapterVersionRequirement::Exact(version),
        [Feature::TextReplace],
    );
    let recipe_port = QueueRecipePort {
        recipes: VecDeque::from([
            SessionRecipe::new([missing_requirement], ControllerLossPolicy::Continue),
            SessionRecipe::new([installed_requirement], ControllerLossPolicy::Continue),
        ]),
    };
    let mut manager = SessionManager::new(registry, recipe_port, AckAllHost, RunningTargets);
    let first_target = TargetInstance::new(
        TargetInstanceId::new("target-instance-5"),
        TargetFacts::new("windows", "x86_64"),
    );

    let rejection = manager
        .start(first_target, [Feature::TextReplace])
        .expect_err("missing adapter must reject session creation");

    assert_eq!(
        rejection,
        SessionError::FeaturesUnavailable(vec![BoundFeature::new(
            missing_id,
            version,
            Feature::TextReplace,
        )])
    );
    assert_eq!(
        manager.status(SessionId::new(1)),
        Err(SessionError::SessionNotFound(SessionId::new(1)))
    );

    let second_target = TargetInstance::new(
        TargetInstanceId::new("target-instance-6"),
        TargetFacts::new("windows", "x86_64"),
    );
    let status = manager
        .start(second_target, [Feature::TextReplace])
        .expect("a later valid request should start normally");

    assert_eq!(status.session_id(), SessionId::new(1));
    assert_eq!(
        status.phase(&BoundFeature::new(
            installed_id,
            version,
            Feature::TextReplace,
        )),
        Some(FeaturePhase::Active)
    );
}

#[test]
fn ses_006_reports_mismatch_when_target_runtime_acks_an_old_generation() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
    };
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        UpdateReportingHost {
            reports: VecDeque::from([HostGenerationReport::target_runtime(Generation::new(1))]),
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-7"),
        TargetFacts::new("windows", "x86_64"),
    );
    let status = manager
        .start(target, [Feature::TextReplace])
        .expect("session should start");
    let bound_feature = BoundFeature::new(adapter_id, version, Feature::TextReplace);

    let updated = manager
        .update(status.session_id(), Generation::new(2))
        .expect("old ACK should remain a visible session state");

    assert_eq!(
        updated.generation(&bound_feature),
        Some(GenerationPhase::Mismatch {
            desired: Generation::new(2),
            acknowledged: Generation::new(1),
            applied: None,
        })
    );
}

#[test]
fn ses_007_applies_a_new_target_runtime_generation_atomically() {
    let (package, requirement, adapter_id) = adapter_fixture(
        "example.synthetic.atomic-runtime",
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace, Feature::FontSubstitute],
        0x95,
    );
    let version = AdapterVersion::new(1, 0, 0);
    let policy = AdapterTrustPolicy::new([SignerId::new(TRUSTED_SIGNER)], [adapter_id.clone()]);
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("package should load");
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
    };
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        UpdateReportingHost {
            reports: VecDeque::from([HostGenerationReport::target_runtime(Generation::new(2))]),
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-8"),
        TargetFacts::new("windows", "x86_64"),
    );
    let status = manager
        .start(target, [Feature::TextReplace, Feature::FontSubstitute])
        .expect("session should start");
    let text_feature = BoundFeature::new(adapter_id.clone(), version, Feature::TextReplace);
    let font_feature = BoundFeature::new(adapter_id, version, Feature::FontSubstitute);

    let updated = manager
        .update(status.session_id(), Generation::new(2))
        .expect("new generation ACK should apply");

    assert_eq!(
        updated.generation(&text_feature),
        Some(GenerationPhase::Applied(Generation::new(2)))
    );
    assert_eq!(
        updated.generation(&font_feature),
        Some(GenerationPhase::Applied(Generation::new(2)))
    );
}

#[test]
fn ses_008_advances_a_worker_feature_after_applied_generation_ack() {
    let generation = Generation::new(2);
    let bound_feature = worker_bound_feature();
    let mut manager = isolated_session([HostGenerationReport::isolated([(
        bound_feature.clone(),
        generation,
    )])]);
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-9"),
        TargetFacts::new("windows", "x86_64"),
    );
    let status = manager
        .start(target, [Feature::TextReplace])
        .expect("worker session should start");

    let updated = manager
        .update(status.session_id(), generation)
        .expect("applied Worker ACK should update status");

    assert_eq!(
        updated.generation(&bound_feature),
        Some(GenerationPhase::Applied(generation))
    );
}

#[test]
fn ses_009_keeps_worker_feature_updating_until_new_decision_is_applied() {
    let first_generation = Generation::new(1);
    let next_generation = Generation::new(2);
    let bound_feature = worker_bound_feature();
    let mut manager = isolated_session([
        HostGenerationReport::isolated([(bound_feature.clone(), first_generation)]),
        HostGenerationReport::pending(),
    ]);
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-10"),
        TargetFacts::new("windows", "x86_64"),
    );
    let status = manager
        .start(target, [Feature::TextReplace])
        .expect("worker session should start");
    manager
        .update(status.session_id(), first_generation)
        .expect("first Worker generation should apply");

    let updating = manager
        .update(status.session_id(), next_generation)
        .expect("unacknowledged Worker decision should remain pending");

    assert_eq!(
        updating.generation(&bound_feature),
        Some(GenerationPhase::Updating {
            desired: next_generation,
            applied: Some(first_generation),
        })
    );
}

#[test]
fn ses_010_keeps_established_adapter_state_when_controller_loss_policy_is_continue() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = HealthRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
        health: ControllerHealth::Lost,
    };
    let mut manager = SessionManager::new(registry, recipe_port, AckAllHost, RunningTargets);
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-11"),
        TargetFacts::new("windows", "x86_64"),
    );
    let started = manager
        .start(target, [Feature::TextReplace])
        .expect("session should become active before controller loss");
    let bound_feature = BoundFeature::new(adapter_id, version, Feature::TextReplace);

    let status = manager
        .status(started.session_id())
        .expect("controller loss must not delete the session");

    assert_eq!(status.phase(&bound_feature), Some(FeaturePhase::Active));
}

#[test]
fn ses_010_degrades_established_adapter_state_when_controller_loss_policy_requires_it() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = HealthRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Degrade),
        health: ControllerHealth::Lost,
    };
    let mut manager = SessionManager::new(registry, recipe_port, AckAllHost, RunningTargets);
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-12"),
        TargetFacts::new("windows", "x86_64"),
    );
    let started = manager
        .start(target, [Feature::TextReplace])
        .expect("session should become active before controller loss");
    let bound_feature = BoundFeature::new(adapter_id, version, Feature::TextReplace);

    let status = manager
        .status(started.session_id())
        .expect("degraded session must remain observable");

    assert_eq!(status.phase(&bound_feature), Some(FeaturePhase::Degraded));
}

#[test]
fn ses_011_degrades_only_features_owned_by_a_failed_adapter() {
    let (failed_package, failed_requirement, failed_id) = adapter_fixture(
        "example.synthetic.failed-adapter",
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
        0x97,
    );
    let (healthy_package, healthy_requirement, healthy_id) = adapter_fixture(
        "example.synthetic.healthy-adapter",
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::FontSubstitute],
        0x98,
    );
    let version = AdapterVersion::new(1, 0, 0);
    let policy = AdapterTrustPolicy::new(
        [SignerId::new(TRUSTED_SIGNER)],
        [failed_id.clone(), healthy_id.clone()],
    );
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([failed_package, healthy_package]))
        .expect("both packages should load");
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new(
            [failed_requirement, healthy_requirement],
            ControllerLossPolicy::Continue,
        ),
    };
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        AdapterHealthHost {
            failed_adapters: vec![BoundAdapter::new(failed_id.clone(), version)],
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-13"),
        TargetFacts::new("windows", "x86_64"),
    );
    let started = manager
        .start(target, [Feature::TextReplace, Feature::FontSubstitute])
        .expect("both adapters should become active before one fails");
    let failed_feature = BoundFeature::new(failed_id, version, Feature::TextReplace);
    let healthy_feature = BoundFeature::new(healthy_id, version, Feature::FontSubstitute);

    let status = manager
        .status(started.session_id())
        .expect("one adapter failure must not delete the session");

    assert_eq!(status.phase(&failed_feature), Some(FeaturePhase::Degraded));
    assert_eq!(status.phase(&healthy_feature), Some(FeaturePhase::Active));
}

#[test]
fn ses_012_stops_an_inline_target_runtime_by_entering_pass_mode() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let bound_adapter = BoundAdapter::new(adapter_id.clone(), version);
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
    };
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        DeactivationHost {
            expected: vec![AdapterDeactivation::new(
                bound_adapter.clone(),
                DeactivationMode::PassThrough,
            )],
            completed: vec![bound_adapter],
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-14"),
        TargetFacts::new("windows", "x86_64"),
    );
    let started = manager
        .start(target, [Feature::TextReplace])
        .expect("inline session should start");
    let bound_feature = BoundFeature::new(adapter_id, version, Feature::TextReplace);

    let stopped = manager
        .stop(started.session_id())
        .expect("host should confirm pass-through mode");

    assert_eq!(stopped.phase(&bound_feature), Some(FeaturePhase::Ready));
}

#[test]
fn ses_013_stops_retained_and_external_writeback_by_their_apply_model_contracts() {
    let (retained_package, retained_requirement, retained_id) = adapter_fixture(
        "example.synthetic.retained-stop",
        ApplyModel::RetainedObject,
        Placement::TargetProcess,
        [Feature::TextReplace],
        0x99,
    );
    let (external_package, external_requirement, external_id) = adapter_fixture(
        "example.synthetic.external-stop",
        ApplyModel::ExternalProtocol,
        Placement::IsolatedWorker,
        [Feature::FontSubstitute],
        0x9A,
    );
    let version = AdapterVersion::new(1, 0, 0);
    let retained_adapter = BoundAdapter::new(retained_id.clone(), version);
    let external_adapter = BoundAdapter::new(external_id.clone(), version);
    let policy = AdapterTrustPolicy::new(
        [SignerId::new(TRUSTED_SIGNER)],
        [retained_id.clone(), external_id.clone()],
    );
    let mut registry = AdapterRegistry::new(policy);
    registry
        .reload(AdapterPackageSet::new([retained_package, external_package]))
        .expect("both packages should load");
    let recipe_port = StaticRecipePort {
        recipe: SessionRecipe::new(
            [retained_requirement, external_requirement],
            ControllerLossPolicy::Continue,
        ),
    };
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        DeactivationHost {
            expected: vec![
                AdapterDeactivation::new(
                    retained_adapter.clone(),
                    DeactivationMode::RestoreOriginal,
                ),
                AdapterDeactivation::new(external_adapter.clone(), DeactivationMode::StopWriteback),
            ],
            completed: vec![retained_adapter, external_adapter],
        },
        RunningTargets,
    );
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-15"),
        TargetFacts::new("windows", "x86_64"),
    );
    let started = manager
        .start(target, [Feature::TextReplace, Feature::FontSubstitute])
        .expect("both writeback models should start");
    let retained_feature = BoundFeature::new(retained_id, version, Feature::TextReplace);
    let external_feature = BoundFeature::new(external_id, version, Feature::FontSubstitute);

    let stopped = manager
        .stop(started.session_id())
        .expect("both model-specific deactivations should complete");

    assert_eq!(stopped.phase(&retained_feature), Some(FeaturePhase::Ready));
    assert_eq!(stopped.phase(&external_feature), Some(FeaturePhase::Ready));
}

#[test]
fn ses_014_releases_only_the_session_whose_target_instance_exited() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = QueueRecipePort {
        recipes: VecDeque::from([
            SessionRecipe::new([requirement.clone()], ControllerLossPolicy::Continue),
            SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
        ]),
    };
    let exited_target_id = TargetInstanceId::new("target-instance-16");
    let surviving_target_id = TargetInstanceId::new("target-instance-17");
    let released_sessions = Arc::new(Mutex::new(Vec::new()));
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        ReleaseRecordingHost {
            released_sessions: Arc::clone(&released_sessions),
        },
        OneExitedTarget {
            exited_target: exited_target_id.clone(),
        },
    );
    let exited = manager
        .start(
            TargetInstance::new(exited_target_id, TargetFacts::new("windows", "x86_64")),
            [Feature::TextReplace],
        )
        .expect("first target session should start before exit is observed");
    let surviving = manager
        .start(
            TargetInstance::new(surviving_target_id, TargetFacts::new("windows", "x86_64")),
            [Feature::TextReplace],
        )
        .expect("second target session should start");

    assert_eq!(
        manager.status(exited.session_id()),
        Err(SessionError::TargetExited(exited.session_id()))
    );
    assert_eq!(
        released_sessions
            .lock()
            .expect("release log lock should remain available")
            .as_slice(),
        &[exited.session_id()]
    );

    let surviving_status = manager
        .status(surviving.session_id())
        .expect("other target sessions must remain available");
    assert_eq!(
        surviving_status.phase(&BoundFeature::new(
            adapter_id,
            version,
            Feature::TextReplace,
        )),
        Some(FeaturePhase::Active)
    );
}

#[test]
fn ses_015_isolates_state_generation_and_diagnostics_between_target_instances() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let recipe_port = QueueRecipePort {
        recipes: VecDeque::from([
            SessionRecipe::new([requirement.clone()], ControllerLossPolicy::Continue),
            SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
        ]),
    };
    let bound_adapter = BoundAdapter::new(adapter_id.clone(), version);
    let bound_feature = BoundFeature::new(adapter_id, version, Feature::TextReplace);
    let mut manager = SessionManager::new(
        registry,
        recipe_port,
        PerSessionReportingHost {
            failed_adapter: bound_adapter,
        },
        RunningTargets,
    );
    let first = manager
        .start(
            TargetInstance::new(
                TargetInstanceId::new("target-instance-18"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace],
        )
        .expect("first instance should start");
    let second = manager
        .start(
            TargetInstance::new(
                TargetInstanceId::new("target-instance-19"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace],
        )
        .expect("second instance should start");
    manager
        .update(first.session_id(), Generation::new(2))
        .expect("only the first instance should apply the new generation");

    let first_status = manager
        .status(first.session_id())
        .expect("first instance should remain observable as degraded");
    let second_status = manager
        .status(second.session_id())
        .expect("second instance should remain active");

    assert_eq!(
        first_status.phase(&bound_feature),
        Some(FeaturePhase::Degraded)
    );
    assert_eq!(
        second_status.phase(&bound_feature),
        Some(FeaturePhase::Active)
    );
    assert_eq!(
        first_status.generation(&bound_feature),
        Some(GenerationPhase::Applied(Generation::new(2)))
    );
    assert_eq!(
        second_status.generation(&bound_feature),
        Some(GenerationPhase::Unreported)
    );
    assert_eq!(
        first_status.diagnostics(),
        &[SessionDiagnostic::new("first-target-host-failure")]
    );
    assert_eq!(
        second_status.diagnostics(),
        &[SessionDiagnostic::new("second-target-healthy")]
    );
}

#[test]
fn adr_014_rejects_new_sessions_and_degrades_existing_sessions_after_adapter_removal() {
    let (registry, requirement, adapter_id, version) = registry_fixture();
    let mut registry_source = registry.clone();
    let recipe_port = QueueRecipePort {
        recipes: VecDeque::from([
            SessionRecipe::new([requirement.clone()], ControllerLossPolicy::Continue),
            SessionRecipe::new([requirement], ControllerLossPolicy::Continue),
        ]),
    };
    let mut manager = SessionManager::new(registry, recipe_port, AckAllHost, RunningTargets);
    let started = manager
        .start(
            TargetInstance::new(
                TargetInstanceId::new("target-instance-20"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace],
        )
        .expect("installed adapter should start the first session");
    let bound_feature = BoundFeature::new(adapter_id.clone(), version, Feature::TextReplace);

    registry_source
        .reload(AdapterPackageSet::new(std::iter::empty()))
        .expect("removing an adapter package should publish a new registry revision");

    let existing = manager
        .status(started.session_id())
        .expect("existing session should remain observable after removal");
    assert_eq!(existing.phase(&bound_feature), Some(FeaturePhase::Degraded));

    let rejection = manager
        .start(
            TargetInstance::new(
                TargetInstanceId::new("target-instance-21"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace],
        )
        .expect_err("new sessions must not resolve a removed adapter");
    assert_eq!(
        rejection,
        SessionError::FeaturesUnavailable(vec![BoundFeature::new(
            adapter_id,
            version,
            Feature::TextReplace,
        )])
    );
}
