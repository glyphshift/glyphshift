use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_capture::{
    CaptureCatalog, CaptureConfiguration, CaptureObservationBatch, CaptureObservationRecord,
    CaptureProducerId, CaptureSessionId, FileCaptureSink,
};
use glyphshift_domain::{
    AdapterId, ApplyModel, Feature, Generation, Placement, RouteProgram, TargetFacts,
};
use glyphshift_extension::{ExtensionId, ProtocolVersion};
use glyphshift_protocol::{
    ControllerConnection, ControllerHello, ControllerInventory, ControllerNonce,
    ControllerRuntimeAck, ControllerRuntimeDeployment, ControllerRuntimeFontOutcome,
    ControllerRuntimeTextOutcome, ControllerRuntimeTraceBatch, ControllerRuntimeTraceRecord,
    ControllerRuntimeTraceStatus, ControllerTarget, ControllerTargetToken, ControllerTransport,
    NonceLedger, TransportFailure,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterHostPort, BoundAdapter, BoundFeature, HostActivation, HostDeactivation,
    HostGenerationReport, RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceStatus, SessionId,
    TargetInstance, TargetInstanceId,
};
use glyphshift_target_process_host::{RuntimeArtifact, TargetArtifactCatalog, TargetProcessHost};
use glyphshift_target_runtime_contract::TargetRuntimeDeployment;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct ContractTransport {
    activation_ack: u64,
    wrong_publication_identity: bool,
    active_adapter_ids: Option<Vec<AdapterId>>,
}

impl ControllerTransport for ContractTransport {
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
        Ok(ControllerInventory::new(
            [],
            [ControllerTarget::new(
                ControllerTargetToken::new("private-target"),
                "Synthetic Host · 运行中",
                TargetFacts::new("windows", "x86_64"),
            )],
        ))
    }

    fn activate_runtime(
        &mut self,
        target: &ControllerTargetToken,
        deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        if target.as_str() != "private-target"
            || deployment.runtime_library_sha256() != [0x61; 32]
            || deployment.generation() != 4
        {
            return Err(TransportFailure::MalformedMessage);
        }
        let decoded = TargetRuntimeDeployment::decode_json(deployment.deployment_json())
            .map_err(|_| TransportFailure::MalformedMessage)?;
        if decoded.publication().generation() != Generation::new(4)
            || decoded.adapters().is_empty()
            || decoded
                .adapters()
                .iter()
                .any(|adapter| adapter.binding().artifact_hash != ArtifactHash::sha256([0x62; 32]))
        {
            return Err(TransportFailure::MalformedMessage);
        }
        let mut identity = decoded
            .publication()
            .identity()
            .map_err(|_| TransportFailure::MalformedMessage)?
            .as_bytes();
        if self.wrong_publication_identity {
            identity = [0xFF; 32];
        }
        Ok(self.active_adapter_ids.clone().map_or_else(
            || ControllerRuntimeAck::new(self.activation_ack, identity),
            |active_adapter_ids| {
                ControllerRuntimeAck::reported(self.activation_ack, identity, active_adapter_ids)
            },
        ))
    }

    fn update_runtime(
        &mut self,
        target: &ControllerTargetToken,
        publication_json: &str,
        generation: u64,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        let publication = RuntimePublication::decode_json(publication_json)
            .map_err(|_| TransportFailure::MalformedMessage)?;
        if target.as_str() != "private-target"
            || generation != 5
            || publication.generation() != Generation::new(5)
        {
            return Err(TransportFailure::MalformedMessage);
        }
        let identity = publication
            .identity()
            .map_err(|_| TransportFailure::MalformedMessage)?
            .as_bytes();
        Ok(ControllerRuntimeAck::new(5, identity))
    }

    fn control_runtime_diagnostics(
        &mut self,
        target: &ControllerTargetToken,
        _enabled: bool,
    ) -> Result<(), TransportFailure> {
        (target.as_str() == "private-target")
            .then_some(())
            .ok_or(TransportFailure::MalformedMessage)
    }

    fn query_runtime_diagnostics(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<ControllerRuntimeTraceBatch, TransportFailure> {
        if target.as_str() != "private-target" {
            return Err(TransportFailure::MalformedMessage);
        }
        Ok(ControllerRuntimeTraceBatch::new(
            [ControllerRuntimeTraceRecord::new(
                "example.synthetic.inline",
                "Open",
                ControllerRuntimeTraceStatus::Matched,
                ControllerRuntimeTextOutcome::Replaced,
                ControllerRuntimeFontOutcome::Protected,
                4,
                [0x71; 32],
                [0x72; 32],
                [0x73; 32],
            )],
            2,
        ))
    }

    fn deactivate_runtime(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        (target.as_str() == "private-target")
            .then_some(())
            .ok_or(TransportFailure::MalformedMessage)
    }

    fn terminate(&mut self) {}
}

