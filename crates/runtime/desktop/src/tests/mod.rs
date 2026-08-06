use super::*;
use glyphshift_desktop_backend::{
    DesktopBackend, DesktopEnvironment, DictionaryCreate, DictionaryEdit, DictionaryEntryCreate,
    ExecutableSelection, WorkflowCreate, WorkflowTargetCreate,
};
use glyphshift_protocol::{
    ControllerHello, ControllerInventory, ControllerTarget, ControllerTargetToken, TransportFailure,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tempfile::tempdir;

const TEST_ADAPTER_ID: &str = "test.inline";

fn open_test_backend(root: impl AsRef<Path>) -> DesktopBackend {
    DesktopBackend::open_with_environment(
        root,
        DesktopEnvironment::new(
            [AdapterRequirement::new(
                glyphshift_domain::AdapterId::new(TEST_ADAPTER_ID),
                AdapterVersionRequirement::Exact(glyphshift_adapter_registry::AdapterVersion::new(
                    1, 0, 0,
                )),
                [
                    Feature::TextObserve,
                    Feature::TextReplace,
                    Feature::FontSubstitute,
                ],
            )],
            Vec::<Box<str>>::new(),
        ),
    )
    .expect("desktop backend")
}

struct InventoryController;

struct InMemoryRuntimeFactory;

impl RuntimeFactory for InMemoryRuntimeFactory {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
        Ok(Box::new(InMemoryRuntime {
            stop_fails: application_id.contains("stopfailure"),
            application_id,
            active_features: BTreeSet::new(),
            generation: spec.publication().generation(),
            target_ids: vec![1],
            captured_target_ids: None,
        }))
    }
}

struct InMemoryRuntime {
    application_id: Box<str>,
    active_features: BTreeSet<Feature>,
    generation: Generation,
    stop_fails: bool,
    target_ids: Vec<u64>,
    captured_target_ids: Option<Arc<Mutex<Vec<u64>>>>,
}

struct FamilyCaptureFactory {
    captured_target_ids: Arc<Mutex<Vec<u64>>>,
}

impl RuntimeFactory for FamilyCaptureFactory {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
        Ok(Box::new(InMemoryRuntime {
            application_id,
            active_features: BTreeSet::new(),
            generation: spec.publication().generation(),
            stop_fails: false,
            target_ids: vec![1, 2, 3],
            captured_target_ids: Some(self.captured_target_ids.clone()),
        }))
    }
}

struct RetryRuntimeFactory {
    discoveries: Arc<AtomicUsize>,
}

impl RuntimeFactory for RetryRuntimeFactory {
    fn discover(
        &mut self,
        application_id: Box<str>,
        spec: &DesktopRuntimeSpec,
    ) -> Result<Box<dyn ManagedRuntime>, DesktopRuntimeError> {
        let reject_capture = self.discoveries.fetch_add(1, Ordering::SeqCst) == 0;
        Ok(Box::new(RetryRuntime {
            inner: InMemoryRuntime {
                application_id,
                active_features: BTreeSet::new(),
                generation: spec.publication().generation(),
                stop_fails: false,
                target_ids: vec![1],
                captured_target_ids: None,
            },
            reject_capture,
        }))
    }
}

struct RetryRuntime {
    inner: InMemoryRuntime,
    reject_capture: bool,
}

impl ManagedRuntime for RetryRuntime {
    fn application_id(&self) -> &str {
        self.inner.application_id()
    }

    fn targets(&self) -> Vec<RuntimeTarget> {
        self.inner.targets()
    }

    fn supported_features(&self) -> BTreeSet<Feature> {
        self.inner.supported_features()
    }

    fn active_features(&self) -> BTreeSet<Feature> {
        self.inner.active_features()
    }

    fn active_target_id(&self) -> Option<u64> {
        self.inner.active_target_id()
    }

    fn applied_generation(&self) -> Option<Generation> {
        self.inner.applied_generation()
    }

