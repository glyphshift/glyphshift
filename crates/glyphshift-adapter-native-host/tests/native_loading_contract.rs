#![cfg(windows)]

use glyphshift_adapter_native_host::{LoadedNativeAdapter, NativeHostError};
use glyphshift_domain::Feature;
use std::path::PathBuf;

fn native_package(name: &str) -> PathBuf {
    let executable = std::env::current_exe().expect("current test executable");
    let profile = executable
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo profile directory");
    profile.join(format!("{name}.dll"))
}

#[test]
fn native_001_loads_and_negotiates_two_independent_adapter_packages() {
    let cases = [
        (
            native_package("glyphshift_adapter_gdi_native"),
            glyphshift_adapter_gdi::descriptor(),
        ),
        (
            native_package("glyphshift_adapter_gdiplus_native"),
            glyphshift_adapter_gdiplus::descriptor(),
        ),
    ];

    for (path, expected) in cases {
        let package = unsafe {
            LoadedNativeAdapter::load(&path, &expected).expect("load verified native package")
        };
        assert_eq!(package.descriptor().adapter_id(), expected.adapter_id());
        assert_eq!(
            package
                .negotiate(
                    [Feature::TextReplace],
                    [Feature::TextReplace, Feature::FontSubstitute],
                )
                .expect("authorized negotiation"),
            [Feature::TextReplace]
        );
        assert_eq!(
            package.negotiate([Feature::FontSubstitute], [Feature::TextReplace]),
            Err(NativeHostError::UnauthorizedFeature)
        );
        assert_eq!(
            package.negotiate([Feature::LayoutAdjust], [Feature::LayoutAdjust]),
            Err(NativeHostError::UnsupportedFeature)
        );
        package.deactivate().expect("deactivate package");
    }
}

#[test]
fn native_002_inspects_a_trusted_package_without_a_product_side_id_branch() {
    let path = native_package("glyphshift_adapter_gdi_native");
    let descriptor = unsafe {
        LoadedNativeAdapter::inspect(&path).expect("inspect verified native package descriptor")
    };

    assert_eq!(descriptor, glyphshift_adapter_gdi::descriptor());
}

#[test]
fn native_003_rejects_a_package_whose_loaded_descriptor_differs_from_registry_evidence() {
    let path = native_package("glyphshift_adapter_gdi_native");
    let wrong_descriptor = glyphshift_adapter_gdiplus::descriptor();
    let result = unsafe { LoadedNativeAdapter::load(&path, &wrong_descriptor) };
    assert!(matches!(result, Err(NativeHostError::DescriptorMismatch)));
}