fn local_artifact(name: &str) -> PathBuf {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/local-test/target-process-host");
    std::fs::create_dir_all(&directory).expect("local artifact directory");
    let path = directory.join(name);
    std::fs::File::create(&path).expect("local artifact");
    path
}

fn publication(generation: u64, translation: &str) -> RuntimePublication {
    RuntimePublication::new(
        RouteProgram::direct("menu"),
        TranslationSnapshot::empty(Generation::new(generation)).with_entry(
            "menu",
            "Open",
            translation,
        ),
        FontPolicy::empty(),
    )
}

#[test]
fn tph_001_turns_controller_runtime_acks_into_session_host_facts() {
    let extension_id = ExtensionId::new("org.example.synthetic");
    let mut connection = ControllerConnection::connect(
        ContractTransport {
            activation_ack: 4,
            wrong_publication_identity: false,
            active_adapter_ids: None,
        },
        extension_id,
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([0x51; 32]),
        &mut NonceLedger::new(),
    )
    .expect("controller connection");
    let controller_target = connection
        .inventory()
        .expect("controller inventory")
        .targets()[0]
        .id();
    let runtime_library = local_artifact("runtime.dll");
    let adapter_library = local_artifact("adapter.dll");
    let artifacts = TargetArtifactCatalog::new(
        RuntimeArtifact::new(runtime_library, [0x61; 32]),
        [(
            PackageArtifactId::new("adapters/synthetic"),
            adapter_library,
        )],
    )
    .expect("absolute local artifacts");
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance"),
        TargetFacts::new("windows", "x86_64"),
    );
    let mut host = TargetProcessHost::new(connection, artifacts);
    host.register_target(target.id().clone(), controller_target);

    let adapter_id = AdapterId::new("example.synthetic.inline");
    let version = AdapterVersion::new(1, 0, 0);
    let descriptor = AdapterDescriptor::new(
        adapter_id.clone(),
        version,
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace],
    );
    let binding = AdapterBinding {
        descriptor,
        adapter_id: adapter_id.clone(),
        version,
        apply_model: ApplyModel::InlineRender,
        artifact_hash: ArtifactHash::sha256([0x62; 32]),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/synthetic"),
        },
        features: vec![Feature::TextReplace],
    };
    let feature = BoundFeature::new(adapter_id.clone(), version, Feature::TextReplace);

    assert_eq!(
        host.activate_runtime(
            &target,
            std::slice::from_ref(&binding),
            &publication(4, "First"),
        ),
        Ok(HostActivation::connected([feature]))
    );
    host.control_runtime_diagnostics(SessionId::new(1), &target, true)
        .expect("enable runtime diagnostics");
    let diagnostics = host
        .query_runtime_diagnostics(SessionId::new(1), &target)
        .expect("query runtime diagnostics");
    assert_eq!(diagnostics.records().len(), 1);
    assert_eq!(diagnostics.records()[0].source_text(), "Open");
    assert_eq!(
        diagnostics.records()[0].status(),
        RuntimeTraceStatus::Matched
    );
    assert_eq!(
        diagnostics.records()[0].text(),
        RuntimeTextOutcome::Replaced
    );
    assert_eq!(
        diagnostics.records()[0].font(),
        RuntimeFontOutcome::Protected
    );
    assert_eq!(diagnostics.dropped(), 2);
    assert_eq!(
        host.update_runtime(
            SessionId::new(1),
            &target,
            std::slice::from_ref(&binding),
            &publication(5, "Second"),
        ),
        Ok(HostGenerationReport::target_runtime(Generation::new(5)))
    );
    assert_eq!(
        host.deactivate(SessionId::new(1), &target, &[binding], &[]),
        Ok(HostDeactivation::completed([BoundAdapter::new(
            adapter_id, version,
        )]))
    );
}