    fn start(
        &mut self,
        target_id: u64,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<(), DesktopRuntimeError> {
        self.inner.start(target_id, requested_features)
    }

    fn start_capture(
        &mut self,
        target_ids: &[u64],
        requested_features: &BTreeSet<Feature>,
        capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        if self.reject_capture {
            return Err(DesktopRuntimeError::ProtocolRejected);
        }
        self.inner
            .start_capture(target_ids, requested_features, capture)
    }

    fn publish(
        &mut self,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        self.inner.publish(publication)
    }

    fn control_capture(&mut self, paused: bool) -> Result<(), DesktopRuntimeError> {
        self.inner.control_capture(paused)
    }

    fn control_runtime_diagnostics(&mut self, enabled: bool) -> Result<(), DesktopRuntimeError> {
        self.inner.control_runtime_diagnostics(enabled)
    }

    fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        self.inner.query_runtime_diagnostics()
    }

    fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
        self.inner.stop()
    }
}

impl ManagedRuntime for InMemoryRuntime {
    fn application_id(&self) -> &str {
        &self.application_id
    }

    fn targets(&self) -> Vec<RuntimeTarget> {
        self.target_ids
            .iter()
            .map(|target_id| RuntimeTarget {
                id: *target_id,
                display_name: format!("合成目标 {target_id}").into(),
            })
            .collect()
    }

    fn supported_features(&self) -> BTreeSet<Feature> {
        [
            Feature::TextObserve,
            Feature::TextReplace,
            Feature::FontSubstitute,
        ]
        .into_iter()
        .collect()
    }

    fn active_features(&self) -> BTreeSet<Feature> {
        self.active_features.clone()
    }

    fn active_target_id(&self) -> Option<u64> {
        (!self.active_features.is_empty())
            .then(|| self.target_ids.first().copied())
            .flatten()
    }

    fn applied_generation(&self) -> Option<Generation> {
        (!self.active_features.is_empty()).then_some(self.generation)
    }

    fn start(
        &mut self,
        _target_id: u64,
        requested_features: &BTreeSet<Feature>,
    ) -> Result<(), DesktopRuntimeError> {
        self.active_features = requested_features.clone();
        Ok(())
    }

    fn start_capture(
        &mut self,
        target_ids: &[u64],
        requested_features: &BTreeSet<Feature>,
        _capture: CaptureConfiguration,
    ) -> Result<(), DesktopRuntimeError> {
        if let Some(captured) = &self.captured_target_ids {
            *captured
                .lock()
                .map_err(|_| DesktopRuntimeError::InvalidState)? = target_ids.to_vec();
        }
        self.active_features = requested_features.clone();
        Ok(())
    }

    fn publish(
        &mut self,
        publication: glyphshift_runtime_contract::RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        self.generation = publication.generation();
        Ok(())
    }

    fn control_capture(&mut self, _paused: bool) -> Result<(), DesktopRuntimeError> {
        Ok(())
    }

    fn control_runtime_diagnostics(&mut self, _enabled: bool) -> Result<(), DesktopRuntimeError> {
        Ok(())
    }

    fn query_runtime_diagnostics(&mut self) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        Ok(RuntimeTraceBatch::new(
            [RuntimeTraceRecord::new(
                TEST_ADAPTER_ID,
                "Open",
                RuntimeTraceStatus::Matched,
                RuntimeTextOutcome::Replaced,
                RuntimeFontOutcome::Protected,
                self.generation.value(),
                [1; 32],
                [2; 32],
                [3; 32],
            )],
            3,
        ))
    }

    fn stop(&mut self) -> Result<(), DesktopRuntimeError> {
        if self.stop_fails {
            return Err(DesktopRuntimeError::SessionRejected);
        }
        self.active_features.clear();
        Ok(())
    }
}

impl ControllerTransport for InventoryController {
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
                ControllerTargetToken::new("private-process-token"),
                "MotionCanvas — 主窗口",
                TargetFacts::new("windows", "x86_64"),
            )],
        ))
    }

    fn terminate(&mut self) {}
}

mod bundle;
mod contract;
mod pool;
