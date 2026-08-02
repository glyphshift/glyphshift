use glyphshift_adapter_registry::{
    AdapterBinding, AdapterDescriptor, AdapterPackage, AdapterPackageSet, AdapterRegistry,
    AdapterRequirement, AdapterTrustPolicy, AdapterVersion, AdapterVersionRequirement,
    ArtifactHash, PackageArtifactId, SignerId,
};
use glyphshift_adapter_sdk::{ActivationGrant, DrawCommand, InlineTextInput};
use glyphshift_domain::{
    AbiVersion, AdapterId, ApplyModel, Feature, Generation, Placement, RouteProgram, TargetFacts,
    TextDecision, TextObservation,
};
use glyphshift_extension::{
    ExtensionId, ExtensionPackage, ExtensionPackageSet, ExtensionRegistry, ExtensionRequirement,
    ExtensionVersion, ExtensionVersionRequirement, SoftwareIdentity, TranslationLocation,
};
use glyphshift_reference_adapters::InlineReferenceAdapter;
use glyphshift_runtime_kernel::RuntimeKernel;
use glyphshift_service::{GlyphshiftService, ServiceError};
use glyphshift_session::{
    AdapterDeactivation, AdapterHostPort, BoundAdapter, BoundFeature, FeaturePhase,
    GenerationPhase, HostActivation, HostDeactivation, HostFailure, HostGenerationReport,
    HostHealthReport, SessionError, SessionId, TargetHealth, TargetInstance, TargetInstanceId,
    TargetLifecyclePort,
};
use glyphshift_translation::{
    FontPolicy, SourceDocument, SourceId, SourceLayer, SourceRevision, TranslationSnapshot,
    TranslationWorkspace, WorkspaceChange, WorkspaceEntry, WorkspaceLocation,
};
use std::sync::{Arc, Mutex};

const TEXT_ADAPTER: &str = "example.synthetic.text";
const FONT_ADAPTER: &str = "example.synthetic.font";

struct HeadlessState {
    kernel: Option<RuntimeKernel>,
    inline: InlineReferenceAdapter,
    desired_snapshot: TranslationSnapshot,
    desired_font: FontPolicy,
    failed_text_adapter: bool,
    stopped: bool,
}

impl HeadlessState {
    fn render(&mut self, source: &str, font: &str) -> DrawCommand {
        if self.stopped {
            return DrawCommand::new(source, font);
        }
        let mut decision = self
            .kernel
            .as_ref()
            .expect("active kernel")
            .decide(&TextObservation::new(TEXT_ADAPTER, source, "surface-main"));
        if self.failed_text_adapter {
            decision.text = TextDecision::Keep;
        }
        self.inline.invoke(
            InlineTextInput::utf16(source.encode_utf16(), font),
            |_| Ok(decision),
            |command| command,
        )
    }
}

struct HeadlessHost {
    state: Arc<Mutex<HeadlessState>>,
}

impl AdapterHostPort for HeadlessHost {
    fn activate(
        &mut self,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
    ) -> Result<HostActivation, HostFailure> {
        let mut state = self.state.lock().map_err(|_| HostFailure::Unavailable)?;
        state.kernel = Some(
            RuntimeKernel::activate(
                bindings.iter().cloned(),
                RouteProgram::direct("menu"),
                state.desired_snapshot.clone(),
                state.desired_font.clone(),
            )
            .map_err(|_| HostFailure::HandshakeRejected)?,
        );
        state.stopped = false;
        Ok(HostActivation::connected(bindings.iter().flat_map(
            |binding| {
                binding.features.iter().map(|feature| {
                    BoundFeature::new(binding.adapter_id.clone(), binding.version, *feature)
                })
            },
        )))
    }

    fn update(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
        generation: Generation,
    ) -> Result<HostGenerationReport, HostFailure> {
        let mut state = self.state.lock().map_err(|_| HostFailure::Unavailable)?;
        let snapshot = state.desired_snapshot.clone();
        let font = state.desired_font.clone();
        state
            .kernel
            .as_mut()
            .ok_or(HostFailure::Unavailable)?
            .update(snapshot, font)
            .map_err(|_| HostFailure::HandshakeRejected)?;
        Ok(HostGenerationReport::target_runtime(generation))
    }