#[test]
fn tph_005_reports_the_active_target_adapter_subset_to_the_session() {
    let active_id = AdapterId::new("example.synthetic.compatible");
    let failed_id = AdapterId::new("example.synthetic.unavailable");
    let extension_id = ExtensionId::new("org.example.synthetic");
    let mut connection = ControllerConnection::connect(
        ContractTransport {
            activation_ack: 4,
            wrong_publication_identity: false,
            active_adapter_ids: Some(vec![active_id.clone()]),
        },
        extension_id,
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([0x54; 32]),
        &mut NonceLedger::new(),
    )
    .expect("controller connection");
    let controller_target = connection
        .inventory()
        .expect("controller inventory")
        .targets()[0]
        .id();
    let active_artifact = PackageArtifactId::new("adapters/compatible");
    let failed_artifact = PackageArtifactId::new("adapters/unavailable");
    let artifacts = TargetArtifactCatalog::new(
        RuntimeArtifact::new(local_artifact("runtime-partial.dll"), [0x61; 32]),
        [
            (
                active_artifact.clone(),
                local_artifact("adapter-compatible.dll"),
            ),
            (
                failed_artifact.clone(),
                local_artifact("adapter-unavailable.dll"),
            ),
        ],
    )
    .expect("absolute local artifacts");
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-partial"),
        TargetFacts::new("windows", "x86_64"),
    );
    let mut host = TargetProcessHost::new(connection, artifacts);
    host.register_target(target.id().clone(), controller_target);
    let version = AdapterVersion::new(1, 0, 0);
    let binding = |adapter_id: AdapterId, library: PackageArtifactId| AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::InlineRender,
            Placement::TargetProcess,
            [Feature::TextReplace],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::InlineRender,
        artifact_hash: ArtifactHash::sha256([0x62; 32]),
        host: AdapterHostBinding::TargetProcess { library },
        features: vec![Feature::TextReplace],
    };
    let bindings = [
        binding(active_id.clone(), active_artifact),
        binding(failed_id.clone(), failed_artifact),
    ];

    assert_eq!(
        host.activate_runtime(&target, &bindings, &publication(4, "First")),
        Ok(HostActivation::reported(
            [BoundFeature::new(active_id, version, Feature::TextReplace,)],
            [BoundFeature::new(failed_id, version, Feature::TextReplace,)],
        ))
    );
}

#[test]
fn tph_002_rejects_a_controller_ack_for_the_wrong_generation() {
    let extension_id = ExtensionId::new("org.example.synthetic");
    let mut connection = ControllerConnection::connect(
        ContractTransport {
            activation_ack: 3,
            wrong_publication_identity: false,
            active_adapter_ids: None,
        },
        extension_id,
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([0x52; 32]),
        &mut NonceLedger::new(),
    )
    .expect("controller connection");
    let controller_target = connection
        .inventory()
        .expect("controller inventory")
        .targets()[0]
        .id();
    let artifacts = TargetArtifactCatalog::new(
        RuntimeArtifact::new(local_artifact("runtime-mismatch.dll"), [0x61; 32]),
        [(
            PackageArtifactId::new("adapters/synthetic"),
            local_artifact("adapter-mismatch.dll"),
        )],
    )
    .expect("absolute local artifacts");
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-mismatch"),
        TargetFacts::new("windows", "x86_64"),
    );
    let mut host = TargetProcessHost::new(connection, artifacts);
    host.register_target(target.id().clone(), controller_target);
    let adapter_id = AdapterId::new("example.synthetic.inline");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::InlineRender,
            Placement::TargetProcess,
            [Feature::TextReplace],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::InlineRender,
        artifact_hash: ArtifactHash::sha256([0x62; 32]),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/synthetic"),
        },
        features: vec![Feature::TextReplace],
    };

    assert_eq!(
        host.activate_runtime(&target, &[binding], &publication(4, "First")),
        Err(glyphshift_session::HostFailure::HandshakeRejected)
    );
}

