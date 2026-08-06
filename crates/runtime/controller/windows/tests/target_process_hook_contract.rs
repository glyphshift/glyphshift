#![cfg(windows)]

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_capture::{CaptureProducerConfiguration, CaptureProducerId};
use glyphshift_controller_sdk::{
    ControllerPlugin, WireAdapterRequirement, WireControllerConfiguration, WireFeature,
    WireRuntimeDeployment, WireRuntimeTextOutcome, WireRuntimeTraceStatus,
};
use glyphshift_controller_windows::WindowsController;
use glyphshift_domain::{Feature, Generation, RouteProgram};
use glyphshift_runtime_contract::RuntimePublication;
use glyphshift_target_runtime_contract::{NativeAdapterDeployment, TargetRuntimeDeployment};
use glyphshift_translation::{FontPolicy, TranslationSnapshot};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

const TARGET_BINARY: &str = "glyphshift-windows-runtime-target.exe";
static TARGET_PROCESS_LOCK: Mutex<()> = Mutex::new(());

struct TargetProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl TargetProcess {
    fn spawn(profile: &Path) -> Self {
        use std::os::windows::process::CommandExt;

        let mut child = Command::new(profile.join(TARGET_BINARY))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .expect("synthetic target should start");
        let stdin = child.stdin.take().expect("target stdin");
        let stdout = BufReader::new(child.stdout.take().expect("target stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn render(&mut self) -> String {
        self.command("render")
    }

    fn command(&mut self, command: &str) -> String {
        writeln!(self.stdin, "{command}").expect("send target command");
        self.stdin.flush().expect("flush target command");
        let mut response = String::new();
        self.stdout
            .read_line(&mut response)
            .expect("read target response");
        assert!(!response.is_empty(), "target should return a response");
        response.trim().into()
    }

    fn stop(&mut self) {
        let _ = writeln!(self.stdin, "exit");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

impl Drop for TargetProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn profile_directory() -> PathBuf {
    std::env::current_exe()
        .expect("contract executable path")
        .parent()
        .and_then(Path::parent)
        .expect("Cargo profile directory")
        .to_path_buf()
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

fn sha256(path: &Path) -> [u8; 32] {
    let mut file = File::open(path).expect("artifact file");
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).expect("artifact digest");
    digest.finalize().into()
}

fn deployment(profile: &Path, publication: RuntimePublication) -> TargetRuntimeDeployment {
    let descriptor = glyphshift_adapter_gdi::descriptor();
    let adapter_library = profile.join("glyphshift_adapter_gdi_native.dll");
    let binding = AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256(sha256(&adapter_library)),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/gdi"),
        },
        features: vec![Feature::TextReplace],
    };
    TargetRuntimeDeployment::new(
        publication,
        [NativeAdapterDeployment::new(adapter_library, binding)
            .expect("target-process adapter deployment")],
    )
}

fn console_deployment(profile: &Path, publication: RuntimePublication) -> TargetRuntimeDeployment {
    let descriptor = glyphshift_adapter_console::descriptor();
    let adapter_library = profile.join("glyphshift_adapter_console_native.dll");
    let binding = AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256(sha256(&adapter_library)),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/console"),
        },
        features: vec![Feature::TextObserve],
    };
    TargetRuntimeDeployment::new(
        publication,
        [NativeAdapterDeployment::new(adapter_library, binding)
            .expect("target-process Console observer deployment")],
    )
}

