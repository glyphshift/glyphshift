//! Stable C ABI values shared by target-process Adapter packages and their loader.

use glyphshift_adapter_sdk::{AdapterDescriptor, AdapterVersion};
use glyphshift_domain::{AbiVersion, AdapterId, ApplyModel, Feature, Placement};

pub const ENTRY_SYMBOL_V1: &[u8] = b"glyphshift_adapter_entry_v1\0";
pub const NATIVE_ABI_V1: AbiVersion = AbiVersion::new(1, 0);

pub const FEATURE_TEXT_OBSERVE: u64 = 1 << 0;
pub const FEATURE_TEXT_REPLACE: u64 = 1 << 1;
pub const FEATURE_FONT_SUBSTITUTE: u64 = 1 << 2;
pub const FEATURE_LAYOUT_ADJUST: u64 = 1 << 3;
pub const FEATURE_RESOURCE_REPLACE: u64 = 1 << 4;

pub const ARCH_X86: u32 = 1 << 0;
pub const ARCH_X86_64: u32 = 1 << 1;
pub const ARCH_ARM64: u32 = 1 << 2;

pub const PLATFORM_WINDOWS: u32 = 1 << 0;
pub const PLATFORM_MACOS: u32 = 1 << 1;

pub const STATUS_OK: i32 = 0;
pub const STATUS_UNSUPPORTED_FEATURE: i32 = 1;
pub const STATUS_UNAUTHORIZED_FEATURE: i32 = 2;
pub const STATUS_ACTIVATION_FAILED: i32 = 3;
pub const STATUS_INVALID_HOST: i32 = 4;
pub const STATUS_OUTPUT_TOO_SMALL: i32 = 5;

pub const DECISION_TEXT_REPLACE: u32 = 1 << 0;
pub const DECISION_FONT_SUBSTITUTE: u32 = 1 << 1;

const APPLY_MODEL_INLINE_RENDER: u32 = 1;
const APPLY_MODEL_RETAINED_OBJECT: u32 = 2;
const APPLY_MODEL_EXTERNAL_PROTOCOL: u32 = 3;
const APPLY_MODEL_OBSERVE_ONLY: u32 = 4;
const PLACEMENT_TARGET_PROCESS: u32 = 1;
const PLACEMENT_ISOLATED_WORKER: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedUtf8 {
    len: u16,
    bytes: [u8; 96],
}

impl FixedUtf8 {
    #[must_use]
    pub const fn new(value: &str) -> Self {
        let source = value.as_bytes();
        assert!(source.len() <= 96, "native ABI string exceeds 96 bytes");
        let mut bytes = [0; 96];
        let mut index = 0;
        while index < source.len() {
            bytes[index] = source[index];
            index += 1;
        }
        Self {
            len: source.len() as u16,
            bytes,
        }
    }

    pub fn as_str(&self) -> Result<&str, NativeAbiError> {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .map_err(|_| NativeAbiError::InvalidUtf8)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeAdapterDescriptorV1 {
    pub struct_size: u32,
    pub adapter_id: FixedUtf8,
    pub version_major: u16,
    pub version_minor: u16,
    pub version_patch: u16,
    pub abi_major: u16,
    pub abi_minor: u16,
    pub apply_model: u32,
    pub placement: u32,
    pub feature_bits: u64,
    pub platform_bits: u32,
    pub architecture_bits: u32,
}

impl NativeAdapterDescriptorV1 {
    #[must_use]
    pub const fn inline_target(
        adapter_id: &'static str,
        version: (u16, u16, u16),
        feature_bits: u64,
        platform_bits: u32,
        architecture_bits: u32,
    ) -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            adapter_id: FixedUtf8::new(adapter_id),
            version_major: version.0,
            version_minor: version.1,
            version_patch: version.2,
            abi_major: 1,
            abi_minor: 0,
            apply_model: APPLY_MODEL_INLINE_RENDER,
            placement: PLACEMENT_TARGET_PROCESS,
            feature_bits,
            platform_bits,
            architecture_bits,
        }
    }

