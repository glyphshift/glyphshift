use super::*;

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
