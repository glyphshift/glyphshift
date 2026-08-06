use super::*;

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
