use glyphshift_adapter_registry::{
    AdapterBinding, AdapterDescriptor, AdapterHostBinding, AdapterPackage, AdapterPackageSet,
    AdapterRegistry, AdapterRequirement, AdapterTrustPolicy, AdapterVersion,
    AdapterVersionRequirement, ArtifactHash, PackageArtifactId, SignerId,
};
use glyphshift_domain::{
    AbiVersion, AdapterId, ApplyModel, Feature, FontDecision, Generation, Placement,
    RenderDecision, RouteProgram, TargetFacts, TextDecision, TextObservation,
};
use glyphshift_runtime_kernel::{RuntimeKernel, RuntimeKernelError};
use glyphshift_translation::{FontPolicy, FontRule, TranslationSnapshot};

#[test]
fn rtk_001_activates_a_runtime_discovered_adapter_without_an_id_branch() {
    let adapter_id = AdapterId::new("example.synthetic.runtime-writeback");
    let version = AdapterVersion::new(1, 0, 0);
    let hash = ArtifactHash::sha256([0x81; 32]);
    let package = AdapterPackage::new(
        AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::InlineRender,
            Placement::TargetProcess,
            [Feature::TextReplace, Feature::FontSubstitute],
        )
        .with_abi(AbiVersion::new(1, 0)),
        PackageArtifactId::new("runtime/adapter"),
        SignerId::new("example.signer.trusted"),
        hash,
        hash,
    );
    let requirement = AdapterRequirement::new(
        adapter_id,
        AdapterVersionRequirement::Exact(version),
        [Feature::TextReplace, Feature::FontSubstitute],
    );
    let mut registry = AdapterRegistry::new(AdapterTrustPolicy::new(
        [SignerId::new("example.signer.trusted")],
        [requirement.adapter_id().clone()],
    ));
    registry
        .reload(AdapterPackageSet::new([package]))
        .expect("runtime package should load");
    let binding = registry
        .resolve(&requirement, &TargetFacts::new("windows", "x86_64"))
        .expect("runtime-discovered adapter should bind");
    let kernel = RuntimeKernel::activate(
        [binding],
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(1)).with_entry("menu", "Open", "打开"),
        FontPolicy::empty().with_location("menu", "Example Sans CJK"),
    )
    .expect("verified target-process binding should activate");

    let decision = kernel.decide(&TextObservation::new(
        "example.synthetic.runtime-writeback",
        "Open",
        "surface-main",
    ));

    assert_eq!(
        decision,
        RenderDecision {
            text: TextDecision::Replace("打开".into()),
            font: FontDecision::Substitute("Example Sans CJK".into()),
            generation: Generation::new(1),
        }
    );
}

#[test]
fn rtk_002_rejects_an_isolated_worker_binding_in_the_target_process() {
    let adapter_id = AdapterId::new("example.synthetic.worker-only");
    let rejection = RuntimeKernel::activate(
        [AdapterBinding {
            descriptor: AdapterDescriptor::new(
                adapter_id.clone(),
                AdapterVersion::new(1, 0, 0),
                ApplyModel::ExternalProtocol,
                Placement::IsolatedWorker,
                [Feature::TextReplace],
            ),
            adapter_id: adapter_id.clone(),
            version: AdapterVersion::new(1, 0, 0),
            apply_model: ApplyModel::ExternalProtocol,
            artifact_hash: ArtifactHash::sha256([0; 32]),
            host: AdapterHostBinding::IsolatedWorker {
                executable: PackageArtifactId::new("worker/adapter"),
            },
            features: vec![Feature::TextReplace],
        }],
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(1)),
        FontPolicy::empty(),
    )
    .expect_err("isolated workers must remain outside the target Runtime");

    assert_eq!(
        rejection,
        RuntimeKernelError::UnsupportedBinding(adapter_id)
    );
}

#[test]
fn rtk_003_atomically_switches_snapshot_and_font_policy_by_generation() {
    let mut kernel = RuntimeKernel::activate(
        std::iter::empty(),
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(1)).with_entry("menu", "Open", "Old"),
        FontPolicy::empty()
            .with_location("menu", "Old Default")
            .with_entry("menu", "Open", FontRule::Unchanged),
    )
    .expect("decision-only runtime fixture should activate");
    let observation = TextObservation::new("example.synthetic.writeback", "Open", "surface-main");

    assert_eq!(
        kernel.decide(&observation),
        RenderDecision {
            text: TextDecision::Replace("Old".into()),
            font: FontDecision::Keep,
            generation: Generation::new(1),
        }
    );

    let acknowledged = kernel
        .update(
            TranslationSnapshot::empty(Generation::new(2)).with_entry("menu", "Open", "New"),
            FontPolicy::empty()
                .with_location("menu", "New Default")
                .with_entry("menu", "Open", FontRule::Substitute("New Override".into())),
        )
        .expect("a newer immutable snapshot should apply atomically");

    assert_eq!(acknowledged, Generation::new(2));
    assert_eq!(
        kernel.decide(&observation),
        RenderDecision {
            text: TextDecision::Replace("New".into()),
            font: FontDecision::Substitute("New Override".into()),
            generation: Generation::new(2),
        }
    );
    assert_eq!(
        kernel.update(
            TranslationSnapshot::empty(Generation::new(1)),
            FontPolicy::empty(),
        ),
        Err(RuntimeKernelError::StaleGeneration {
            current: Generation::new(2),
            incoming: Generation::new(1),
        })
    );
    assert_eq!(
        kernel.decide(&observation),
        RenderDecision {
            text: TextDecision::Replace("New".into()),
            font: FontDecision::Substitute("New Override".into()),
            generation: Generation::new(2),
        }
    );
}
