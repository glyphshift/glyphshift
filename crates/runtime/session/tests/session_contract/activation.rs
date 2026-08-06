use super::*;

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