#[test]
fn tph_003_rejects_a_controller_ack_for_the_wrong_publication_identity() {
    let extension_id = ExtensionId::new("org.example.synthetic");
    let mut connection = ControllerConnection::connect(
        ContractTransport {
            activation_ack: 4,
            wrong_publication_identity: true,
            active_adapter_ids: None,
        },
        extension_id,
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([0x53; 32]),
        &mut NonceLedger::new(),
    )
    .expect("controller connection");
    let controller_target = connection
        .inventory()
        .expect("controller inventory")
        .targets()[0]
        .id();
    let artifacts = TargetArtifactCatalog::new(
        RuntimeArtifact::new(local_artifact("runtime-identity-mismatch.dll"), [0x61; 32]),
        [(
            PackageArtifactId::new("adapters/synthetic"),
            local_artifact("adapter-identity-mismatch.dll"),
        )],
    )
    .expect("absolute local artifacts");
    let target = TargetInstance::new(
        TargetInstanceId::new("target-instance-identity-mismatch"),
        TargetFacts::new("windows", "x86_64"),
    );
    let mut host = TargetProcessHost::new(connection, artifacts);
    host.register_target(target.id().clone(), controller_target);
    let adapter_id = AdapterId::new("example.synthetic.inline");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::InlineRender,
            Placement::TargetProcess,
            [Feature::TextReplace],
        ),
        adapter_id,
        version,
        apply_model: ApplyModel::InlineRender,
        artifact_hash: ArtifactHash::sha256([0x62; 32]),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/synthetic"),
        },
        features: vec![Feature::TextReplace],
    };

    assert_eq!(
        host.activate_runtime(&target, &[binding], &publication(4, "First")),
        Err(glyphshift_session::HostFailure::HandshakeRejected)
    );
}

#[derive(Default)]
struct CaptureTransportState {
    queries: u64,
    paused: bool,
    deactivated: bool,
}

struct CaptureTransport {
    state: Arc<Mutex<CaptureTransportState>>,
}

impl ControllerTransport for CaptureTransport {
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
        Ok(ControllerInventory::new(
            [],
            [ControllerTarget::new(
                ControllerTargetToken::new("capture-target"),
                "Synthetic capture target",
                TargetFacts::new("windows", "x86_64"),
            )],
        ))
    }

    fn activate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
        deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        let decoded = TargetRuntimeDeployment::decode_json(deployment.deployment_json())
            .map_err(|_| TransportFailure::MalformedMessage)?;
        let producer = decoded
            .observation_producer()
            .ok_or(TransportFailure::MalformedMessage)?;
        if decoded.capture().is_some()
            || producer.producer_id().as_str() != "target-1"
            || producer.generation() != 1
        {
            return Err(TransportFailure::MalformedMessage);
        }
        let identity = decoded
            .publication()
            .identity()
            .map_err(|_| TransportFailure::MalformedMessage)?
            .as_bytes();
        Ok(ControllerRuntimeAck::new(deployment.generation(), identity))
    }

    fn control_capture(
        &mut self,
        _target: &ControllerTargetToken,
        paused: bool,
    ) -> Result<(), TransportFailure> {
        self.state
            .lock()
            .map_err(|_| TransportFailure::MalformedMessage)?
            .paused = paused;
        Ok(())
    }

    fn query_observations(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<CaptureObservationBatch, TransportFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| TransportFailure::MalformedMessage)?;
        state.queries += 1;
        let (dropped, records) = match state.queries {
            1 => (
                0,
                vec![
                    CaptureObservationRecord::new(1, "example.synthetic.observe", "Open")
                        .map_err(|_| TransportFailure::MalformedMessage)?,
                ],
            ),
            2 => (
                1,
                vec![
                    CaptureObservationRecord::new(3, "example.synthetic.observe", "File")
                        .map_err(|_| TransportFailure::MalformedMessage)?,
                ],
            ),
            _ => (1, Vec::new()),
        };
        CaptureObservationBatch::new(
            CaptureProducerId::new("target-1").map_err(|_| TransportFailure::MalformedMessage)?,
            1,
            dropped,
            records,
        )
        .map_err(|_| TransportFailure::MalformedMessage)
    }

    fn deactivate_runtime(
        &mut self,
        _target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        self.state
            .lock()
            .map_err(|_| TransportFailure::MalformedMessage)?
            .deactivated = true;
        Ok(())
    }

    fn terminate(&mut self) {}
}

