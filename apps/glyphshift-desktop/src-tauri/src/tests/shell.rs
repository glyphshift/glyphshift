use super::*;

#[test]
fn elevated_restart_preserves_explicit_desktop_roots() {
    let context = DesktopLaunchContext {
        data_root: Some(PathBuf::from("<synthetic-data-root>")),
        runtime_root: Some(PathBuf::from("<synthetic-runtime-root>")),
    };

    let restored = DesktopLaunchContext::from_args(context.elevation_arguments());

    assert_eq!(restored, context);
}

#[test]
fn reports_the_embedded_shell_only_when_the_command_is_reachable() {
    let status = desktop_status();

    assert!(status.shell_ready);
    assert_eq!(status.product_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(status.api_version, DESKTOP_API_VERSION);
}

#[test]
fn product_catalog_excludes_observe_only_adapters() {
    assert!(!product_adapter_enabled([Feature::TextObserve]));
    assert!(product_adapter_enabled([
        Feature::TextObserve,
        Feature::TextReplace,
    ]));
}
