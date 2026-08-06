use super::*;

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
