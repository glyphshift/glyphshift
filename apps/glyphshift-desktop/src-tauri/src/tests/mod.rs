use super::*;
use glyphshift_adapter_registry::{AdapterRequirement, AdapterVersion, AdapterVersionRequirement};
use glyphshift_desktop_backend::{
    DictionaryCreate, DictionaryEdit, DictionaryEntryCreate, WorkflowCreate, WorkflowEdit,
    WorkflowTargetCreate,
};
use glyphshift_dictionary_distribution::{
    ArtifactPresentation, DictionaryArtifactDescriptor, FixedInstallationClock,
    InMemoryDictionaryCatalog, InMemoryTrustVerifier, Sha256Digest,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::{Arc, Mutex as StdMutex};
use tempfile::tempdir;

const TEST_ADAPTER_ID: &str = "test.inline";
const TEST_DICTIONARY_URL: &str = "https://catalog.example/dictionary.json";

fn fixture_dictionary_distribution(data_root: &std::path::Path) -> DictionaryDistribution {
    let payload = glyphshift_dictionary_package::DictionaryPackage::create(
        glyphshift_dictionary_package::DictionaryCreate::new(
            "dictionary.catalog",
            "Catalog Dictionary",
            "en-US",
            "zh-CN",
        )
        .with_release_version("1.2.0")
        .with_entries([glyphshift_dictionary_package::DictionaryEntryCreate::new(
            "Open", "打开",
        )]),
    )
    .expect("dictionary package")
    .encode_json()
    .expect("encode dictionary")
    .into_bytes();
    let digest: [u8; 32] = Sha256::digest(&payload).into();
    let publisher = PublisherIdentity::new("publisher.example").expect("publisher");
    let signature =
        SignatureEnvelope::new("fixture", "test-key", "signed-statement").expect("signature");
    let release = CatalogRelease::new(
        DictionaryReleaseKey::new("glyphshift.official", "dictionary.catalog", "1.2.0")
            .expect("release key"),
        "en-US",
        "zh-CN",
        "en-US",
        vec![
            ArtifactPresentation::new("en-US", "Catalog Dictionary", "Menu translations")
                .expect("English presentation")
                .with_tags(["menus", "desktop"])
                .expect("English presentation tags"),
            ArtifactPresentation::new("zh-CN", "目录词典", "菜单翻译")
                .expect("Chinese presentation")
                .with_tags(["菜单", "桌面"])
                .expect("Chinese presentation tags"),
        ],
        DictionaryArtifactDescriptor::new(
            payload.len() as u64,
            Sha256Digest::new(digest),
            [TEST_DICTIONARY_URL],
            publisher.clone(),
            signature.clone(),
        )
        .expect("artifact descriptor"),
    )
    .expect("catalog release");
    DictionaryDistribution::new(
        Box::new(
            InMemoryDictionaryCatalog::new()
                .with_release(release.clone())
                .with_artifact(TEST_DICTIONARY_URL, payload),
        ),
        Box::new(InMemoryTrustVerifier::new().with_trusted_artifact(
            ArtifactStatement::for_release(&release),
            signature,
            publisher,
        )),
        Box::new(FileDictionaryInstallStore::open(data_root).expect("dictionary install store")),
        Box::new(FixedInstallationClock::new(1_700_000_000_000)),
    )
}

#[derive(Default)]
struct WorkflowRuntimeCalls {
    enabled: Vec<(Box<str>, bool, Vec<Box<str>>)>,
    disabled: Vec<Box<str>>,
    refreshed: Vec<Box<str>>,
    captures_started: Vec<Box<str>>,
    captures_stopped: Vec<Box<str>>,
    captures_abandoned: Vec<Box<str>>,
    capture_start_error: Option<DesktopRuntimeError>,
    capture_controls: Vec<(Box<str>, bool)>,
    capture_control_error: Option<DesktopRuntimeError>,
    capture_publications: Vec<(Box<str>, RuntimePublication)>,
    software_removed: Vec<Box<str>>,
}

struct RecordingWorkflowRuntime {
    calls: Arc<StdMutex<WorkflowRuntimeCalls>>,
    start_capture_error: Option<DesktopRuntimeError>,
    capture_capability: ProbeRuntimeCapability,
}

impl WorkflowRuntimeService for RecordingWorkflowRuntime {
    fn activate_workflow(
        &mut self,
        intent: &glyphshift_desktop_backend::EffectiveWorkflowIntent,
        replace_conflicts: bool,
    ) -> WorkflowRuntimeView {
        let software_ids = intent
            .targets()
            .iter()
            .map(|target| Box::<str>::from(target.software_id()))
            .collect::<Vec<_>>();
        self.calls.lock().expect("runtime call log").enabled.push((
            intent.workflow_id().into(),
            replace_conflicts,
            software_ids.clone(),
        ));
        WorkflowRuntimeView {
            workflow_id: intent.workflow_id().into(),
            targets: intent
                .targets()
                .iter()
                .map(|target| WorkflowTargetRuntimeView {
                    software_id: target.software_id().into(),
                    discovered: true,
                    active: true,
                    translation_requested: true,
                    font_requested: false,
                    translation_active: true,
                    font_active: false,
                    applied_generation: Some(
                        target.runtime_spec().publication().generation().value(),
                    ),
                })
                .collect(),
            errors: BTreeMap::new(),
        }
    }

    fn stop_workflow(
        &mut self,
        intent: &glyphshift_desktop_backend::EffectiveWorkflowIntent,
    ) -> WorkflowRuntimeView {
        self.calls
            .lock()
            .expect("runtime call log")
            .disabled
            .push(intent.workflow_id().into());
        WorkflowRuntimeView {
            workflow_id: intent.workflow_id().into(),
            targets: intent
                .targets()
                .iter()
                .map(|target| WorkflowTargetRuntimeView {
                    software_id: target.software_id().into(),
                    discovered: true,
                    active: false,
                    translation_requested: false,
                    font_requested: false,
                    translation_active: false,
                    font_active: false,
                    applied_generation: None,
                })
                .collect(),
            errors: BTreeMap::new(),
        }
    }

    fn refresh_workflow(
        &mut self,
        intent: &glyphshift_desktop_backend::EffectiveWorkflowIntent,
    ) -> WorkflowRuntimeView {
        self.calls
            .lock()
            .expect("runtime call log")
            .refreshed
            .push(intent.workflow_id().into());
        self.activate_workflow(intent, false)
    }

    fn remove_software(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError> {
        self.calls
            .lock()
            .expect("runtime call log")
            .software_removed
            .push(software_id.into());
        Ok(())
    }

    fn start_capture(
        &mut self,
        software_id: &str,
        _spec: &glyphshift_desktop_backend::DesktopRuntimeSpec,
        configuration: CaptureConfiguration,
    ) -> Result<ProbeRuntimeCapability, DesktopRuntimeError> {
        let dynamic_error = {
            let mut calls = self.calls.lock().expect("runtime call log");
            calls.captures_started.push(software_id.into());
            calls.capture_start_error
        };
        if let Some(error) = dynamic_error.or(self.start_capture_error) {
            return Err(error);
        }
        glyphshift_capture::FileCaptureSink::start(configuration)
            .and_then(glyphshift_capture::FileCaptureSink::finish)
            .map(|_| self.capture_capability)
            .map_err(|_| DesktopRuntimeError::SessionRejected)
    }

    fn stop_capture(&mut self, software_id: &str) -> Result<(), DesktopRuntimeError> {
        self.calls
            .lock()
            .expect("runtime call log")
            .captures_stopped
            .push(software_id.into());
        Ok(())
    }

    fn control_capture(
        &mut self,
        software_id: &str,
        paused: bool,
    ) -> Result<(), DesktopRuntimeError> {
        let mut calls = self.calls.lock().expect("runtime call log");
        calls.capture_controls.push((software_id.into(), paused));
        if let Some(error) = calls.capture_control_error {
            return Err(error);
        }
        Ok(())
    }

    fn abandon_capture(&mut self, software_id: &str) {
        self.calls
            .lock()
            .expect("runtime call log")
            .captures_abandoned
            .push(software_id.into());
    }

    fn control_runtime_diagnostics(
        &mut self,
        _software_id: &str,
        _enabled: bool,
    ) -> Result<(), DesktopRuntimeError> {
        Ok(())
    }

    fn query_runtime_diagnostics(
        &mut self,
        _software_id: &str,
    ) -> Result<RuntimeTraceBatch, DesktopRuntimeError> {
        use glyphshift_desktop_runtime::{
            RuntimeFontOutcome, RuntimeTextOutcome, RuntimeTraceStatus,
        };

        Ok(RuntimeTraceBatch::new(
            [RuntimeTraceRecord::new(
                TEST_ADAPTER_ID,
                "Open",
                RuntimeTraceStatus::Matched,
                RuntimeTextOutcome::Replaced,
                RuntimeFontOutcome::Protected,
                4,
                [0x71; 32],
                [0x72; 32],
                [0x73; 32],
            )],
            3,
        ))
    }

    fn publish_capture(
        &mut self,
        software_id: &str,
        publication: RuntimePublication,
    ) -> Result<(), DesktopRuntimeError> {
        self.calls
            .lock()
            .expect("runtime call log")
            .capture_publications
            .push((software_id.into(), publication));
        Ok(())
    }
}

fn workflow_application() -> (
    DesktopApplication,
    Arc<StdMutex<WorkflowRuntimeCalls>>,
    Box<str>,
    tempfile::TempDir,
) {
    let data_root = tempdir().expect("temporary product data");
    let executable = write_synthetic_executable(data_root.path(), "SyntheticWorkflowHost.exe");
    let mut backend =
        DesktopBackend::open_with_environment(data_root.path(), test_desktop_environment())
            .expect("open product backend");
    let software_id: Box<str> = backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add synthetic software")
        .software()[0]
        .id()
        .into();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.product", "产品词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Open", "打开")]),
        )
        .expect("create dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.product", "产品工作流").with_targets([
                WorkflowTargetCreate::new(
                    software_id.clone(),
                    [TEST_ADAPTER_ID],
                    ["dictionary.product"],
                ),
            ]),
        )
        .expect("create workflow");
    let calls = Arc::new(StdMutex::new(WorkflowRuntimeCalls::default()));
    let runtimes: Box<dyn WorkflowRuntimeService> = Box::new(RecordingWorkflowRuntime {
        calls: Arc::clone(&calls),
        start_capture_error: None,
        capture_capability: ProbeRuntimeCapability::DirectReplace,
    });
    (
        test_desktop_application(data_root.path(), backend, runtimes),
        calls,
        software_id,
        data_root,
    )
}

