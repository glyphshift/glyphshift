use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_domain::{
    AdapterId, ApplyModel, Feature, Generation, Placement, RouteProgram, TargetFacts,
};
use glyphshift_extension::{ExtensionId, ProtocolVersion};
use glyphshift_protocol::{
    ControllerConnection, ControllerHello, ControllerInventory, ControllerNonce,
    ControllerRuntimeAck, ControllerRuntimeDeployment, ControllerTarget, ControllerTargetToken,
    ControllerTransport, NonceLedger, TransportFailure,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_session::{
    AdapterHostPort, BoundAdapter, BoundFeature, HostActivation, HostDeactivation,
    HostGenerationReport, SessionId, TargetInstance, TargetInstanceId,
};
use glyphshift_target_process_host::{RuntimeArtifact, TargetArtifactCatalog, TargetProcessHost};
use glyphshift_target_runtime_contract::TargetRuntimeDeployment;
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use std::path::{Path, PathBuf};

struct ContractTransport {
    activation_ack: u64,
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
            || decoded.adapters().len() != 1
            || decoded.adapters()[0].binding().artifact_hash != ArtifactHash::sha256([0x62; 32])
        {
            return Err(TransportFailure::MalformedMessage);
        }
        Ok(ControllerRuntimeAck::new(self.activation_ack))
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
        Ok(ControllerRuntimeAck::new(5))
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
        ContractTransport { activation_ack: 4 },
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
fn tph_002_rejects_a_controller_ack_for_the_wrong_generation() {
    let extension_id = ExtensionId::new("org.example.synthetic");
    let mut connection = ControllerConnection::connect(
        ContractTransport { activation_ack: 3 },
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
