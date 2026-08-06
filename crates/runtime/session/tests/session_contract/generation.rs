use super::*;

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
