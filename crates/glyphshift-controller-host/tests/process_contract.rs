#![cfg(windows)]

use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_controller_host::{
    measure_code_hash, ControllerLoadError, ControllerStartupConfig, ControllerTrustPolicy,
    ProcessControllerTransport, VerifiedControllerArtifact,
};
use glyphshift_domain::{AdapterId, Feature};
use glyphshift_extension::{
    CodeHash, ControllerArtifactId, ControllerCodeIdentity, ControllerSignerId, ExtensionId,
    ProtocolVersion,
};
use glyphshift_protocol::{
    ControllerConnection, ControllerHealth, ControllerNonce, ControllerProtocolError, NonceLedger,
    TransportFailure,
};
use std::path::PathBuf;
use std::time::Duration;

const SIGNER: &str = "test.controller.signer";
const ADAPTER: &str = "example.synthetic.process-inline";

fn controller_executable() -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join("glyphshift-test-controller-plugin.exe")
}

fn identity(hash: CodeHash) -> ControllerCodeIdentity {
    ControllerCodeIdentity::new(
        ControllerArtifactId::new("controller.synthetic"),
        ControllerSignerId::new(SIGNER),
        hash,
        ProtocolVersion::new(1, 0),
    )
}

fn verified_artifact() -> VerifiedControllerArtifact {
    let path = controller_executable();
    let hash = measure_code_hash(&path).expect("measure controller artifact");
    VerifiedControllerArtifact::verify(path, &identity(hash), &ControllerTrustPolicy::new([SIGNER]))
        .expect("verify controller artifact")
}

fn requirement() -> AdapterRequirement {
    AdapterRequirement::new(
        AdapterId::new(ADAPTER),
        AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
        [Feature::TextReplace, Feature::FontSubstitute],
    )
}

fn connection() -> ControllerConnection<ProcessControllerTransport> {
    let transport = ProcessControllerTransport::spawn_configured(
        verified_artifact(),
        Duration::from_secs(2),
        ControllerStartupConfig::new(["SyntheticEditor.exe"], [requirement()]),
    )
    .expect("spawn configured controller");
    ControllerConnection::connect(
        transport,
        ExtensionId::new("software.synthetic.process"),
        ProtocolVersion::new(1, 0),
        ControllerNonce::new([7; 32]),
        &mut NonceLedger::new(),
    )
    .expect("connect controller")
}

#[test]
fn ctl_process_001_verified_process_runs_inventory_launch_and_authorized_recipe() {
    let mut connection = connection();
    connection.authorize_capabilities([requirement()]);
    let inventory = connection.inventory().expect("inventory");
    assert_eq!(inventory.installations().len(), 1);
    assert_eq!(inventory.targets().len(), 2);

    let receipt = connection
        .launch(inventory.installations()[0].id())
        .expect("launch request");
    assert_eq!(receipt.installation_id(), inventory.installations()[0].id());

    let recipe = connection
        .prepare(
            inventory.targets()[0].id(),
            [Feature::TextReplace, Feature::FontSubstitute],
        )
        .expect("prepare recipe");
    assert_eq!(recipe.requirements().len(), 1);
    assert_eq!(recipe.requirements()[0].adapter_id().as_str(), ADAPTER);
    assert_eq!(connection.health(), ControllerHealth::Available);
}

#[test]
fn ctl_process_002_rejects_untrusted_or_modified_artifacts_before_spawn() {
    let path = controller_executable();
    let observed = measure_code_hash(&path).expect("measure controller artifact");
    assert!(matches!(
        VerifiedControllerArtifact::verify(
            path.clone(),
            &identity(observed),
            &ControllerTrustPolicy::new(["another.signer"]),
        ),
        Err(ControllerLoadError::UntrustedSigner)
    ));
    assert!(matches!(
        VerifiedControllerArtifact::verify(
            path,
            &identity(CodeHash::new([0; 32])),
            &ControllerTrustPolicy::new([SIGNER]),
        ),
        Err(ControllerLoadError::HashMismatch)
    ));
}

#[test]
fn ctl_process_003_one_controller_crash_degrades_only_its_connection() {
    let mut crashing = connection();
    crashing.authorize_capabilities([requirement()]);
    let crash_inventory = crashing.inventory().expect("crash inventory");
    assert_eq!(
        crashing.prepare(crash_inventory.targets()[1].id(), [Feature::TextReplace],),
        Err(ControllerProtocolError::Transport(
            TransportFailure::Crashed
        ))
    );
    assert_eq!(crashing.health(), ControllerHealth::Degraded);

    let mut healthy = connection();
    assert_eq!(
        healthy
            .inventory()
            .expect("independent inventory")
            .targets()
            .len(),
        2
    );
    assert_eq!(healthy.health(), ControllerHealth::Available);
}