#[test]
fn ctl_windows_004_injects_hook_updates_translation_and_restores_pass_through() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn(&profile);
    let baseline = target.render();

    let descriptor = glyphshift_adapter_gdi::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-target",
            &WireControllerConfiguration {
                executable_names: vec![TARGET_BINARY.into()],
                executable_paths: Vec::new(),
                descendant_executable_names: Vec::new(),
                adapter_requirements: vec![WireAdapterRequirement {
                    adapter_id: descriptor.adapter_id().as_str().into(),
                    version_major: descriptor.version().major(),
                    version_minor: descriptor.version().minor(),
                    version_patch: descriptor.version().patch(),
                    features: vec![WireFeature::TextReplace],
                }],
            },
        )
        .expect("target configuration");
    let inventory = controller.inventory().expect("target inventory");
    let target_token = inventory
        .targets
        .first()
        .expect("synthetic target should be discovered")
        .token
        .clone();

    let first_publication = publication(1, "First translated label");
    let first_identity = first_publication
        .identity()
        .expect("first publication identity")
        .as_bytes();
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let encoded_deployment = deployment(&profile, first_publication)
        .encode_json()
        .expect("runtime deployment json");
    let first_ack = controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: encoded_deployment,
                generation: 1,
            },
        )
        .expect("target Runtime activation");
    assert_eq!(first_ack.generation, 1);
    assert_eq!(first_ack.publication_identity, first_identity);
    assert_eq!(
        first_ack.active_adapter_ids,
        Some(vec![glyphshift_adapter_gdi::ADAPTER_ID.into()])
    );
    controller
        .control_diagnostics(&target_token, true)
        .expect("enable bounded runtime diagnostics");
    let first = target.render();
    assert_ne!(first, baseline, "the injected GDI hook should replace text");
    let first_trace = controller
        .query_diagnostics(&target_token)
        .expect("query target Runtime diagnostics");
    assert_eq!(first_trace.records.len(), 1);
    assert_eq!(first_trace.records[0].source_text, "Open");
    assert_eq!(
        first_trace.records[0].status,
        WireRuntimeTraceStatus::Matched
    );
    assert_eq!(
        first_trace.records[0].text,
        WireRuntimeTextOutcome::Replaced
    );
    assert_eq!(first_trace.records[0].generation, 1);
    assert_eq!(first_trace.records[0].publication_identity, first_identity);

    let second_publication = publication(2, "Second translated label");
    let second_identity = second_publication
        .identity()
        .expect("second publication identity")
        .as_bytes();
    let second_ack = controller
        .update_runtime(
            &target_token,
            &second_publication
                .encode_json()
                .expect("runtime publication json"),
            2,
        )
        .expect("target Runtime update");
    assert_eq!(second_ack.generation, 2);
    assert_eq!(second_ack.publication_identity, second_identity);
    assert_eq!(second_ack.active_adapter_ids, None);
    let second = target.render();
    assert_ne!(second, first, "the hook should use the new translation");

    controller
        .deactivate_runtime(&target_token)
        .expect("target Runtime pass-through");
    assert_eq!(target.render(), baseline, "deactivate should restore text");
    target.stop();
}

#[test]
fn ctl_windows_006_console_observation_requires_deployment_in_each_process_family_member() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn(&profile);
    assert_eq!(target.command("start-console-child"), "child-started");

    let descriptor = glyphshift_adapter_console::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-console-family",
            &WireControllerConfiguration {
                executable_names: vec![TARGET_BINARY.into()],
                executable_paths: Vec::new(),
                descendant_executable_names: vec![TARGET_BINARY.into()],
                adapter_requirements: vec![WireAdapterRequirement {
                    adapter_id: descriptor.adapter_id().as_str().into(),
                    version_major: descriptor.version().major(),
                    version_minor: descriptor.version().minor(),
                    version_patch: descriptor.version().patch(),
                    features: vec![WireFeature::TextObserve],
                }],
            },
        )
        .expect("synthetic Console process family configuration");
    let inventory = controller
        .inventory()
        .expect("synthetic Console process family inventory");
    assert_eq!(inventory.targets.len(), 2);
    let parent_token = inventory.targets[0].token.clone();
    let child_token = inventory.targets[1].token.clone();

    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let deployment = console_deployment(&profile, publication(1, "Unused"))
        .encode_json()
        .expect("Console observer deployment json");
    let runtime = || WireRuntimeDeployment {
        runtime_library: runtime_library.to_string_lossy().into_owned(),
        runtime_library_sha256: sha256(&runtime_library),
        deployment_json: deployment.clone(),
        generation: 1,
    };

    controller
        .activate_runtime(&parent_token, &runtime())
        .expect("parent Runtime activation");
    controller
        .control_diagnostics(&parent_token, true)
        .expect("enable parent diagnostics");

    assert_eq!(target.command("write-console"), "parent-console-attempted");
    let parent_trace = controller
        .query_diagnostics(&parent_token)
        .expect("query parent diagnostics");
    assert_eq!(parent_trace.records.len(), 1);
    assert_eq!(
        parent_trace.records[0].adapter_id,
        descriptor.adapter_id().as_str()
    );
    assert_eq!(parent_trace.records[0].source_text, "ParentConsoleText");

    assert_eq!(
        target.command("child-write-console"),
        "child-console-attempted"
    );
    assert!(controller
        .query_diagnostics(&parent_token)
        .expect("query parent after uninjected child output")
        .records
        .is_empty());

    controller
        .activate_runtime(&child_token, &runtime())
        .expect("child Runtime activation");
    controller
        .control_diagnostics(&child_token, true)
        .expect("enable child diagnostics");
    assert_eq!(
        target.command("child-write-console"),
        "child-console-attempted"
    );
    let child_trace = controller
        .query_diagnostics(&child_token)
        .expect("query child diagnostics");
    assert_eq!(child_trace.records.len(), 1);
    assert_eq!(
        child_trace.records[0].adapter_id,
        descriptor.adapter_id().as_str()
    );
    assert_eq!(child_trace.records[0].source_text, "ChildConsoleText");
    assert!(controller
        .query_diagnostics(&parent_token)
        .expect("query isolated parent diagnostics")
        .records
        .is_empty());

    controller
        .deactivate_runtime(&child_token)
        .expect("child Runtime pass-through");
    controller
        .deactivate_runtime(&parent_token)
        .expect("parent Runtime pass-through");
    assert_eq!(target.command("stop-console-child"), "child-stopped");
    target.stop();
}

