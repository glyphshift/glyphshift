use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_capture::{
    CaptureConfiguration, CaptureProducerConfiguration, CaptureProducerId, CaptureSessionId,
};
use glyphshift_domain::{
    AbiVersion, AdapterId, ApplyModel, Feature, Generation, Placement, RouteProgram,
};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime_contract::{NativeAdapterDeployment, TargetRuntimeDeployment};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};

#[test]
fn trc_001_round_trips_verified_binding_evidence_and_publication() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.runtime-adapter"),
        AdapterVersion::new(2, 1, 3),
        ApplyModel::InlineRender,
        Placement::TargetProcess,
        [Feature::TextReplace, Feature::FontSubstitute],
    )
    .with_platforms(["windows"])
    .with_architectures(["x86_64"])
    .with_abi(AbiVersion::new(1, 4));
    let binding = AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256([0x31; 32]),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/runtime"),
        },
        features: vec![Feature::TextReplace],
    };
    let deployment = TargetRuntimeDeployment::new(
        RuntimePublication::new(
            RouteProgram::direct("menu"),
            TranslationSnapshot::empty(Generation::new(8)).with_entry("menu", "Open", "打开"),
            FontPolicy::empty(),
        ),
        [
            NativeAdapterDeployment::new("artifacts/adapter.dll", binding)
                .expect("target-process deployment"),
        ],
    )
    .with_capture(
        CaptureConfiguration::new(
            CaptureSessionId::new("capture-contract").expect("capture id"),
            synthetic_capture_path(),
            500,
        )
        .expect("capture configuration"),
    );

    let encoded = deployment.encode_json().expect("deployment encode");
    let decoded = TargetRuntimeDeployment::decode_json(&encoded).expect("deployment decode");

    assert_eq!(decoded, deployment);
    assert!(encoded.contains("glyphshift.target-runtime/3"));
    assert!(encoded.contains("windows"));
    assert!(encoded.contains("capture-contract"));
    assert!(!encoded.contains("process_id"));
    assert!(!encoded.contains("driver"));
}

#[test]
fn trc_003_round_trips_an_observation_producer_without_a_checkpoint_path() {
    let deployment = TargetRuntimeDeployment::new(
        RuntimePublication::new(
            RouteProgram::direct("capture"),
            TranslationSnapshot::empty(Generation::new(1)),
            FontPolicy::empty(),
        ),
        std::iter::empty(),
    )
    .with_observation_producer(
        CaptureProducerConfiguration::new(
            CaptureProducerId::new("target-12").expect("producer id"),
            4,
        )
        .expect("producer configuration"),
    );

    let encoded = deployment.encode_json().expect("producer encode");
    let decoded = TargetRuntimeDeployment::decode_json(&encoded).expect("producer decode");

    assert_eq!(decoded, deployment);
    assert!(decoded.capture().is_none());
    assert_eq!(
        decoded
            .observation_producer()
            .expect("observation producer")
            .producer_id()
            .as_str(),
        "target-12"
    );
    assert!(!encoded.contains("output_path"));
    assert!(!encoded.contains("SyntheticFixtures"));
}

fn synthetic_capture_path() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        std::path::PathBuf::from(r"X:\SyntheticFixtures\capture.json")
    }
    #[cfg(not(windows))]
    {
        std::path::PathBuf::from("/synthetic-fixtures/capture.json")
    }
}

#[test]
fn trc_002_rejects_an_isolated_worker_as_a_target_runtime_library() {
    let descriptor = AdapterDescriptor::new(
        AdapterId::new("example.synthetic.worker"),
        AdapterVersion::new(1, 0, 0),
        ApplyModel::ExternalProtocol,
        Placement::IsolatedWorker,
        [Feature::TextObserve],
    );
    let binding = AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256([0x32; 32]),
        host: AdapterHostBinding::IsolatedWorker {
            executable: PackageArtifactId::new("workers/observe"),
        },
        features: vec![Feature::TextObserve],
    };

    assert!(NativeAdapterDeployment::new("artifacts/worker.exe", binding).is_err());
}
