#![cfg(windows)]

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_capture::{CaptureProducerConfiguration, CaptureProducerId};
use glyphshift_controller_sdk::{
    ControllerPlugin, PluginError, WireAdapterRequirement, WireControllerConfiguration,
    WireFeature, WireRuntimeDeployment, WireRuntimeTextOutcome, WireRuntimeTraceStatus,
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
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};

const TARGET_BINARY: &str = "glyphshift-windows-runtime-target.exe";
const PREPARE_ADAPTER_BINARY: &str = "glyphshift_test_native_adapter.dll";
static TARGET_PROCESS_LOCK: Mutex<()> = Mutex::new(());

struct TargetProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl TargetProcess {
    fn spawn(profile: &Path) -> Self {
        Self::spawn_with_env(profile, &[])
    }

    fn spawn_with_env(profile: &Path, environment: &[(&str, &str)]) -> Self {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new(profile.join(TARGET_BINARY));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000);
        for (name, value) in environment {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("synthetic target should start");
        let stdin = child.stdin.take().expect("target stdin");
        let stdout = BufReader::new(child.stdout.take().expect("target stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn process_id(&self) -> u32 {
        self.child.id()
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

fn module_is_loaded(process_id: u32, module_name: &str) -> bool {
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id)
            .expect("target module snapshot")
    };
    let mut entry = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = false;
    if unsafe { Module32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let length = entry
                .szModule
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(entry.szModule.len());
            let module = String::from_utf16_lossy(&entry.szModule[..length]);
            if module.eq_ignore_ascii_case(module_name) {
                found = true;
                break;
            }
            if unsafe { Module32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    found
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

fn prepare_adapter_deployment(
    profile: &Path,
    publication: RuntimePublication,
) -> TargetRuntimeDeployment {
    let descriptor = glyphshift_test_native_adapter::descriptor();
    let adapter_library = profile.join(PREPARE_ADAPTER_BINARY);
    let binding = AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256(sha256(&adapter_library)),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/synthetic-prepare"),
        },
        features: vec![Feature::TextReplace],
    };
    TargetRuntimeDeployment::new(
        publication,
        [NativeAdapterDeployment::new(adapter_library, binding)
            .expect("synthetic prepare adapter deployment")],
    )
}

fn observation_deployment(
    profile: &Path,
    publication: RuntimePublication,
) -> TargetRuntimeDeployment {
    let descriptor = glyphshift_adapter_gdi::descriptor();
    let adapter_library = profile.join("glyphshift_adapter_gdi_native.dll");
    let binding = AdapterBinding {
        descriptor: descriptor.clone(),
        adapter_id: descriptor.adapter_id().clone(),
        version: descriptor.version(),
        apply_model: descriptor.apply_model(),
        artifact_hash: ArtifactHash::sha256(sha256(&adapter_library)),
        host: AdapterHostBinding::TargetProcess {
            library: PackageArtifactId::new("adapters/gdi-observer"),
        },
        features: vec![Feature::TextObserve],
    };
    TargetRuntimeDeployment::new(
        publication,
        [NativeAdapterDeployment::new(adapter_library, binding)
            .expect("target-process observation deployment")],
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
fn ctl_windows_007_drains_bounded_observation_batches_through_the_controller() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn(&profile);
    let descriptor = glyphshift_adapter_gdi::descriptor();
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
    let deployment = observation_deployment(&profile, publication(1, "Unused"))
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
    let _ = target.render();

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
    assert_eq!(batch.records[0].source, "Open");

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
fn ctl_windows_008_prepares_optional_adapter_before_runtime_activation() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn_with_env(
        &profile,
        &[("GLYPHSHIFT_TEST_NATIVE_ADAPTER_REQUIRE_PREPARE", "1")],
    );

    let descriptor = glyphshift_test_native_adapter::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-prepare",
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
        .expect("prepare target configuration");
    let target_token = controller
        .inventory()
        .expect("prepare target inventory")
        .targets[0]
        .token
        .clone();
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let deployment = prepare_adapter_deployment(&profile, publication(1, "Prepared"))
        .encode_json()
        .expect("prepare deployment json");

    let acknowledgement = controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: deployment,
                generation: 1,
            },
        )
        .expect("prepared Runtime activation");
    assert_eq!(
        acknowledgement.active_adapter_ids,
        Some(vec![glyphshift_test_native_adapter::ADAPTER_ID.into()])
    );
    assert!(module_is_loaded(
        target.process_id(),
        PREPARE_ADAPTER_BINARY
    ));
    controller
        .deactivate_runtime(&target_token)
        .expect("prepared Runtime deactivation");
    target.stop();
}

#[test]
fn ctl_windows_012_waits_for_pending_optional_prepare_before_runtime_activation() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn_with_env(
        &profile,
        &[
            ("GLYPHSHIFT_TEST_NATIVE_ADAPTER_PREPARE_PENDING", "1"),
            ("GLYPHSHIFT_TEST_NATIVE_ADAPTER_REQUIRE_PREPARE", "1"),
        ],
    );
    let descriptor = glyphshift_test_native_adapter::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-prepare-pending",
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
        .expect("pending prepare target configuration");
    let target_token = controller
        .inventory()
        .expect("pending prepare target inventory")
        .targets[0]
        .token
        .clone();
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let deployment = prepare_adapter_deployment(&profile, publication(1, "Pending then ready"))
        .encode_json()
        .expect("pending prepare deployment json");

    let acknowledgement = controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: deployment,
                generation: 1,
            },
        )
        .expect("pending prepare should poll until ready before Runtime activation");
    assert_eq!(
        acknowledgement.active_adapter_ids,
        Some(vec![glyphshift_test_native_adapter::ADAPTER_ID.into()])
    );
    controller
        .deactivate_runtime(&target_token)
        .expect("pending prepare Runtime deactivation");
    target.stop();
}

