use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_domain::{AdapterId, Feature, TargetFacts};
use glyphshift_extension::{ExtensionId, ProtocolVersion};
use glyphshift_protocol::{
    ControllerConnection, ControllerHealth, ControllerHello, ControllerInstallation,
    ControllerInstallationToken, ControllerInventory, ControllerLaunchAck, ControllerNonce,
    ControllerOperation, ControllerProtocolError, ControllerRecipe, ControllerTarget,
    ControllerTargetToken, ControllerTransport, LaunchState, NonceLedger,
    RecipeControllerLossPolicy, RecipeDirective, RecipeViolation, TransportFailure,
};
use std::collections::BTreeSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Debug)]
struct HelloTransport {
    hello: ControllerHello,
    terminated: Arc<AtomicBool>,
}

impl ControllerTransport for HelloTransport {
    fn handshake(
        &mut self,
        _expected_extension: &ExtensionId,
        _version: ProtocolVersion,
        _nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(self.hello.clone())
    }

    fn terminate(&mut self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct InventoryTransport {
    hello: ControllerHello,
    inventory: Option<ControllerInventory>,
    terminated: Arc<AtomicBool>,
}

impl ControllerTransport for InventoryTransport {
    fn handshake(
        &mut self,
        _expected_extension: &ExtensionId,
        _version: ProtocolVersion,
        _nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(self.hello.clone())
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        self.inventory
            .take()
            .ok_or(TransportFailure::MalformedMessage)
    }

    fn terminate(&mut self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct LaunchTransport {
    hello: ControllerHello,
    inventory: Option<ControllerInventory>,
    launched: Arc<Mutex<Vec<ControllerInstallationToken>>>,
    terminated: Arc<AtomicBool>,
}

impl ControllerTransport for LaunchTransport {
    fn handshake(
        &mut self,
        _expected_extension: &ExtensionId,
        _version: ProtocolVersion,
        _nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(self.hello.clone())
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        self.inventory
            .take()
            .ok_or(TransportFailure::MalformedMessage)
    }

    fn launch(
        &mut self,
        installation: &ControllerInstallationToken,
    ) -> Result<ControllerLaunchAck, TransportFailure> {
        self.launched
            .lock()
            .expect("launch log lock should remain available")
            .push(installation.clone());
        Ok(ControllerLaunchAck::accepted())
    }

    fn terminate(&mut self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct PrepareTransport {
    hello: ControllerHello,
    inventory: Option<ControllerInventory>,
    recipe: Option<ControllerRecipe>,
    terminated: Arc<AtomicBool>,
}

impl ControllerTransport for PrepareTransport {
    fn handshake(
        &mut self,
        _expected_extension: &ExtensionId,
        _version: ProtocolVersion,
        _nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(self.hello.clone())
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        self.inventory
            .take()
            .ok_or(TransportFailure::MalformedMessage)
    }

    fn prepare(
        &mut self,
        _target: &ControllerTargetToken,
        _requested_features: &BTreeSet<Feature>,
    ) -> Result<ControllerRecipe, TransportFailure> {
        self.recipe.take().ok_or(TransportFailure::MalformedMessage)
    }

    fn terminate(&mut self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct FailingInventoryTransport {
    hello: ControllerHello,
    failure: TransportFailure,
    terminated: Arc<AtomicBool>,
}

impl ControllerTransport for FailingInventoryTransport {
    fn handshake(
        &mut self,
        _expected_extension: &ExtensionId,
        _version: ProtocolVersion,
        _nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(self.hello.clone())
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        Err(self.failure)
    }

    fn terminate(&mut self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct CancelTransport {
    hello: ControllerHello,
    inventory: Option<ControllerInventory>,
    cancelled: Arc<Mutex<Vec<ControllerOperation>>>,
    terminated: Arc<AtomicBool>,
}

impl ControllerTransport for CancelTransport {
    fn handshake(
        &mut self,
        _expected_extension: &ExtensionId,
        _version: ProtocolVersion,
        _nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        Ok(self.hello.clone())
    }

    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        self.inventory
            .take()
            .ok_or(TransportFailure::MalformedMessage)
    }

    fn cancel(&mut self, operation: &ControllerOperation) -> Result<(), TransportFailure> {
        self.cancelled
            .lock()
            .expect("cancel log lock should remain available")
            .push(operation.clone());
        Ok(())
    }

    fn terminate(&mut self) {
        self.terminated.store(true, Ordering::SeqCst);
    }
}

#[test]
fn ctl_001_connects_when_version_nonce_and_extension_identity_match() {
    let extension_id = ExtensionId::new("org.example.editor");
    let version = ProtocolVersion::new(1, 0);
    let nonce = ControllerNonce::new([0x41; 32]);
    let transport = HelloTransport {
        hello: ControllerHello::new(extension_id.clone(), version, nonce),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut ledger = NonceLedger::new();

    let connection =
        ControllerConnection::connect(transport, extension_id.clone(), version, nonce, &mut ledger)
            .expect("matching handshake should connect");

    assert_eq!(connection.extension_id(), &extension_id);
    assert_eq!(connection.health(), ControllerHealth::Available);
}

#[test]
fn ctl_002_rejects_a_protocol_version_mismatch_and_terminates_the_plugin() {
    let extension_id = ExtensionId::new("org.example.editor");
    let expected_version = ProtocolVersion::new(1, 0);
    let nonce = ControllerNonce::new([0x42; 32]);
    let terminated = Arc::new(AtomicBool::new(false));
    let transport = HelloTransport {
        hello: ControllerHello::new(extension_id.clone(), ProtocolVersion::new(2, 0), nonce),
        terminated: Arc::clone(&terminated),
    };
    let mut ledger = NonceLedger::new();

    let rejection = ControllerConnection::connect(
        transport,
        extension_id,
        expected_version,
        nonce,
        &mut ledger,
    )
    .expect_err("incompatible protocol versions must reject before inventory");

    assert_eq!(rejection, ControllerProtocolError::VersionMismatch);
    assert!(terminated.load(Ordering::SeqCst));
}

#[test]
fn ctl_003_rejects_nonce_mismatch_and_replay_and_terminates_each_plugin() {
    let extension_id = ExtensionId::new("org.example.editor");
    let version = ProtocolVersion::new(1, 0);
    let nonce = ControllerNonce::new([0x43; 32]);
    let mismatch_terminated = Arc::new(AtomicBool::new(false));
    let mismatch_transport = HelloTransport {
        hello: ControllerHello::new(
            extension_id.clone(),
            version,
            ControllerNonce::new([0x44; 32]),
        ),
        terminated: Arc::clone(&mismatch_terminated),
    };
    let mut ledger = NonceLedger::new();

    assert_eq!(
        ControllerConnection::connect(
            mismatch_transport,
            extension_id.clone(),
            version,
            nonce,
            &mut ledger,
        )
        .expect_err("a wrong nonce must reject the plugin"),
        ControllerProtocolError::NonceMismatch
    );
    assert!(mismatch_terminated.load(Ordering::SeqCst));

    let accepted_transport = HelloTransport {
        hello: ControllerHello::new(extension_id.clone(), version, nonce),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    ControllerConnection::connect(
        accepted_transport,
        extension_id.clone(),
        version,
        nonce,
        &mut ledger,
    )
    .expect("the nonce should be accepted exactly once");

    let replay_terminated = Arc::new(AtomicBool::new(false));
    let replay_transport = HelloTransport {
        hello: ControllerHello::new(extension_id.clone(), version, nonce),
        terminated: Arc::clone(&replay_terminated),
    };
    assert_eq!(
        ControllerConnection::connect(replay_transport, extension_id, version, nonce, &mut ledger,)
            .expect_err("an accepted nonce must not be reused"),
        ControllerProtocolError::NonceReplay(nonce)
    );
    assert!(replay_terminated.load(Ordering::SeqCst));
}

#[test]
fn ctl_004_maps_inventory_tokens_to_opaque_service_ids() {
    let extension_id = ExtensionId::new("org.example.editor");
    let version = ProtocolVersion::new(1, 0);
    let nonce = ControllerNonce::new([0x45; 32]);
    let transport = InventoryTransport {
        hello: ControllerHello::new(extension_id.clone(), version, nonce),
        inventory: Some(ControllerInventory::new(
            [ControllerInstallation::new(
                ControllerInstallationToken::new("private-installation-token"),
                "Example Editor 1",
            )],
            [ControllerTarget::new(
                ControllerTargetToken::new("private-target-token"),
                "Example Document",
                TargetFacts::new("windows", "x86_64"),
            )],
        )),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut ledger = NonceLedger::new();
    let mut connection =
        ControllerConnection::connect(transport, extension_id, version, nonce, &mut ledger)
            .expect("handshake should connect");

    let inventory = connection
        .inventory()
        .expect("inventory should return opaque service views");

    assert_eq!(inventory.installations().len(), 1);
    assert_eq!(inventory.targets().len(), 1);
    assert_eq!(
        inventory.installations()[0].display_name(),
        "Example Editor 1"
    );
    assert_eq!(inventory.targets()[0].display_name(), "Example Document");
    assert_eq!(inventory.targets()[0].facts().architecture(), "x86_64");
    assert_eq!(inventory.installations()[0].id().as_u64(), 1);
    assert_eq!(inventory.targets()[0].id().as_u64(), 1);
}

#[test]
fn ctl_005_returns_a_launch_receipt_without_claiming_session_activity() {
    let extension_id = ExtensionId::new("org.example.editor");
    let version = ProtocolVersion::new(1, 0);
    let nonce = ControllerNonce::new([0x46; 32]);
    let installation_token = ControllerInstallationToken::new("private-installation-token");
    let launched = Arc::new(Mutex::new(Vec::new()));
    let transport = LaunchTransport {
        hello: ControllerHello::new(extension_id.clone(), version, nonce),
        inventory: Some(ControllerInventory::new(
            [ControllerInstallation::new(
                installation_token.clone(),
                "Example Editor 1",
            )],
            std::iter::empty(),
        )),
        launched: Arc::clone(&launched),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut ledger = NonceLedger::new();
    let mut connection =
        ControllerConnection::connect(transport, extension_id, version, nonce, &mut ledger)
            .expect("handshake should connect");
    let inventory = connection.inventory().expect("inventory should succeed");

    let receipt = connection
        .launch(inventory.installations()[0].id())
        .expect("controller should accept launch");

    assert_eq!(receipt.state(), LaunchState::Requested);
    assert_eq!(receipt.installation_id(), inventory.installations()[0].id());
    assert_eq!(
        launched
            .lock()
            .expect("launch log lock should remain available")
            .as_slice(),
        &[installation_token]
    );
}

#[test]
fn ctl_006_returns_a_valid_recipe_for_service_side_adapter_revalidation() {
    let extension_id = ExtensionId::new("org.example.editor");
    let version = ProtocolVersion::new(1, 0);
    let nonce = ControllerNonce::new([0x47; 32]);
    let adapter_requirement = AdapterRequirement::new(
        AdapterId::new("example.synthetic.writeback"),
        AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
        [Feature::TextReplace],
    );
    let transport = PrepareTransport {
        hello: ControllerHello::new(extension_id.clone(), version, nonce),
        inventory: Some(ControllerInventory::new(
            std::iter::empty(),
            [ControllerTarget::new(
                ControllerTargetToken::new("private-target-token"),
                "Example Document",
                TargetFacts::new("windows", "x86_64"),
            )],
        )),
        recipe: Some(ControllerRecipe::new(
            [RecipeDirective::adapter(adapter_requirement.clone())],
            RecipeControllerLossPolicy::Continue,
        )),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut ledger = NonceLedger::new();
    let mut connection =
        ControllerConnection::connect(transport, extension_id, version, nonce, &mut ledger)
            .expect("handshake should connect");
    connection.authorize_capabilities([adapter_requirement.clone()]);
    let inventory = connection.inventory().expect("inventory should succeed");

    let prepared = connection
        .prepare(inventory.targets()[0].id(), [Feature::TextReplace])
        .expect("authorized recipe should pass protocol validation");

    assert_eq!(prepared.requirements(), &[adapter_requirement]);
    assert_eq!(
        prepared.controller_loss_policy(),
        RecipeControllerLossPolicy::Continue
    );
}

#[test]
fn ctl_007_rejects_native_instructions_unknown_adapters_and_unauthorized_features() {
    let allowed_requirement = AdapterRequirement::new(
        AdapterId::new("example.synthetic.allowed"),
        AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
        [Feature::TextReplace],
    );
    let cases = [
        (
            ControllerRecipe::new(
                [RecipeDirective::function_address(0x1234)],
                RecipeControllerLossPolicy::Continue,
            ),
            RecipeViolation::FunctionAddress,
        ),
        (
            ControllerRecipe::new(
                [RecipeDirective::hook_code()],
                RecipeControllerLossPolicy::Continue,
            ),
            RecipeViolation::HookCode,
        ),
        (
            ControllerRecipe::new(
                [RecipeDirective::adapter(AdapterRequirement::new(
                    AdapterId::new("example.synthetic.unknown"),
                    AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
                    [Feature::TextReplace],
                ))],
                RecipeControllerLossPolicy::Continue,
            ),
            RecipeViolation::UnknownAdapter(AdapterId::new("example.synthetic.unknown")),
        ),
        (
            ControllerRecipe::new(
                [RecipeDirective::adapter(AdapterRequirement::new(
                    AdapterId::new("example.synthetic.allowed"),
                    AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
                    [Feature::FontSubstitute],
                ))],
                RecipeControllerLossPolicy::Continue,
            ),
            RecipeViolation::UnauthorizedFeature {
                adapter_id: AdapterId::new("example.synthetic.allowed"),
                feature: Feature::FontSubstitute,
            },
        ),
    ];

    for (index, (recipe, expected_violation)) in cases.into_iter().enumerate() {
        let extension_id = ExtensionId::new(format!("org.example.recipe-case-{index}"));
        let version = ProtocolVersion::new(1, 0);
        let nonce = ControllerNonce::new([0x50 + index as u8; 32]);
        let transport = PrepareTransport {
            hello: ControllerHello::new(extension_id.clone(), version, nonce),
            inventory: Some(ControllerInventory::new(
                std::iter::empty(),
                [ControllerTarget::new(
                    ControllerTargetToken::new("private-target-token"),
                    "Example Document",
                    TargetFacts::new("windows", "x86_64"),
                )],
            )),
            recipe: Some(recipe),
            terminated: Arc::new(AtomicBool::new(false)),
        };
        let mut ledger = NonceLedger::new();
        let mut connection =
            ControllerConnection::connect(transport, extension_id, version, nonce, &mut ledger)
                .expect("handshake should connect");
        connection.authorize_capabilities([allowed_requirement.clone()]);
        let inventory = connection.inventory().expect("inventory should succeed");

        assert_eq!(
            connection.prepare(inventory.targets()[0].id(), [Feature::TextReplace],),
            Err(ControllerProtocolError::InvalidRecipe(expected_violation))
        );
    }
}

#[test]
fn ctl_008_degrades_only_the_controller_that_times_out_crashes_or_sends_malformed_data() {
    let failures = [
        TransportFailure::Timeout,
        TransportFailure::Crashed,
        TransportFailure::MalformedMessage,
    ];
    let version = ProtocolVersion::new(1, 0);
    let mut ledger = NonceLedger::new();

    for (index, failure) in failures.into_iter().enumerate() {
        let extension_id = ExtensionId::new(format!("org.example.failing-controller-{index}"));
        let nonce = ControllerNonce::new([0x60 + index as u8; 32]);
        let terminated = Arc::new(AtomicBool::new(false));
        let transport = FailingInventoryTransport {
            hello: ControllerHello::new(extension_id.clone(), version, nonce),
            failure,
            terminated: Arc::clone(&terminated),
        };
        let mut connection =
            ControllerConnection::connect(transport, extension_id, version, nonce, &mut ledger)
                .expect("handshake should connect before the operation fails");

        assert_eq!(
            connection.inventory(),
            Err(ControllerProtocolError::Transport(failure))
        );
        assert_eq!(connection.health(), ControllerHealth::Degraded);
        assert!(terminated.load(Ordering::SeqCst));
    }

    let healthy_extension = ExtensionId::new("org.example.healthy-controller");
    let healthy_nonce = ControllerNonce::new([0x70; 32]);
    let healthy_transport = InventoryTransport {
        hello: ControllerHello::new(healthy_extension.clone(), version, healthy_nonce),
        inventory: Some(ControllerInventory::new(
            std::iter::empty(),
            std::iter::empty(),
        )),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut healthy = ControllerConnection::connect(
        healthy_transport,
        healthy_extension,
        version,
        healthy_nonce,
        &mut ledger,
    )
    .expect("other controllers should still connect");

    assert!(healthy.inventory().is_ok());
    assert_eq!(healthy.health(), ControllerHealth::Available);
}

#[test]
fn ctl_009_cancels_inventory_and_prepare_without_degrading_or_creating_a_recipe() {
    let version = ProtocolVersion::new(1, 0);
    let mut ledger = NonceLedger::new();
    let inventory_cancelled = Arc::new(Mutex::new(Vec::new()));
    let inventory_extension = ExtensionId::new("org.example.cancel-inventory");
    let inventory_nonce = ControllerNonce::new([0x71; 32]);
    let inventory_transport = CancelTransport {
        hello: ControllerHello::new(inventory_extension.clone(), version, inventory_nonce),
        inventory: Some(ControllerInventory::new(
            std::iter::empty(),
            std::iter::empty(),
        )),
        cancelled: Arc::clone(&inventory_cancelled),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut inventory_connection = ControllerConnection::connect(
        inventory_transport,
        inventory_extension,
        version,
        inventory_nonce,
        &mut ledger,
    )
    .expect("inventory connection should start");

    inventory_connection
        .cancel_inventory()
        .expect("inventory cancellation should be delivered");

    assert_eq!(
        inventory_connection.inventory(),
        Err(ControllerProtocolError::Transport(
            TransportFailure::Cancelled
        ))
    );
    assert_eq!(
        inventory_cancelled
            .lock()
            .expect("cancel log lock should remain available")
            .as_slice(),
        &[ControllerOperation::Inventory]
    );
    assert_eq!(inventory_connection.health(), ControllerHealth::Available);

    let prepare_cancelled = Arc::new(Mutex::new(Vec::new()));
    let prepare_extension = ExtensionId::new("org.example.cancel-prepare");
    let prepare_nonce = ControllerNonce::new([0x72; 32]);
    let target_token = ControllerTargetToken::new("private-target-token");
    let prepare_transport = CancelTransport {
        hello: ControllerHello::new(prepare_extension.clone(), version, prepare_nonce),
        inventory: Some(ControllerInventory::new(
            std::iter::empty(),
            [ControllerTarget::new(
                target_token.clone(),
                "Example Document",
                TargetFacts::new("windows", "x86_64"),
            )],
        )),
        cancelled: Arc::clone(&prepare_cancelled),
        terminated: Arc::new(AtomicBool::new(false)),
    };
    let mut prepare_connection = ControllerConnection::connect(
        prepare_transport,
        prepare_extension,
        version,
        prepare_nonce,
        &mut ledger,
    )
    .expect("prepare connection should start");
    let inventory = prepare_connection
        .inventory()
        .expect("target inventory should succeed");
    let target_id = inventory.targets()[0].id();

    prepare_connection
        .cancel_prepare(target_id)
        .expect("prepare cancellation should be delivered");

    assert_eq!(
        prepare_connection.prepare(target_id, [Feature::TextReplace]),
        Err(ControllerProtocolError::Transport(
            TransportFailure::Cancelled
        ))
    );
    assert_eq!(
        prepare_cancelled
            .lock()
            .expect("cancel log lock should remain available")
            .as_slice(),
        &[ControllerOperation::Prepare(target_token)]
    );
    assert_eq!(prepare_connection.health(), ControllerHealth::Available);
}
