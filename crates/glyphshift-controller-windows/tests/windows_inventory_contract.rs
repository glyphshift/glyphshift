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
                adapter_requirements: Vec::new(),
            },
        )
        .expect("an absolute executable path should configure discovery");

    let inventory = controller.inventory().expect("process inventory");

    assert_eq!(inventory.targets.len(), 1);
    assert_eq!(inventory.targets[0].token, "target:1");
}
