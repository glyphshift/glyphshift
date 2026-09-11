use glyphshift_adapter_registry::{
    AdapterBinding, AdapterDescriptor, AdapterHostBinding, AdapterPackage, AdapterPackageSet,
    AdapterRegistry, AdapterRequirement, AdapterTrustPolicy, AdapterVersion,
    AdapterVersionRequirement, ArtifactHash, PackageArtifactId, SignerId,
};
use glyphshift_decision::{DecisionTraceStatus, FontTrace, TextTrace};
use glyphshift_domain::{
    AbiVersion, AdapterId, ApplyModel, Feature, FontDecision, Generation, Placement,
    RenderDecision, RouteProgram, TargetFacts, TextDecision, TextObservation,
};
use glyphshift_runtime_contract::RuntimePublication;
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

#[test]
fn rtk_004_keeps_decision_tracing_disabled_by_default_and_bounded_when_enabled() {
    let publication = RuntimePublication::new(
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(4)).with_entry("menu", "Open", "打开"),
        FontPolicy::empty().with_location("menu", "Example Sans CJK"),
    );
    let publication_identity = publication.identity().expect("valid publication identity");
    let kernel = RuntimeKernel::from_publication(std::iter::empty(), publication)
        .expect("decision-only runtime fixture should activate");
    let observation = TextObservation::new("adapter-a", "Open", "surface-main");

    let decision_without_trace = kernel.decide(&observation);
    assert!(kernel.drain_decision_traces().records().is_empty());

    kernel.set_decision_tracing(true);
    let decision_with_trace = kernel.decide(&observation);
    let batch = kernel.drain_decision_traces();
    assert_eq!(decision_with_trace, decision_without_trace);
    assert_eq!(batch.dropped(), 0);
    assert_eq!(batch.records().len(), 1);
    let record = &batch.records()[0];
    assert_eq!(record.adapter_id(), "adapter-a");
    assert_eq!(record.source_text(), "Open");
    assert_eq!(record.decision(), &decision_with_trace);
    assert_eq!(record.publication_identity(), publication_identity);
    assert_eq!(record.trace().status(), DecisionTraceStatus::Matched);
    assert_eq!(record.trace().text(), TextTrace::Replaced);
    assert_eq!(record.trace().font(), FontTrace::Substituted);

    for _ in 0..257 {
        let _ = kernel.decide(&TextObservation::new(
            "adapter-a",
            "Unmatched",
            "surface-main",
        ));
    }
    let bounded = kernel.drain_decision_traces();
    assert_eq!(bounded.records().len(), 256);
    assert_eq!(bounded.dropped(), 1);

    kernel.set_decision_tracing(false);
    let _ = kernel.decide(&observation);
    assert!(kernel.drain_decision_traces().records().is_empty());
}

#[test]
fn rtk_005_rejects_a_stale_publication_without_partially_switching_its_route() {
    let mut kernel = RuntimeKernel::from_publication(
        std::iter::empty(),
        RuntimePublication::new(
            RouteProgram::direct("menu"),
            TranslationSnapshot::empty(Generation::new(2)).with_entry("menu", "Open", "当前"),
            FontPolicy::empty(),
        ),
    )
    .expect("current publication should activate");
    let stale = RuntimePublication::new(
        RouteProgram::direct("panel"),
        TranslationSnapshot::empty(Generation::new(1)).with_entry("panel", "Open", "陈旧"),
        FontPolicy::empty(),
    );

    assert_eq!(
        kernel.apply_publication(stale),
        Err(RuntimeKernelError::StaleGeneration {
            current: Generation::new(2),
            incoming: Generation::new(1),
        })
    );
    assert_eq!(
        kernel
            .decide(&TextObservation::new("adapter-a", "Open", "surface-main"))
            .text,
        TextDecision::Replace("当前".into())
    );
}

#[test]
fn regex_publications_apply_next_generation_and_restore_on_removal() {
    use glyphshift_translation::{RegexTranslationRule, RegexTranslationRules};
    let rules = RegexTranslationRules::compile(vec![RegexTranslationRule { pattern: r"^(.+?)(:[0-9]+)$".into(), replacement: "{{TR}}$2".into(), enabled: true }]).unwrap();
    let snap = |generation, text: &str| TranslationSnapshot::empty(Generation::new(generation)).with_entry("text", "Total", text);
    let mut kernel = RuntimeKernel::activate(std::iter::empty(), RouteProgram::direct("text"), snap(1, "总计").with_dictionary_rules("text", rules.clone()), FontPolicy::empty()).unwrap();
    let observation = TextObservation::new("example.synthetic.writeback", "Total:33", "surface");
    assert_eq!(kernel.decide(&observation).text, TextDecision::Replace("总计:33".into()));
    kernel.update(snap(2, "合计").with_dictionary_rules("text", rules), FontPolicy::empty()).unwrap();
    assert_eq!(kernel.decide(&observation).text, TextDecision::Replace("合计:33".into()));
    kernel.update(snap(3, "合计"), FontPolicy::empty()).unwrap();
    assert_eq!(kernel.decide(&observation).text, TextDecision::Keep);
}
