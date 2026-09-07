//! One logical connection, architecture-scoped controller tokens. No process pointers cross here.
use super::*;

pub struct ArchitectureControllerTransport<T> {
    controllers: BTreeMap<String, T>,
    failed: BTreeMap<String, TransportFailure>,
}
impl<T: ControllerTransport> ArchitectureControllerTransport<T> {
    pub fn new(
        controllers: impl IntoIterator<Item = (String, T)>,
    ) -> Result<Self, TransportFailure> {
        let mut result = BTreeMap::new();
        for (architecture, controller) in controllers {
            if !matches!(architecture.as_str(), "x86" | "x86_64")
                || result.insert(architecture, controller).is_some()
            {
                return Err(TransportFailure::MalformedMessage);
            }
        }
        if result.is_empty() {
            return Err(TransportFailure::MalformedMessage);
        }
        Ok(Self {
            controllers: result,
            failed: BTreeMap::new(),
        })
    }
    pub fn with_unavailable(
        mut self,
        architecture: String,
        failure: TransportFailure,
    ) -> Result<Self, TransportFailure> {
        if !matches!(architecture.as_str(), "x86" | "x86_64")
            || self.controllers.contains_key(&architecture)
        {
            return Err(TransportFailure::MalformedMessage);
        }
        self.failed.insert(architecture, failure);
        Ok(self)
    }
    fn call<R>(
        &mut self,
        target: &ControllerTargetToken,
        action: impl FnOnce(&mut T, &ControllerTargetToken) -> Result<R, TransportFailure>,
    ) -> Result<R, TransportFailure> {
        let (architecture, token) = target
            .as_str()
            .split_once(':')
            .ok_or(TransportFailure::MalformedMessage)?;
        if self.failed.contains_key(architecture) {
            return Err(TransportFailure::Rejected(ControllerRejection::Unknown));
        }
        let controller = self
            .controllers
            .get_mut(architecture)
            .ok_or(TransportFailure::MalformedMessage)?;
        let result = action(controller, &ControllerTargetToken::new(token));
        match result {
            Err(
                error @ (TransportFailure::Crashed
                | TransportFailure::Timeout
                | TransportFailure::MalformedMessage),
            ) => {
                controller.terminate();
                self.failed.insert(architecture.into(), error);
                // A failed architecture must not terminate a healthy peer via ControllerConnection.
                Err(TransportFailure::Rejected(ControllerRejection::Unknown))
            }
            other => other,
        }
    }
}
impl<T: ControllerTransport> ControllerTransport for ArchitectureControllerTransport<T> {
    fn handshake(
        &mut self,
        extension: &ExtensionId,
        version: ProtocolVersion,
        nonce: ControllerNonce,
    ) -> Result<ControllerHello, TransportFailure> {
        let expected = ControllerHello::new(extension.clone(), version, nonce);
        let mut healthy = 0;
        for (architecture, controller) in &mut self.controllers {
            match controller.handshake(extension, version, nonce) {
                Ok(hello) if hello == expected => healthy += 1,
                other => {
                    controller.terminate();
                    self.failed.insert(
                        architecture.clone(),
                        other.err().unwrap_or(TransportFailure::MalformedMessage),
                    );
                }
            }
        }
        if healthy == 0 {
            Err(TransportFailure::Crashed)
        } else {
            Ok(expected)
        }
    }
    fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
        let mut installations = Vec::new();
        let mut targets = Vec::new();
        let mut healthy = 0;
        for (architecture, controller) in &mut self.controllers {
            if self.failed.contains_key(architecture) {
                continue;
            }
            match controller.inventory() {
                Ok(inventory) => {
                    healthy += 1;
                    installations.extend(inventory.installations.into_iter().map(|v| {
                        ControllerInstallation::new(
                            ControllerInstallationToken::new(format!(
                                "{architecture}:{}",
                                v.token.as_str()
                            )),
                            v.display_name,
                        )
                    }));
                    // Every controller sees the full authorized process family, so a different-
                    // architecture parent can still authorize a matching child. Only deployment
                    // ownership is partitioned here, after discovery.
                    targets.extend(
                        inventory
                            .targets
                            .into_iter()
                            .enumerate()
                            .filter(|(_, v)| v.facts.architecture() == architecture)
                            .map(|(order, v)| {
                                (
                                    order,
                                    ControllerTarget::new(
                                        ControllerTargetToken::new(format!(
                                            "{architecture}:{}",
                                            v.token.as_str()
                                        )),
                                        v.display_name,
                                        v.facts,
                                    ),
                                )
                            }),
                    );
                }
                Err(error) => {
                    controller.terminate();
                    self.failed.insert(architecture.clone(), error);
                }
            }
        }
        if healthy == 0 || (targets.is_empty() && !self.failed.is_empty()) {
            return Err(TransportFailure::Crashed);
        }
        // Preserve the controller's primary/root ordering instead of sorting the
        // user-visible default target by architecture name.
        targets.sort_by_key(|(order, _)| *order);
        Ok(ControllerInventory::new(
            installations,
            targets.into_iter().map(|(_, target)| target),
        ))
    }
    fn launch(
        &mut self,
        installation: &ControllerInstallationToken,
    ) -> Result<ControllerLaunchAck, TransportFailure> {
        let token = ControllerTargetToken::new(installation.as_str());
        self.call(&token, |controller, value| {
            controller.launch(&ControllerInstallationToken::new(value.as_str()))
        })
    }
    fn prepare(
        &mut self,
        target: &ControllerTargetToken,
        features: &BTreeSet<Feature>,
    ) -> Result<ControllerRecipe, TransportFailure> {
        self.call(target, |c, t| c.prepare(t, features))
    }
    fn authorize_worker_target(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<ControllerWorkerTargetGrant, TransportFailure> {
        self.call(target, |c, t| c.authorize_worker_target(t))
    }
    fn activate_runtime(
        &mut self,
        target: &ControllerTargetToken,
        deployment: &ControllerRuntimeDeployment,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        self.call(target, |c, t| c.activate_runtime(t, deployment))
    }
    fn update_runtime(
        &mut self,
        target: &ControllerTargetToken,
        publication: &str,
        generation: u64,
    ) -> Result<ControllerRuntimeAck, TransportFailure> {
        self.call(target, |c, t| c.update_runtime(t, publication, generation))
    }
    fn control_capture(
        &mut self,
        target: &ControllerTargetToken,
        paused: bool,
    ) -> Result<(), TransportFailure> {
        self.call(target, |c, t| c.control_capture(t, paused))
    }
    fn control_runtime_diagnostics(
        &mut self,
        target: &ControllerTargetToken,
        enabled: bool,
    ) -> Result<(), TransportFailure> {
        self.call(target, |c, t| c.control_runtime_diagnostics(t, enabled))
    }
    fn query_runtime_diagnostics(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<ControllerRuntimeTraceBatch, TransportFailure> {
        self.call(target, |c, t| c.query_runtime_diagnostics(t))
    }
    fn query_observations(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<CaptureObservationBatch, TransportFailure> {
        self.call(target, |c, t| c.query_observations(t))
    }
    fn deactivate_runtime(
        &mut self,
        target: &ControllerTargetToken,
    ) -> Result<(), TransportFailure> {
        self.call(target, |c, t| c.deactivate_runtime(t))
    }
    fn cancel(&mut self, operation: &ControllerOperation) -> Result<(), TransportFailure> {
        match operation {
            ControllerOperation::Prepare(target) => self.call(target, |c, t| {
                c.cancel(&ControllerOperation::Prepare(t.clone()))
            }),
            ControllerOperation::Inventory => {
                for c in self.controllers.values_mut() {
                    c.cancel(operation)?;
                }
                Ok(())
            }
        }
    }
    fn terminate(&mut self) {
        for c in self.controllers.values_mut() {
            c.terminate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    struct Fake {
        arch: &'static str,
        calls: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }
    impl ControllerTransport for Fake {
        fn handshake(
            &mut self,
            e: &ExtensionId,
            v: ProtocolVersion,
            n: ControllerNonce,
        ) -> Result<ControllerHello, TransportFailure> {
            Ok(ControllerHello::new(e.clone(), v, n))
        }
        fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
            Ok(ControllerInventory::new(
                [],
                [
                    ControllerTarget::new(
                        ControllerTargetToken::new("same-token"),
                        "fixture",
                        TargetFacts::new("windows", self.arch),
                    ),
                    ControllerTarget::new(
                        ControllerTargetToken::new("other-bitness"),
                        "peer",
                        TargetFacts::new(
                            "windows",
                            if self.arch == "x86" { "x86_64" } else { "x86" },
                        ),
                    ),
                ],
            ))
        }
        fn deactivate_runtime(
            &mut self,
            t: &ControllerTargetToken,
        ) -> Result<(), TransportFailure> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{}:{}", self.arch, t.as_str()));
            if self.fail {
                Err(TransportFailure::Crashed)
            } else {
                Ok(())
            }
        }
        fn terminate(&mut self) {}
    }
    #[test]
    fn scopes_identical_tokens_and_keeps_a_healthy_peer() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transport = ArchitectureControllerTransport::new([
            (
                "x86".into(),
                Fake {
                    arch: "x86",
                    calls: calls.clone(),
                    fail: true,
                },
            ),
            (
                "x86_64".into(),
                Fake {
                    arch: "x86_64",
                    calls: calls.clone(),
                    fail: false,
                },
            ),
        ])
        .unwrap();
        let inventory = transport.inventory().unwrap();
        assert_eq!(inventory.targets.len(), 2);
        assert_eq!(inventory.targets[0].token.as_str(), "x86:same-token");
        assert!(matches!(
            transport.deactivate_runtime(&inventory.targets[0].token),
            Err(TransportFailure::Rejected(_))
        ));
        transport
            .deactivate_runtime(&inventory.targets[1].token)
            .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            ["x86:same-token", "x86_64:same-token"]
        );
        assert_eq!(transport.inventory().unwrap().targets.len(), 1);
    }
    #[test]
    fn rejects_unknown_architecture() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        assert!(ArchitectureControllerTransport::new([(
            "unknown".into(),
            Fake {
                arch: "x86",
                calls,
                fail: false
            }
        )])
        .is_err());
    }
    #[test]
    fn an_unavailable_x86_controller_does_not_block_x64_discovery() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut transport = ArchitectureControllerTransport::new([(
            "x86_64".into(),
            Fake {
                arch: "x86_64",
                calls: calls.clone(),
                fail: false,
            },
        )])
        .unwrap()
        .with_unavailable("x86".into(), TransportFailure::Crashed)
        .unwrap();
        transport
            .handshake(
                &ExtensionId::new("synthetic.dual"),
                ProtocolVersion::new(1, 0),
                ControllerNonce::new([7; 32]),
            )
            .unwrap();
        let inventory = transport.inventory().unwrap();
        assert_eq!(inventory.targets.len(), 1);
        assert!(matches!(
            transport.deactivate_runtime(&ControllerTargetToken::new("x86:old")),
            Err(TransportFailure::Rejected(_))
        ));
        transport
            .deactivate_runtime(&inventory.targets[0].token)
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), ["x86_64:same-token"]);
    }
    #[test]
    fn mixed_family_keeps_the_root_as_the_default_target() {
        struct Family;
        impl ControllerTransport for Family {
            fn handshake(
                &mut self,
                e: &ExtensionId,
                v: ProtocolVersion,
                n: ControllerNonce,
            ) -> Result<ControllerHello, TransportFailure> {
                Ok(ControllerHello::new(e.clone(), v, n))
            }
            fn inventory(&mut self) -> Result<ControllerInventory, TransportFailure> {
                Ok(ControllerInventory::new(
                    [],
                    [
                        ControllerTarget::new(
                            ControllerTargetToken::new("root"),
                            "root",
                            TargetFacts::new("windows", "x86_64"),
                        ),
                        ControllerTarget::new(
                            ControllerTargetToken::new("child"),
                            "child",
                            TargetFacts::new("windows", "x86"),
                        ),
                    ],
                ))
            }
            fn terminate(&mut self) {}
        }
        let mut router = ArchitectureControllerTransport::new([
            ("x86".into(), Family),
            ("x86_64".into(), Family),
        ])
        .unwrap();
        let inventory = router.inventory().unwrap();
        assert_eq!(inventory.targets[0].token.as_str(), "x86_64:root");
        assert_eq!(inventory.targets[1].token.as_str(), "x86:child");
    }
}
