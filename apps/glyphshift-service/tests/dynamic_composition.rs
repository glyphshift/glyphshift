use glyphshift_adapter_registry::{
    AdapterBinding, AdapterDescriptor, AdapterPackage, AdapterPackageSet, AdapterRegistry,
    AdapterRequirement, AdapterTrustPolicy, AdapterVersion, AdapterVersionRequirement,
    ArtifactHash, PackageArtifactId, SignerId,
};
use glyphshift_domain::{
    AbiVersion, AdapterId, ApplyModel, Feature, FontDecision, Generation, Placement,
    RenderDecision, RouteProgram, TargetFacts, TextDecision, TextObservation,
};
use glyphshift_extension::{
    ExtensionId, ExtensionPackage, ExtensionPackageSet, ExtensionRegistry, ExtensionRequirement,
    ExtensionVersion, ExtensionVersionRequirement, SoftwareIdentity, TranslationLocation,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_runtime_kernel::RuntimeKernel;
use glyphshift_service::GlyphshiftService;
use glyphshift_session::{
    AdapterHostPort, BoundFeature, FeaturePhase, HostActivation, HostFailure, SessionId,
    TargetHealth, TargetInstance, TargetInstanceId, TargetLifecyclePort,
};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::sync::{Arc, Mutex};

struct KernelHost {
    kernel: Arc<Mutex<Option<RuntimeKernel>>>,
}

impl AdapterHostPort for KernelHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        panic!("the production service path must publish runtime decision inputs")
    }

    fn activate_runtime(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
        publication: &RuntimePublication,
    ) -> Result<HostActivation, HostFailure> {
        let runtime =
            RuntimeKernel::from_publication(bindings.iter().cloned(), publication.clone())
                .expect("verified target-process bindings should activate in the Runtime");
        *self
            .kernel
            .lock()
            .expect("runtime store lock should remain available") = Some(runtime);
        Ok(HostActivation::connected(bindings.iter().flat_map(
            |binding| {
                binding.features.iter().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
                })
            },
        )))
    }
}

struct RunningTargets;

impl TargetLifecyclePort for RunningTargets {
    fn health(&mut self, _target: &TargetInstance) -> TargetHealth {
        TargetHealth::Running
    }
}

#[test]
fn service_composes_runtime_added_unknown_software_and_adapter_without_id_branches() {
    let adapter_id = AdapterId::new("example.synthetic.dynamic-writeback");
    let adapter_version = AdapterVersion::new(1, 0, 0);
    let signer = SignerId::new("example.signer.trusted");
    let hash = ArtifactHash::sha256([0x91; 32]);
    let adapter_package = AdapterPackage::new(
        AdapterDescriptor::new(
            adapter_id.clone(),
            adapter_version,
            ApplyModel::InlineRender,
            Placement::TargetProcess,
            [Feature::TextReplace, Feature::FontSubstitute],
        )
        .with_abi(AbiVersion::new(1, 0)),
        PackageArtifactId::new("runtime/adapter"),
        signer.clone(),
        hash,
        hash,
    );
    let adapter_requirement = AdapterRequirement::new(
        adapter_id.clone(),
        AdapterVersionRequirement::Exact(adapter_version),
        [Feature::TextReplace, Feature::FontSubstitute],
    );
    let adapter_registry =
        AdapterRegistry::new(AdapterTrustPolicy::new([signer], [adapter_id.clone()]));
    let extension_id = ExtensionId::new("org.example.dynamic-editor");
    let extension_version = ExtensionVersion::new(1, 0, 0);
    let extension_package = ExtensionPackage::software(
        extension_id.clone(),
        extension_version,
        SoftwareIdentity::new("Dynamic Editor", "Example Vendor", ["DynamicEditor.exe"]),
        [adapter_requirement],
        [TranslationLocation::without_context("menu", "Menu")],
    );
    let kernel = Arc::new(Mutex::new(None));
    let mut service = GlyphshiftService::new(
        ExtensionRegistry::new(),
        adapter_registry,
        KernelHost {
            kernel: Arc::clone(&kernel),
        },
        RunningTargets,
    );

    service
        .reload_adapters(AdapterPackageSet::new([adapter_package]))
        .expect("runtime-added Adapter should publish");
    service
        .reload_extensions(ExtensionPackageSet::new([extension_package]))
        .expect("runtime-added software should publish");
    let status = service
        .start_extension_session_with_runtime(
            &ExtensionRequirement::new(
                extension_id,
                ExtensionVersionRequirement::Exact(extension_version),
            ),
            TargetInstance::new(
                TargetInstanceId::new("target-instance-dynamic"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace, Feature::FontSubstitute],
            &RuntimePublication::new(
                RouteProgram::direct("menu"),
                TranslationSnapshot::empty(Generation::new(1)).with_entry("menu", "Open", "打开"),
                FontPolicy::empty().with_location("menu", "Example Sans CJK"),
            ),
        )
        .expect("dynamic packages should compose into an active Session");

    assert_eq!(
        status.phase(&BoundFeature::new(
            adapter_id.clone(),
            adapter_version,
            Feature::TextReplace,
        )),
        Some(FeaturePhase::Active)
    );
    assert_eq!(
        status.phase(&BoundFeature::new(
            adapter_id,
            adapter_version,
            Feature::FontSubstitute,
        )),
        Some(FeaturePhase::Active)
    );
    let decision = kernel
        .lock()
        .expect("runtime store lock should remain available")
        .as_ref()
        .expect("Runtime Kernel should be active")
        .decide(&TextObservation::new(
            "example.synthetic.dynamic-writeback",
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

    let _session_id: SessionId = status.session_id();
}
