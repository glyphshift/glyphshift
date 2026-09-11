//! Synthetic native Adapter used to verify the target Runtime lifecycle seam.

use glyphshift_adapter_native_abi::{
    NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeNegotiationV1, NativeRuntimeHostV1,
    ARCH_X86_64, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS, STATUS_INVALID_HOST, STATUS_OK,
    STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use glyphshift_adapter_sdk::AdapterDescriptor;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

pub const ADAPTER_ID: &str = "example.synthetic.native-refresh";
const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_REPLACE;

static DEACTIVATE_STATUS: AtomicI32 = AtomicI32::new(STATUS_OK);

static REFRESH_COUNT: AtomicU32 = AtomicU32::new(0);

#[must_use]
pub fn descriptor() -> AdapterDescriptor {
    NativeAdapterDescriptorV1::inline_target(
        ADAPTER_ID,
        (1, 0, 0),
        SUPPORTED_FEATURES,
        PLATFORM_WINDOWS,
        ARCH_X86_64,
    )
    .to_descriptor()
    .expect("synthetic native descriptor")
}

extern "C" fn negotiate_features(requested: u64, granted: u64) -> NativeNegotiationV1 {
    let status = if requested & !SUPPORTED_FEATURES != 0 {
        STATUS_UNSUPPORTED_FEATURE
    } else if requested & !granted != 0 {
        STATUS_UNAUTHORIZED_FEATURE
    } else {
        STATUS_OK
    };
    NativeNegotiationV1 {
        status,
        active_feature_bits: if status == STATUS_OK { requested } else { 0 },
    }
}

extern "C" fn activate(
    host: *const NativeRuntimeHostV1,
    requested: u64,
    granted: u64,
) -> NativeNegotiationV1 {
    if host.is_null()
        || unsafe { (*host).struct_size } != std::mem::size_of::<NativeRuntimeHostV1>() as u32
    {
        return NativeNegotiationV1 {
            status: STATUS_INVALID_HOST,
            active_feature_bits: 0,
        };
    }
    negotiate_features(requested, granted)
}

extern "C" fn deactivate() -> i32 {
    DEACTIVATE_STATUS.load(Ordering::Acquire)
}

#[no_mangle]
pub extern "C" fn glyphshift_test_set_deactivate_status_v1(status: i32) {
    DEACTIVATE_STATUS.store(status, Ordering::Release);
}

extern "C" fn request_refresh() {
    REFRESH_COUNT.fetch_add(1, Ordering::AcqRel);
}

#[no_mangle]
pub extern "C" fn glyphshift_test_refresh_count_v1() -> u32 {
    REFRESH_COUNT.load(Ordering::Acquire)
}

#[no_mangle]
pub extern "C" fn glyphshift_adapter_entry_v1() -> NativeAdapterApiV1 {
    NativeAdapterApiV1 {
        struct_size: std::mem::size_of::<NativeAdapterApiV1>() as u32,
        descriptor: NativeAdapterDescriptorV1::inline_target(
            ADAPTER_ID,
            (1, 0, 0),
            SUPPORTED_FEATURES,
            PLATFORM_WINDOWS,
            ARCH_X86_64,
        ),
        negotiate_features,
        activate,
        deactivate,
        request_refresh,
    }
}
