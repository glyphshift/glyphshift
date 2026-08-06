use super::*;

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