fn write_synthetic_executable(root: &Path, name: &str) -> PathBuf {
    let executable = root.join(name);
    let mut image = vec![0_u8; 256];
    image[0..2].copy_from_slice(b"MZ");
    image[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    image[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
    fs::write(&executable, image).expect("synthetic executable");
    executable
}

fn test_desktop_environment() -> DesktopEnvironment {
    DesktopEnvironment::new(
        [AdapterRequirement::new(
            glyphshift_domain::AdapterId::new(TEST_ADAPTER_ID),
            AdapterVersionRequirement::Exact(AdapterVersion::new(1, 0, 0)),
            [
                Feature::TextObserve,
                Feature::TextReplace,
                Feature::FontSubstitute,
            ],
        )],
        Vec::<Box<str>>::new(),
    )
}

fn test_desktop_application(
    data_root: &Path,
    backend: DesktopBackend,
    runtimes: Box<dyn WorkflowRuntimeService>,
) -> DesktopApplication {
    DesktopApplication {
        backend,
        dictionary_distribution: offline_dictionary_distribution(data_root)
            .expect("offline dictionary distribution"),
        runtimes: Some(runtimes),
        runtime_bundle_error: None,
        workflow_runtime_status: BTreeMap::new(),
        adapters: vec![AdapterView {
            id: TEST_ADAPTER_ID.into(),
            name: "Synthetic adapter".into(),
            version: "1.0.0".into(),
            summary: "Deterministic test adapter".into(),
            platforms: vec!["windows".into()],
            technologies: vec!["Synthetic".into()],
            features: vec!["textObserve".into(), "textReplace".into()],
            technical_target: "SyntheticTarget".into(),
            documentation_url: None,
            configuration: "none".into(),
        }],
        adapter_target_support: BTreeMap::from([(
            TEST_ADAPTER_ID.into(),
            AdapterTargetSupport {
                placement: Placement::TargetProcess,
                platforms: vec!["windows".into()],
                architectures: vec!["x86_64".into()],
                features: vec![Feature::TextObserve, Feature::TextReplace],
            },
        )]),
        font_families: Vec::new(),
        font_cache_root: data_root.to_path_buf(),
        probe_runs: ProbeRunStore::open(data_root.join("probe-runs")).expect("probe run store"),
        quick_probe_sessions: QuickProbeSessionStore::open(data_root)
            .expect("quick probe session store"),
        active_probe_run_id: None,
        active_probe_capability: None,
    }
}

mod dictionary;
mod probe;
mod quick_probe;
mod shell;
mod software;
mod workflow;
