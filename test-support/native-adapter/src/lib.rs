//! Synthetic native Adapter used to verify the target Runtime lifecycle seam.

use glyphshift_adapter_native_abi::{
    NativeAdapterApiV1, NativeAdapterDescriptorV1, NativeAdapterPrepareCommandV1,
    NativeNegotiationV1, NativeRuntimeHostV1, ARCH_X86_64, FEATURE_TEXT_REPLACE, PLATFORM_WINDOWS,
    PREPARE_ACTION_CANCEL, PREPARE_ACTION_POLL, PREPARE_ACTION_REQUEST, PREPARE_STATUS_CANCELLED,
    PREPARE_STATUS_FAILED, PREPARE_STATUS_PENDING, PREPARE_STATUS_READY, PREPARE_STATUS_UNKNOWN,
    PREPARE_STATUS_UNSUPPORTED, STATUS_ACTIVATION_FAILED, STATUS_INVALID_HOST, STATUS_OK,
    STATUS_UNAUTHORIZED_FEATURE, STATUS_UNSUPPORTED_FEATURE,
};
use glyphshift_adapter_sdk::AdapterDescriptor;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

pub const ADAPTER_ID: &str = "example.synthetic.native-refresh";
const SUPPORTED_FEATURES: u64 = FEATURE_TEXT_REPLACE;
const PREPARE_REQUEST_ID: u32 = 1;
const PREPARE_FAIL_ENV: &str = "GLYPHSHIFT_TEST_NATIVE_ADAPTER_PREPARE_FAIL";
const PREPARE_PENDING_ENV: &str = "GLYPHSHIFT_TEST_NATIVE_ADAPTER_PREPARE_PENDING";
const PREPARE_UNSUPPORTED_ENV: &str = "GLYPHSHIFT_TEST_NATIVE_ADAPTER_PREPARE_UNSUPPORTED";
const REQUIRE_PREPARE_ENV: &str = "GLYPHSHIFT_TEST_NATIVE_ADAPTER_REQUIRE_PREPARE";
const ACTIVATE_FAIL_ENV: &str = "GLYPHSHIFT_TEST_NATIVE_ADAPTER_ACTIVATE_FAIL";

static DEACTIVATE_STATUS: AtomicI32 = AtomicI32::new(STATUS_OK);
static PREPARED: AtomicBool = AtomicBool::new(false);

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
    if std::env::var_os(ACTIVATE_FAIL_ENV).is_some()
        || (std::env::var_os(REQUIRE_PREPARE_ENV).is_some() && !PREPARED.load(Ordering::Acquire))
    {
        return NativeNegotiationV1 {
            status: STATUS_ACTIVATION_FAILED,
            active_feature_bits: 0,
        };
    }
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
pub unsafe extern "system" fn glyphshift_adapter_prepare_v1(
    command: *mut NativeAdapterPrepareCommandV1,
) -> u32 {
    let Some(command) = command.as_mut() else {
        return 87;
    };
    if command.struct_size != std::mem::size_of::<NativeAdapterPrepareCommandV1>() as u32 {
        return 87;
    }
    match command.action {
        PREPARE_ACTION_REQUEST => {
            command.request_id = PREPARE_REQUEST_ID;
            command.status = if std::env::var_os(PREPARE_UNSUPPORTED_ENV).is_some() {
                PREPARED.store(false, Ordering::Release);
                PREPARE_STATUS_UNSUPPORTED
            } else if std::env::var_os(PREPARE_FAIL_ENV).is_some() {
                PREPARED.store(false, Ordering::Release);
                PREPARE_STATUS_FAILED
            } else if std::env::var_os(PREPARE_PENDING_ENV).is_some() {
                PREPARED.store(false, Ordering::Release);
                PREPARE_STATUS_PENDING
            } else {
                PREPARED.store(true, Ordering::Release);
                PREPARE_STATUS_READY
            };
        }
        PREPARE_ACTION_POLL => {
            if command.request_id != PREPARE_REQUEST_ID {
                command.status = PREPARE_STATUS_UNKNOWN;
            } else if std::env::var_os(PREPARE_PENDING_ENV).is_some() {
                PREPARED.store(true, Ordering::Release);
                command.status = PREPARE_STATUS_READY;
            }
        }
        PREPARE_ACTION_CANCEL => {
            if command.request_id != PREPARE_REQUEST_ID {
                command.status = PREPARE_STATUS_UNKNOWN;
            } else {
                PREPARED.store(false, Ordering::Release);
                command.status = PREPARE_STATUS_CANCELLED;
            }
        }
        _ => return 87,
    }
    0
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