    #[must_use]
    pub const fn retained_target(
        adapter_id: &'static str,
        version: (u16, u16, u16),
        feature_bits: u64,
        platform_bits: u32,
        architecture_bits: u32,
    ) -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            adapter_id: FixedUtf8::new(adapter_id),
            version_major: version.0,
            version_minor: version.1,
            version_patch: version.2,
            abi_major: 1,
            abi_minor: 0,
            apply_model: APPLY_MODEL_RETAINED_OBJECT,
            placement: PLACEMENT_TARGET_PROCESS,
            feature_bits,
            platform_bits,
            architecture_bits,
        }
    }

    #[must_use]
    pub const fn observe_target(
        adapter_id: &'static str,
        version: (u16, u16, u16),
        feature_bits: u64,
        platform_bits: u32,
        architecture_bits: u32,
    ) -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            adapter_id: FixedUtf8::new(adapter_id),
            version_major: version.0,
            version_minor: version.1,
            version_patch: version.2,
            abi_major: 1,
            abi_minor: 0,
            apply_model: APPLY_MODEL_OBSERVE_ONLY,
            placement: PLACEMENT_TARGET_PROCESS,
            feature_bits,
            platform_bits,
            architecture_bits,
        }
    }

    pub fn to_descriptor(self) -> Result<AdapterDescriptor, NativeAbiError> {
        let apply_model = match self.apply_model {
            APPLY_MODEL_INLINE_RENDER => ApplyModel::InlineRender,
            APPLY_MODEL_RETAINED_OBJECT => ApplyModel::RetainedObject,
            APPLY_MODEL_EXTERNAL_PROTOCOL => ApplyModel::ExternalProtocol,
            APPLY_MODEL_OBSERVE_ONLY => ApplyModel::ObserveOnly,
            value => return Err(NativeAbiError::UnknownApplyModel(value)),
        };
        let placement = match self.placement {
            PLACEMENT_TARGET_PROCESS => Placement::TargetProcess,
            PLACEMENT_ISOLATED_WORKER => Placement::IsolatedWorker,
            value => return Err(NativeAbiError::UnknownPlacement(value)),
        };
        let descriptor = AdapterDescriptor::new(
            AdapterId::new(self.adapter_id.as_str()?),
            AdapterVersion::new(self.version_major, self.version_minor, self.version_patch),
            apply_model,
            placement,
            features_from_bits(self.feature_bits)?,
        )
        .with_platforms(platforms_from_bits(self.platform_bits)?)
        .with_architectures(architectures_from_bits(self.architecture_bits)?)
        .with_abi(AbiVersion::new(self.abi_major, self.abi_minor));
        Ok(descriptor)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeNegotiationV1 {
    pub status: i32,
    pub active_feature_bits: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeDecisionV1 {
    pub status: i32,
    pub generation: u64,
    pub decision_bits: u32,
    pub text_len: u32,
    pub font_len: u32,
}

pub type DecideUtf16V1 = extern "C" fn(
    context: *mut core::ffi::c_void,
    source: *const u16,
    source_len: u32,
    text_out: *mut u16,
    text_capacity: u32,
    font_out: *mut u16,
    font_capacity: u32,
) -> NativeDecisionV1;
pub type SourceCharactersUtf16V1 =
    extern "C" fn(context: *mut core::ffi::c_void, output: *mut u16, capacity: u32) -> u32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeRuntimeHostV1 {
    pub struct_size: u32,
    pub context: *mut core::ffi::c_void,
    pub decide_utf16: DecideUtf16V1,
    pub source_characters_utf16: SourceCharactersUtf16V1,
}

pub type NegotiateFeaturesV1 = extern "C" fn(u64, u64) -> NativeNegotiationV1;
pub type ActivateV1 = extern "C" fn(*const NativeRuntimeHostV1, u64, u64) -> NativeNegotiationV1;
pub type DeactivateV1 = extern "C" fn() -> i32;
/// Requests that framework-owned visual state be invalidated asynchronously.
///
/// The callback can run on a Runtime control thread. Implementations must only enqueue or request
/// work that is safe for the framework's owning thread; they must not synchronously repaint.
pub type RequestRefreshV1 = extern "C" fn();

pub extern "C" fn request_refresh_noop() {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeAdapterApiV1 {
    pub struct_size: u32,
    pub descriptor: NativeAdapterDescriptorV1,
    pub negotiate_features: NegotiateFeaturesV1,
    pub activate: ActivateV1,
    pub deactivate: DeactivateV1,
    pub request_refresh: RequestRefreshV1,
}

pub type NativeAdapterEntryV1 = extern "C" fn() -> NativeAdapterApiV1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAbiError {
    InvalidUtf8,
    UnknownApplyModel(u32),
    UnknownPlacement(u32),
    UnknownFeatureBits(u64),
    UnknownPlatformBits(u32),
    UnknownArchitectureBits(u32),
}

#[must_use]
pub fn feature_bits(features: impl IntoIterator<Item = Feature>) -> u64 {
    features.into_iter().fold(0, |bits, feature| {
        bits | match feature {
            Feature::TextObserve => FEATURE_TEXT_OBSERVE,
            Feature::TextReplace => FEATURE_TEXT_REPLACE,
            Feature::FontSubstitute => FEATURE_FONT_SUBSTITUTE,
            Feature::LayoutAdjust => FEATURE_LAYOUT_ADJUST,
            Feature::ResourceReplace => FEATURE_RESOURCE_REPLACE,
        }
    })
}

fn features_from_bits(bits: u64) -> Result<Vec<Feature>, NativeAbiError> {
    let known = FEATURE_TEXT_OBSERVE
        | FEATURE_TEXT_REPLACE
        | FEATURE_FONT_SUBSTITUTE
        | FEATURE_LAYOUT_ADJUST
        | FEATURE_RESOURCE_REPLACE;
    if bits & !known != 0 {
        return Err(NativeAbiError::UnknownFeatureBits(bits & !known));
    }
    let candidates = [
        (FEATURE_TEXT_OBSERVE, Feature::TextObserve),
        (FEATURE_TEXT_REPLACE, Feature::TextReplace),
        (FEATURE_FONT_SUBSTITUTE, Feature::FontSubstitute),
        (FEATURE_LAYOUT_ADJUST, Feature::LayoutAdjust),
        (FEATURE_RESOURCE_REPLACE, Feature::ResourceReplace),
    ];
    Ok(candidates
        .into_iter()
        .filter_map(|(flag, feature)| (bits & flag != 0).then_some(feature))
        .collect())
}

fn architectures_from_bits(bits: u32) -> Result<Vec<&'static str>, NativeAbiError> {
    let known = ARCH_X86 | ARCH_X86_64 | ARCH_ARM64;
    if bits & !known != 0 {
        return Err(NativeAbiError::UnknownArchitectureBits(bits & !known));
    }
    let mut architectures = Vec::new();
    if bits & ARCH_X86 != 0 {
        architectures.push("x86");
    }
    if bits & ARCH_X86_64 != 0 {
        architectures.push("x86_64");
    }
    if bits & ARCH_ARM64 != 0 {
        architectures.push("aarch64");
    }
    Ok(architectures)
}

fn platforms_from_bits(bits: u32) -> Result<Vec<&'static str>, NativeAbiError> {
    let known = PLATFORM_WINDOWS | PLATFORM_MACOS;
    if bits & !known != 0 {
        return Err(NativeAbiError::UnknownPlatformBits(bits & !known));
    }
    let mut platforms = Vec::new();
    if bits & PLATFORM_WINDOWS != 0 {
        platforms.push("windows");
    }
    if bits & PLATFORM_MACOS != 0 {
        platforms.push("macos");
    }
    Ok(platforms)
}

#[must_use]
pub fn descriptor_matches(actual: &AdapterDescriptor, expected: &AdapterDescriptor) -> bool {
    actual.adapter_id() == expected.adapter_id()
        && actual.version() == expected.version()
        && actual.apply_model() == expected.apply_model()
        && actual.placement() == expected.placement()
        && actual.features().eq(expected.features())
        && actual.platforms().eq(expected.platforms())
        && actual.architectures().eq(expected.architectures())
        && actual.abi() == expected.abi()
}