    fn health(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<HostHealthReport, HostFailure> {
        let state = self.state.lock().map_err(|_| HostFailure::Unavailable)?;
        Ok(if state.failed_text_adapter {
            HostHealthReport::failed([BoundAdapter::new(
                AdapterId::new(TEXT_ADAPTER),
                AdapterVersion::new(1, 0, 0),
            )])
        } else {
            HostHealthReport::healthy()
        })
    }

    fn deactivate(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        bindings: &[AdapterBinding],
        _plan: &[AdapterDeactivation],
    ) -> Result<HostDeactivation, HostFailure> {
        let mut state = self.state.lock().map_err(|_| HostFailure::Unavailable)?;
        state.stopped = true;
        Ok(HostDeactivation::completed(bindings.iter().map(
            |binding| BoundAdapter::new(binding.adapter_id.clone(), binding.version),
        )))
    }

    fn release(
        &mut self,
        _session_id: SessionId,
        _target: &TargetInstance,
        _bindings: &[AdapterBinding],
    ) -> Result<(), HostFailure> {
        Ok(())
    }
}

struct RunningTargets;

impl TargetLifecyclePort for RunningTargets {
    fn health(&mut self, _target: &TargetInstance) -> TargetHealth {
        TargetHealth::Running
    }
}

fn adapter_package(
    adapter_id: &str,
    feature: Feature,
    signer: &SignerId,
    hash_byte: u8,
) -> (AdapterPackage, AdapterRequirement) {
    let id = AdapterId::new(adapter_id);
    let version = AdapterVersion::new(1, 0, 0);
    let hash = ArtifactHash::sha256([hash_byte; 32]);
    (
        AdapterPackage::new(
            AdapterDescriptor::new(
                id.clone(),
                version,
                ApplyModel::InlineRender,
                Placement::TargetProcess,
                [feature],
            )
            .with_abi(AbiVersion::new(1, 0)),
            PackageArtifactId::new("runtime/adapter"),
            signer.clone(),
            hash,
            hash,
        ),
        AdapterRequirement::new(id, AdapterVersionRequirement::Exact(version), [feature]),
    )
}

#[test]
fn headless_e2e_covers_visible_update_unload_failure_isolation_and_stop() {
    let mut workspace = TranslationWorkspace::new(
        "org.example.editor",
        "zh-CN",
        [WorkspaceLocation::new("menu", "Menu")],
    );
    workspace
        .rescan(SourceDocument::new(
            SourceId::new("builtin"),
            SourceLayer::Builtin,
            [WorkspaceEntry::new("menu", "Open", "打开")],
        ))
        .expect("initial catalog");
    let font_policy = FontPolicy::empty().with_location("menu", "Example CJK");
    let state = Arc::new(Mutex::new(HeadlessState {
        kernel: None,
        inline: InlineReferenceAdapter::activate(
            [Feature::TextReplace, Feature::FontSubstitute],
            &ActivationGrant::new([Feature::TextReplace, Feature::FontSubstitute]),
        )
        .expect("reference inline"),
        desired_snapshot: workspace.snapshot().clone(),
        desired_font: font_policy,
        failed_text_adapter: false,
        stopped: false,
    }));

    let signer = SignerId::new("example.signer.trusted");
    let (text_package, text_requirement) =
        adapter_package(TEXT_ADAPTER, Feature::TextReplace, &signer, 0x31);
    let (font_package, font_requirement) =
        adapter_package(FONT_ADAPTER, Feature::FontSubstitute, &signer, 0x32);
    let adapters = AdapterRegistry::new(AdapterTrustPolicy::new(
        [signer],
        [AdapterId::new(TEXT_ADAPTER), AdapterId::new(FONT_ADAPTER)],
    ));
    let extension_id = ExtensionId::new("org.example.editor");
    let extension_version = ExtensionVersion::new(1, 0, 0);
    let extension = ExtensionPackage::software(
        extension_id.clone(),
        extension_version,
        SoftwareIdentity::new("Example Editor", "Example Vendor", ["ExampleEditor.exe"]),
        [text_requirement, font_requirement],
        [TranslationLocation::without_context("menu", "Menu")],
    )
    .with_route_program(RouteProgram::direct("menu"));
    let extension_requirement = ExtensionRequirement::new(
        extension_id.clone(),
        ExtensionVersionRequirement::Exact(extension_version),
    );
    let mut service = GlyphshiftService::new(
        ExtensionRegistry::new(),
        adapters,
        HeadlessHost {
            state: Arc::clone(&state),
        },
        RunningTargets,
    );
    service
        .reload_adapters(AdapterPackageSet::new([text_package, font_package.clone()]))
        .expect("dynamic adapters");
    service
        .reload_extensions(ExtensionPackageSet::new([extension.clone()]))
        .expect("dynamic extension");

    let status = service
        .start_extension_session(
            &extension_requirement,
            TargetInstance::new(
                TargetInstanceId::new("target-instance-a"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace, Feature::FontSubstitute],
        )
        .expect("active session");
    let session_id = status.session_id();
    let text_feature = BoundFeature::new(
        AdapterId::new(TEXT_ADAPTER),
        AdapterVersion::new(1, 0, 0),
        Feature::TextReplace,
    );
    let font_feature = BoundFeature::new(
        AdapterId::new(FONT_ADAPTER),
        AdapterVersion::new(1, 0, 0),
        Feature::FontSubstitute,
    );
    assert_eq!(status.phase(&text_feature), Some(FeaturePhase::Active));
    assert_eq!(status.phase(&font_feature), Some(FeaturePhase::Active));
    let rendered = state.lock().expect("state").render("Open", "Example Sans");
    assert_eq!(rendered.text(), "打开");
    assert_eq!(rendered.font(), "Example CJK");

    workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Open", "开启")),
            SourceRevision::new(1),
        )
        .expect("workspace hot update");
    state.lock().expect("state").desired_snapshot = workspace.snapshot().clone();
    let updated = service
        .update(session_id, Generation::new(2))
        .expect("runtime generation ACK");
    assert_eq!(
        updated.generation(&text_feature),
        Some(GenerationPhase::Applied(Generation::new(2)))
    );
    assert_eq!(
        state.lock().expect("state").render("Open", "Example Sans"),
        DrawCommand::new("开启", "Example CJK")
    );

    service
        .reload_extensions(ExtensionPackageSet::new([]))
        .expect("remove software extension");
    assert_eq!(
        service
            .status(session_id)
            .expect("existing session")
            .phase(&font_feature),
        Some(FeaturePhase::Active)
    );
    assert!(matches!(
        service.start_extension_session(
            &extension_requirement,
            TargetInstance::new(
                TargetInstanceId::new("target-instance-b"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace, Feature::FontSubstitute],
        ),
        Err(ServiceError::Extension(_))
    ));

    state.lock().expect("state").failed_text_adapter = true;
    let degraded = service
        .status(session_id)
        .expect("isolated adapter failure");
    assert_eq!(degraded.phase(&text_feature), Some(FeaturePhase::Degraded));
    assert_eq!(degraded.phase(&font_feature), Some(FeaturePhase::Active));
    assert_eq!(
        state.lock().expect("state").render("Open", "Example Sans"),
        DrawCommand::new("Open", "Example CJK")
    );

    state.lock().expect("state").failed_text_adapter = false;
    service
        .reload_adapters(AdapterPackageSet::new([font_package]))
        .expect("remove text adapter");
    let removed = service.status(session_id).expect("removed adapter status");
    assert_eq!(removed.phase(&text_feature), Some(FeaturePhase::Degraded));
    assert_eq!(removed.phase(&font_feature), Some(FeaturePhase::Active));
    service
        .reload_extensions(ExtensionPackageSet::new([extension]))
        .expect("restore extension metadata");
    assert!(matches!(
        service.start_extension_session(
            &extension_requirement,
            TargetInstance::new(
                TargetInstanceId::new("target-instance-c"),
                TargetFacts::new("windows", "x86_64"),
            ),
            [Feature::TextReplace, Feature::FontSubstitute],
        ),
        Err(ServiceError::Session(SessionError::FeaturesUnavailable(_)))
    ));

    let stopped = service.stop(session_id).expect("safe stop");
    assert_eq!(stopped.phase(&text_feature), Some(FeaturePhase::Ready));
    assert_eq!(stopped.phase(&font_feature), Some(FeaturePhase::Ready));
    assert_eq!(
        state.lock().expect("state").render("Open", "Example Sans"),
        DrawCommand::new("Open", "Example Sans")
    );
}
