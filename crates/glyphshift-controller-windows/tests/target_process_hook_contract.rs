#![cfg(windows)]

use glyphshift_adapter_registry::{
    AdapterBinding, AdapterHostBinding, ArtifactHash, PackageArtifactId,
};
use glyphshift_controller_sdk::{
    ControllerPlugin, WireAdapterRequirement, WireControllerConfiguration, WireFeature,
    WireRuntimeDeployment,
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

const TARGET_BINARY: &str = "glyphshift-windows-runtime-target.exe";

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
        writeln!(self.stdin, "render").expect("send render command");
        self.stdin.flush().expect("flush render command");
        let mut evidence = String::new();
        self.stdout
            .read_line(&mut evidence)
            .expect("read render evidence");
        assert!(!evidence.is_empty(), "target should return pixel evidence");
        evidence.trim().into()
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

#[test]
fn ctl_windows_004_injects_hook_updates_translation_and_restores_pass_through() {
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
    let runtime_library = profile.join("glyphshift_target_runtime.dll");
    let encoded_deployment = deployment(&profile, first_publication)
        .encode_json()
        .expect("runtime deployment json");
    assert_eq!(
        controller
            .activate_runtime(
                &target_token,
                &WireRuntimeDeployment {
                    runtime_library: runtime_library.to_string_lossy().into_owned(),
                    runtime_library_sha256: sha256(&runtime_library),
                    deployment_json: encoded_deployment,
                    generation: 1,
                },
            )
            .expect("target Runtime activation"),
        1
    );
    let first = target.render();
    assert_ne!(first, baseline, "the injected GDI hook should replace text");

    let second_publication = publication(2, "Second translated label");
    assert_eq!(
        controller
            .update_runtime(
                &target_token,
                &second_publication
                    .encode_json()
                    .expect("runtime publication json"),
                2,
            )
            .expect("target Runtime update"),
        2
    );
    let second = target.render();
    assert_ne!(second, first, "the hook should use the new translation");

    controller
        .deactivate_runtime(&target_token)
        .expect("target Runtime pass-through");
    assert_eq!(target.render(), baseline, "deactivate should restore text");
    target.stop();
}

#[test]
fn ctl_windows_005_rejects_a_changed_runtime_before_target_injection() {
    let profile = profile_directory();
    let original_runtime = profile.join("glyphshift_target_runtime.dll");
    let local_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/local-test/controller-runtime-contract");
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
