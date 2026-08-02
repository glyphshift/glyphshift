use glyphshift_adapter_native_abi::{
    NativeAdapterDescriptorV1, ARCH_ARM64, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS,
};

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
