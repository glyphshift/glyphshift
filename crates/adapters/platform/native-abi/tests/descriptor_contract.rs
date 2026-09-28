use glyphshift_adapter_native_abi::{
    NativeAdapterDescriptorV1, NativeAdapterPrepareCommandV1, ARCH_ARM64, FEATURE_TEXT_OBSERVE,
    FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, PREPARE_ACTION_CANCEL, PREPARE_ACTION_POLL,
    PREPARE_ACTION_REQUEST, PREPARE_STATUS_CANCELLED, PREPARE_STATUS_FAILED,
    PREPARE_STATUS_PENDING, PREPARE_STATUS_READY, PREPARE_STATUS_RUNNING,
    PREPARE_STATUS_UNAVAILABLE, PREPARE_STATUS_UNKNOWN, PREPARE_STATUS_UNSUPPORTED,
    PREPARE_SYMBOL_V1,
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

#[test]
fn native_font_scale_bits_are_bounded_and_preserve_other_decisions() {
    use glyphshift_adapter_native_abi::{
        font_scale_bits, font_scale_percent, DECISION_TEXT_REPLACE,
    };
    for percent in [50, 75, 100, 125, 150, 200] {
        assert_eq!(
            font_scale_percent(font_scale_bits(percent) | DECISION_TEXT_REPLACE),
            percent
        );
    }
    for percent in [0, 49, 201, u16::MAX] {
        assert_eq!(font_scale_percent(font_scale_bits(percent)), 100);
    }
}

#[test]
fn native_prepare_v1_layout_and_state_values_are_stable() {
    assert_eq!(PREPARE_SYMBOL_V1, b"glyphshift_adapter_prepare_v1\0");
    assert_eq!(std::mem::size_of::<NativeAdapterPrepareCommandV1>(), 20);
    assert_eq!(std::mem::align_of::<NativeAdapterPrepareCommandV1>(), 4);
    assert_eq!(
        (
            PREPARE_ACTION_REQUEST,
            PREPARE_ACTION_POLL,
            PREPARE_ACTION_CANCEL
        ),
        (1, 2, 3)
    );
    assert_eq!(PREPARE_STATUS_UNKNOWN, -1);
    assert_eq!(PREPARE_STATUS_FAILED, -4);
    assert_eq!(PREPARE_STATUS_UNSUPPORTED, -7);
    assert_eq!(PREPARE_STATUS_UNAVAILABLE, -8);
    assert_eq!(PREPARE_STATUS_PENDING, 0);
    assert_eq!(PREPARE_STATUS_READY, 1);
    assert_eq!(PREPARE_STATUS_CANCELLED, 3);
    assert_eq!(PREPARE_STATUS_RUNNING, 4);
}
