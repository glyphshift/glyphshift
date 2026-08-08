use glyphshift_adapter_native_abi::{
    NativeAdapterDescriptorV1, ARCH_ARM64, FEATURE_TEXT_OBSERVE, FEATURE_TEXT_REPLACE,
    PLATFORM_WINDOWS,
};
use glyphshift_domain::{ApplyModel, Feature, Placement};

#[test]
fn native_descriptor_preserves_platform_and_arm64_machine_facts() {
    let descriptor = NativeAdapterDescriptorV1::inline_target(
        "example.synthetic.arm64",
        (1, 0, 0),
        FEATURE_TEXT_REPLACE,
        PLATFORM_WINDOWS,
        ARCH_ARM64,
    )
    .to_descriptor()
    .expect("known machine bits should produce a descriptor");

    assert_eq!(descriptor.platforms().collect::<Vec<_>>(), ["windows"]);
    assert_eq!(descriptor.architectures().collect::<Vec<_>>(), ["aarch64"]);
}

#[test]
fn native_descriptor_can_declare_a_target_process_observer() {
    let descriptor = NativeAdapterDescriptorV1::observe_target(
        "example.synthetic.observer",
        (1, 0, 0),
        FEATURE_TEXT_OBSERVE,
        PLATFORM_WINDOWS,
        ARCH_ARM64,
    )
    .to_descriptor()
    .expect("observe-only target descriptor");

    assert_eq!(descriptor.apply_model(), ApplyModel::ObserveOnly);
    assert_eq!(descriptor.placement(), Placement::TargetProcess);
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        [Feature::TextObserve]
    );
}

#[test]
fn native_descriptor_can_declare_retained_target_replacement() {
    let descriptor = NativeAdapterDescriptorV1::retained_target(
        "example.synthetic.retained",
        (1, 0, 0),
        FEATURE_TEXT_OBSERVE | FEATURE_TEXT_REPLACE,
        PLATFORM_WINDOWS,
        ARCH_ARM64,
    )
    .to_descriptor()
    .expect("retained target descriptor");

    assert_eq!(descriptor.apply_model(), ApplyModel::RetainedObject);
    assert_eq!(descriptor.placement(), Placement::TargetProcess);
    assert_eq!(
        descriptor.features().collect::<Vec<_>>(),
        [Feature::TextObserve, Feature::TextReplace]
    );
}