#[test]
fn ctl_windows_007_drains_bounded_observation_batches_through_the_controller() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn(&profile);
    let descriptor = glyphshift_adapter_console::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-observation-batch",
            &WireControllerConfiguration {
                executable_names: vec![TARGET_BINARY.into()],
                executable_paths: Vec::new(),
                descendant_executable_names: Vec::new(),
                adapter_requirements: vec![WireAdapterRequirement {
                    adapter_id: descriptor.adapter_id().as_str().into(),
                    version_major: descriptor.version().major(),
                    version_minor: descriptor.version().minor(),
                    version_patch: descriptor.version().patch(),
                    features: vec![WireFeature::TextObserve],
                }],
            },
        )
        .expect("synthetic observation target configuration");
    let target_token = controller
        .inventory()
        .expect("synthetic observation target inventory")
        .targets[0]
        .token
        .clone();
    let deployment = console_deployment(&profile, publication(1, "Unused"))
        .with_observation_producer(
            CaptureProducerConfiguration::new(
                CaptureProducerId::new("synthetic-target").expect("producer id"),
                11,
            )
            .expect("producer configuration"),
        )
        .encode_json()
        .expect("observation producer deployment json");
    let runtime_library = profile.join("glyphshift_target_runtime.dll");

    controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: deployment,
                generation: 1,
            },
        )
        .expect("observation Runtime activation");
    assert_eq!(target.command("write-console"), "parent-console-attempted");

    let batch = controller
        .query_observations(&target_token)
        .expect("query observations through Windows Controller");
    assert_eq!(batch.producer_id, "synthetic-target");
    assert_eq!(batch.generation, 11);
    assert_eq!(batch.dropped_total, 0);
    assert_eq!(batch.records.len(), 1);
    assert_eq!(
        batch.records[0].adapter_id,
        descriptor.adapter_id().as_str()
    );
    assert_eq!(batch.records[0].source, "ParentConsoleText");

    let empty = controller
        .query_observations(&target_token)
        .expect("query empty observation heartbeat");
    assert!(empty.records.is_empty());
    controller
        .deactivate_runtime(&target_token)
        .expect("observation Runtime pass-through");
    target.stop();
}

#[test]
fn ctl_windows_005_rejects_a_changed_runtime_before_target_injection() {
    let profile = profile_directory();
    let original_runtime = profile.join("glyphshift_target_runtime.dll");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../target/local-test/controller-runtime-contract");
    std::fs::create_dir_all(&local_test).expect("local test directory");
    let changed_runtime = local_test.join("changed-runtime.dll");
    std::fs::copy(&original_runtime, &changed_runtime).expect("runtime test copy");
    let mut changed = std::fs::OpenOptions::new()
        .append(true)
        .open(&changed_runtime)
        .expect("changed runtime copy");
    changed.write_all(b"changed").expect("change runtime copy");

    let executable = std::env::current_exe()
        .expect("contract executable")
        .file_name()
        .expect("contract executable name")
        .to_string_lossy()
        .into_owned();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-target",
            &WireControllerConfiguration {
                executable_names: vec![executable],
                executable_paths: Vec::new(),
                descendant_executable_names: Vec::new(),
                adapter_requirements: Vec::new(),
            },
        )
        .expect("test process configuration");
    let target_token = controller
        .inventory()
        .expect("test process inventory")
        .targets[0]
        .token
        .clone();
    let encoded_deployment = TargetRuntimeDeployment::new(publication(1, "Changed"), [])
        .encode_json()
        .expect("runtime deployment json");

    assert!(controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: changed_runtime.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&original_runtime),
                deployment_json: encoded_deployment,
                generation: 1,
            },
        )
        .is_err());
}