#[test]
fn tph_004_owns_one_checkpoint_outside_the_target_and_drains_before_stop() {
    let state = Arc::new(Mutex::new(CaptureTransportState::default()));
    let mut connection = ControllerConnection::connect(
        CaptureTransport {
            state: state.clone(),
        },
        ExtensionId::new("org.example.capture-owner"),
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([0x54; 32]),
        &mut NonceLedger::new(),
    )
    .expect("capture controller connection");
    let controller_target = connection
        .inventory()
        .expect("capture controller inventory")
        .targets()[0]
        .id();
    let artifacts = TargetArtifactCatalog::new(
        RuntimeArtifact::new(local_artifact("runtime-capture-owner.dll"), [0x61; 32]),
        [(
            PackageArtifactId::new("adapters/capture-owner"),
            local_artifact("adapter-capture-owner.dll"),
        )],
    )
    .expect("capture owner artifacts");
    let output = local_artifact("capture-owner.json");
    for checkpoint in [
        output.with_extension("a.json"),
        output.with_extension("b.json"),
    ] {
        let _ = std::fs::remove_file(checkpoint);
    }
    let capture = CaptureConfiguration::new(
        CaptureSessionId::new("capture-owner").expect("capture session id"),
        &output,
        10,
    )
    .expect("capture configuration");
    let target = TargetInstance::new(
        TargetInstanceId::new("capture-target-instance"),
        TargetFacts::new("windows", "x86_64"),
    );
    let sink = FileCaptureSink::start(capture).expect("Desktop capture owner");
    let mut host =
        TargetProcessHost::new(connection, artifacts).with_capture_ingress(sink.ingress());
    host.register_target(target.id().clone(), controller_target);
    let adapter_id = AdapterId::new("example.synthetic.observe");
    let version = AdapterVersion::new(1, 0, 0);
    let binding = AdapterBinding {
        descriptor: AdapterDescriptor::new(
            adapter_id.clone(),
            version,
            ApplyModel::ObserveOnly,
            Placement::TargetProcess,
            [Feature::TextObserve],
        ),
        adapter_id: adapter_id.clone(),
        version,
        apply_model: ApplyModel::ObserveOnly,
        artifact_hash: ArtifactHash::sha256([0x62; 32]),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/capture-owner"),
        },
        features: vec![Feature::TextObserve],
    };

    host.activate_runtime(
        &target,
        std::slice::from_ref(&binding),
        &publication(4, "Unused"),
    )
    .expect("activate capture producer");
    host.control_capture(SessionId::new(1), &target, true)
        .expect("pause after draining producer");
    assert!(state.lock().expect("capture state").paused);
    host.control_capture(SessionId::new(1), &target, false)
        .expect("resume central capture owner");
    assert!(!state.lock().expect("capture state").paused);
    host.deactivate(SessionId::new(1), &target, &[binding], &[])
        .expect("drain then deactivate capture producer");
    sink.finish().expect("finish Desktop capture owner");

    let catalog = CaptureCatalog::read_current(&output).expect("central capture checkpoint");
    assert_eq!(catalog.entries().len(), 2);
    assert_eq!(catalog.dropped_observations(), 1);
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "Open"));
    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.source() == "File"));
    let state = state.lock().expect("final capture state");
    assert!(state.queries >= 3);
    assert!(state.paused);
    assert!(state.deactivated);
}