#[test]
fn ctl_windows_009_releases_adapter_when_optional_prepare_is_rejected() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn_with_env(
        &profile,
        &[("GLYPHSHIFT_TEST_NATIVE_ADAPTER_PREPARE_FAIL", "1")],
    );
    let descriptor = glyphshift_test_native_adapter::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-prepare-rejection",
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
        .expect("prepare rejection target configuration");
    let target_token = controller
        .inventory()
        .expect("prepare rejection target inventory")
        .targets[0]
        .token
        .clone();
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let deployment = prepare_adapter_deployment(&profile, publication(1, "Rejected"))
        .encode_json()
        .expect("prepare rejection deployment json");

    let error = controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: deployment,
                generation: 1,
            },
        )
        .expect_err("prepare rejection should fail activation");
    assert_eq!(
        error,
        PluginError::new("adapter_prepare_failed:adapter_prepare_failed_-4")
    );
    assert!(!module_is_loaded(
        target.process_id(),
        PREPARE_ADAPTER_BINARY
    ));
    target.stop();
}

#[test]
fn ctl_windows_010_reports_optional_prepare_unsupported_without_leaving_adapter_loaded() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn_with_env(
        &profile,
        &[("GLYPHSHIFT_TEST_NATIVE_ADAPTER_PREPARE_UNSUPPORTED", "1")],
    );
    let descriptor = glyphshift_test_native_adapter::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-prepare-unsupported",
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
        .expect("prepare unsupported target configuration");
    let target_token = controller
        .inventory()
        .expect("prepare unsupported target inventory")
        .targets[0]
        .token
        .clone();
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let deployment = prepare_adapter_deployment(&profile, publication(1, "Unsupported"))
        .encode_json()
        .expect("prepare unsupported deployment json");

    let error = controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: deployment,
                generation: 1,
            },
        )
        .expect_err("unsupported prepare should fail activation distinctly");
    assert_eq!(error, PluginError::new("adapter_unsupported"));
    assert!(!module_is_loaded(
        target.process_id(),
        PREPARE_ADAPTER_BINARY
    ));
    target.stop();
}

#[test]
fn ctl_windows_011_releases_preload_when_runtime_activation_fails() {
    let _target_process_guard = TARGET_PROCESS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let profile = profile_directory();
    let mut target = TargetProcess::spawn_with_env(
        &profile,
        &[
            ("GLYPHSHIFT_TEST_NATIVE_ADAPTER_REQUIRE_PREPARE", "1"),
            ("GLYPHSHIFT_TEST_NATIVE_ADAPTER_ACTIVATE_FAIL", "1"),
        ],
    );
    let descriptor = glyphshift_test_native_adapter::descriptor();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.synthetic-activation-failure",
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
        .expect("activation failure target configuration");
    let target_token = controller
        .inventory()
        .expect("activation failure target inventory")
        .targets[0]
        .token
        .clone();
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let deployment = prepare_adapter_deployment(&profile, publication(1, "Failure"))
        .encode_json()
        .expect("activation failure deployment json");

    let error = controller
        .activate_runtime(
            &target_token,
            &WireRuntimeDeployment {
                runtime_library: runtime_library.to_string_lossy().into_owned(),
                runtime_library_sha256: sha256(&runtime_library),
                deployment_json: deployment,
                generation: 1,
            },
        )
        .expect_err("synthetic adapter activation should fail");
    assert_eq!(
        error,
        PluginError::new("runtime_activation_failed:target_runtime_rejected_14")
    );
    assert!(!module_is_loaded(
        target.process_id(),
        PREPARE_ADAPTER_BINARY
    ));
    target.stop();
}

#[test]
fn ctl_windows_005_rejects_a_changed_runtime_before_target_injection() {
    let profile = profile_directory();
    let original_runtime = profile.join("glyphshift_target_runtime.dll");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../local-test/controller-runtime-contract");
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
