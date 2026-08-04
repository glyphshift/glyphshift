#![cfg(windows)]

use glyphshift_controller_sdk::{
    ControllerPlugin, WireAdapterRequirement, WireControllerConfiguration, WireFeature,
};
use glyphshift_controller_windows::WindowsController;

fn configured_controller() -> WindowsController {
    let executable = std::env::current_exe()
        .expect("test executable should have a path")
        .file_name()
        .expect("test executable should have a file name")
        .to_string_lossy()
        .into_owned();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.windows-target",
            &WireControllerConfiguration {
                executable_names: vec![executable],
                executable_paths: Vec::new(),
                descendant_executable_names: Vec::new(),
                adapter_requirements: vec![WireAdapterRequirement {
                    adapter_id: "example.synthetic.inline".into(),
                    version_major: 1,
                    version_minor: 0,
                    version_patch: 0,
                    features: vec![WireFeature::TextReplace, WireFeature::FontSubstitute],
                }],
            },
        )
        .expect("a bare executable name should configure discovery");
    controller
}

#[test]
fn ctl_windows_001_discovers_a_real_process_without_exposing_its_process_id() {
    let mut controller = configured_controller();

    let inventory = controller.inventory().expect("process inventory");
    let target = inventory
        .targets
        .iter()
        .find(|target| target.token == "target:1")
        .expect("the running contract process should be discovered");

    assert_eq!(target.operating_system, "windows");
    assert!(["x86", "x86_64"].contains(&target.architecture.as_str()));
    assert!(!target
        .display_name
        .contains(&std::process::id().to_string()));
    assert_ne!(target.token, format!("process:{}", std::process::id()));
}

#[test]
fn ctl_windows_002_returns_only_configured_adapter_features_for_a_discovered_target() {
    let mut controller = configured_controller();
    let inventory = controller.inventory().expect("process inventory");

    let recipe = controller
        .prepare(&inventory.targets[0].token, &[WireFeature::FontSubstitute])
        .expect("known target should prepare a recipe");

    assert_eq!(recipe.adapters.len(), 1);
    assert_eq!(
        recipe.adapters[0].features,
        vec![WireFeature::FontSubstitute]
    );
}

#[test]
fn ctl_windows_003_rejects_paths_and_unknown_target_tokens() {
    let mut controller = WindowsController::new();
    assert!(controller
        .configure(
            "org.example.windows-target",
            &WireControllerConfiguration {
                executable_names: Vec::new(),
                executable_paths: vec!["relative/program.exe".into()],
                descendant_executable_names: Vec::new(),
                adapter_requirements: Vec::new(),
            },
        )
        .is_err());
    assert!(controller
        .configure(
            "org.example.windows-target",
            &WireControllerConfiguration {
                executable_names: vec!["folder/program.exe".into()],
                executable_paths: Vec::new(),
                descendant_executable_names: Vec::new(),
                adapter_requirements: Vec::new(),
            },
        )
        .is_err());
    assert!(configured_controller()
        .prepare("target:missing", &[WireFeature::TextReplace])
        .is_err());
}

#[test]
fn ctl_windows_004_prefers_the_full_executable_path_over_an_ambiguous_name() {
    let current_executable = std::env::current_exe().expect("test executable should have a path");
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.windows-target",
            &WireControllerConfiguration {
                executable_names: vec!["intentionally-wrong-name.exe".into()],
                executable_paths: vec![current_executable.to_string_lossy().into_owned()],
                descendant_executable_names: Vec::new(),
                adapter_requirements: Vec::new(),
            },
        )
        .expect("an absolute executable path should configure discovery");

    let inventory = controller.inventory().expect("process inventory");

    assert_eq!(inventory.targets.len(), 1);
    assert_eq!(inventory.targets[0].token, "target:1");
}

#[test]
fn ctl_windows_005_mints_a_process_instance_grant_only_for_an_authorized_target() {
    let mut controller = configured_controller();
    let inventory = controller.inventory().expect("process inventory");
    let target = inventory.targets.first().expect("authorized target");

    let grant = controller
        .authorize_worker_target(&target.token)
        .expect("worker target grant");
    assert_eq!(grant.platform, "windows-process-v1");
    let (process_id, started_at) = grant
        .payload
        .split_once(':')
        .expect("process instance payload");
    assert_eq!(process_id.parse::<u32>(), Ok(std::process::id()));
    assert!(started_at.parse::<u64>().is_ok_and(|value| value > 0));
    assert!(controller
        .authorize_worker_target("target:missing")
        .is_err());
}

#[test]
#[ignore = "requires an explicitly authorized local executable and descendant allowlist"]
fn ctl_windows_006_discovers_an_authorized_process_family_without_exposing_process_ids() {
    let executable = std::env::var_os("GLYPHSHIFT_REAL_HOST_EXECUTABLE")
        .map(std::path::PathBuf::from)
        .expect("authorized executable path");
    let descendant_executable_names = std::env::var("GLYPHSHIFT_REAL_HOST_DESCENDANTS")
        .expect("authorized descendant executable names")
        .split(';')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!descendant_executable_names.is_empty());
    let executable_name = executable
        .file_name()
        .expect("authorized executable name")
        .to_string_lossy()
        .into_owned();
    let mut controller = WindowsController::new();
    controller
        .configure(
            "org.example.authorized-process-family",
            &WireControllerConfiguration {
                executable_names: vec![executable_name],
                executable_paths: vec![executable.to_string_lossy().into_owned()],
                descendant_executable_names,
                adapter_requirements: Vec::new(),
            },
        )
        .expect("authorized process family configuration");

    let inventory = controller
        .inventory()
        .expect("authorized process family inventory");

    assert!(inventory.targets.len() > 1);
    assert_eq!(
        inventory
            .targets
            .iter()
            .map(|target| target.token.as_str())
            .collect::<Vec<_>>(),
        (1..=inventory.targets.len())
            .map(|index| format!("target:{index}"))
            .collect::<Vec<_>>()
    );
    eprintln!(
        "authorized process family discovered {} target instances",
        inventory.targets.len()
    );
}
