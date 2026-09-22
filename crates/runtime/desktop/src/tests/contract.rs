use super::*;
use glyphshift_capture::{CaptureObservationBatch, CaptureProducerId, CaptureSessionId};
use glyphshift_protocol::{
    ControllerRecipe, ControllerRuntimeAck, RecipeControllerLossPolicy, RecipeDirective,
};

struct PartialFamilyController {
    requirement: AdapterRequirement,
    publication_identity: [u8; 32],
    rejected_first: bool,
}

impl ControllerTransport for PartialFamilyController {
    fn handshake(
        &mut self,
        expected_extension: &ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(ControllerHello::new(
            expected_extension.clone(),
            version,
            nonce,
        ))
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        let healthy = ControllerTarget::new(
            ControllerTargetToken::new("healthy-target"),
            "Synthetic healthy target",
            TargetFacts::new("windows", "x86_64"),
        );
        let rejected = ControllerTarget::new(
            ControllerTargetToken::new("rejected-target"),
            "Synthetic rejected target",
            TargetFacts::new("windows", "x86_64"),
        );
        let targets = if self.rejected_first {
            vec![rejected, healthy]
        } else {
            vec![healthy, rejected]
        };
        Ok(ControllerInventory::new([], targets))
    }

    fn prepare(
        &mut self,
        _target: &ControllerTargetToken,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<ControllerRecipe, TransportFailure> {
        Ok(ControllerRecipe::new(
            [RecipeDirective::adapter(self.requirement.clone())],
            RecipeControllerLossPolicy::Continue,
        ))
    }

    fn activate_runtime(
        &mut self,
        target: &ControllerTargetToken,
        deployment: &glyphshift_protocol::ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        if target.as_str() == "rejected-target" {
            return Err(TransportFailure::Rejected(
                glyphshift_protocol::ControllerRejection::TargetProcessUnavailable,
            ));
        }
        Ok(ControllerRuntimeAck::new(
            deployment.generation(),
            self.publication_identity,
        ))
    }

    fn control_capture(
        &mut self,
        _target: &ControllerTargetToken,
        _paused: bool,
    ) -> Result<(), TransportFailure> {
        Ok(())
    }

    fn query_observations(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<CaptureObservationBatch, TransportFailure> {
        if target.as_str() != "healthy-target" {
            return Err(TransportFailure::MalformedMessage);
        }
        let producer_id = if self.rejected_first {
            "target-2"
        } else {
            "target-1"
        };
        CaptureObservationBatch::new(
            CaptureProducerId::new(producer_id).map_err(|_| TransportFailure::MalformedMessage)?,
            1,
            0,
            [],
        )
        .map_err(|_| TransportFailure::MalformedMessage)
    }

    fn deactivate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        Ok(())
    }

    fn terminate(&mut self) {}
}

fn partial_family_runtime(
    root: &Path,
    rejected_first: bool,
) -> DesktopRuntime<PartialFamilyController> {
    let runtime_library = root.join("runtime.dll");
    let adapter_library = root.join("adapter.dll");
    fs::write(&runtime_library, b"synthetic runtime").expect("runtime artifact");
    fs::write(&adapter_library, b"synthetic adapter").expect("adapter artifact");

    let adapter_id = AdapterId::new("example.synthetic.observe");
    let version = AdapterVersion::new(1, 0, 0);
    let requirement = AdapterRequirement::new(
        adapter_id.clone(),
        AdapterVersionRequirement::Exact(version),
        [Feature::TextObserve],
    );
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        version,
        ApplyModel::ObserveOnly,
        Placement::TargetProcess,
        [Feature::TextObserve],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86_64"]);
    let hash = ArtifactHash::sha256([0xA5; 32]);
    let artifact_id = PackageArtifactId::new("adapters/synthetic");
    let signer = SignerId::new("example.synthetic.signer");
    let mut registry =
        AdapterRegistry::new(AdapterTrustPolicy::new([signer.clone()], [adapter_id]));
    registry
        .reload(AdapterPackageSet::new([AdapterPackage::new(
            descriptor,
            artifact_id.clone(),
            signer,
            hash,
            hash,
        )]))
        .expect("load synthetic observer");
    let artifacts = TargetArtifactCatalog::new(
        RuntimeArtifact::new(runtime_library, [0; 32]),
        [(artifact_id, adapter_library)],
    )
    .expect("target artifacts");
    let executable = root.join("PartialFamily.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.join("data"));
    let snapshot = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("register partial family software");
    let publication = backend
        .runtime_spec(snapshot.software()[0].id())
        .expect("partial family Runtime spec")
        .publication()
        .clone();
    let publication_identity = publication
        .identity()
        .expect("partial family publication identity")
        .as_bytes();
    DesktopRuntime::connect(
        PartialFamilyController {
            requirement: requirement.clone(),
            publication_identity,
            rejected_first,
        },
        "software.partial-family".into(),
        vec![requirement],
        publication,
        registry,
        artifacts,
        WorkerArtifactCatalog::default(),
        Box::new(AcquisitionWorkerCatalog::default()),
        BTreeSet::new(),
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([0x44; 32]),
        &mut NonceLedger::new(),
    )
    .expect("discover partial family")
}

#[test]
fn desktop_contract_exposes_instances_without_controller_tokens_or_paths() {
    let root = tempdir().expect("desktop data");
    let executable = root.path().join("MotionCanvas.exe");
    fs::write(&executable, b"synthetic executable").expect("selected executable");
    let mut backend = open_test_backend(root.path().join("data"));
    let snapshot = backend
        .add_software(ExecutableSelection::new(executable))
        .expect("registered executable");
    let application_id = snapshot.software()[0].id();
    let spec = backend
        .runtime_spec(application_id)
        .expect("generic runtime spec");

    let runtime_library = root.path().join("runtime.dll");
    fs::write(&runtime_library, b"synthetic runtime").expect("runtime artifact");
    let artifacts = TargetArtifactCatalog::new(RuntimeArtifact::new(&runtime_library, [0; 32]), [])
        .expect("artifact catalog");
    let mut ledger = NonceLedger::new();
    let runtime = DesktopRuntime::connect(
        InventoryController,
        application_id.into(),
        Vec::new(),
        spec.publication().clone(),
        AdapterRegistry::new(AdapterTrustPolicy::new([], [])),
        artifacts,
        WorkerArtifactCatalog::default(),
        Box::new(AcquisitionWorkerCatalog::default()),
        BTreeSet::new(),
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([7; 32]),
        &mut ledger,
    )
    .expect("desktop runtime discovery");

    assert_eq!(runtime.application_id(), application_id);
    assert_eq!(
        runtime
            .targets()
            .map(|target| (target.id(), target.display_name()))
            .collect::<Vec<_>>(),
        vec![(1, "MotionCanvas — 主窗口")]
    );
    assert!(!runtime.is_active());
    assert!(!runtime.supports(Feature::TextReplace));
}

#[test]
fn process_family_capture_keeps_a_healthy_target_when_another_target_rejects_activation() {
    let root = tempdir().expect("partial family Runtime data");
    let mut runtime = partial_family_runtime(root.path(), false);
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("partial-family").expect("capture id"),
        root.path().join("capture.json"),
        100,
    )
    .expect("capture configuration");

    runtime
        .start_capture([1, 2], [Feature::TextObserve], capture)
        .expect("healthy family member keeps capture active");

    assert_eq!(runtime.active_target_count(), 1);
    assert_eq!(runtime.failed_target_count(), 1);
    assert!(runtime.is_feature_active(Feature::TextObserve));
    runtime.stop().expect("stop partial family capture");
}

#[test]
fn translation_activation_falls_back_in_discovery_order_after_the_first_target_rejects() {
    let root = tempdir().expect("fallback family Runtime data");
    let mut runtime = partial_family_runtime(root.path(), true);

    runtime
        .start_first_available([1, 2], [Feature::TextObserve])
        .expect("second family member accepts translation activation");

    assert_eq!(runtime.active_target_id(), Some(2));
    assert_eq!(runtime.active_target_count(), 1);
    assert_eq!(runtime.failed_target_count(), 1);
    assert!(runtime.is_feature_active(Feature::TextObserve));
    runtime.stop().expect("stop fallback family Runtime");
}
