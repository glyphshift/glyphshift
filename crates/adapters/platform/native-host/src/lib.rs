//! Dynamic loader for trusted target-process Adapter packages.

mod metadata;
pub use metadata::{inspect_pe_architecture, NativeAdapterMetadata};

use glyphshift_adapter_native_abi::{
    descriptor_matches, feature_bits, NativeAbiError, NativeAdapterApiV1, NativeAdapterEntryV1,
    NativeRuntimeHostV1, ENTRY_SYMBOL_V1, STATUS_OK, STATUS_UNAUTHORIZED_FEATURE,
    STATUS_UNSUPPORTED_FEATURE,
};
use glyphshift_adapter_sdk::AdapterDescriptor;
use glyphshift_domain::{Feature, SourceTextPolicy};
use libloading::Library;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHostError {
    LoadFailed,
    EntryMissing,
    ApiSizeMismatch,
    DescriptorSizeMismatch,
    InvalidDescriptor(NativeAbiError),
    DescriptorMismatch,
    UnsupportedFeature,
    UnauthorizedFeature,
    PackageFailure(i32),
    InvalidSourcePolicy,
}

pub struct LoadedNativeAdapter {
    api: NativeAdapterApiV1,
    descriptor: AdapterDescriptor,
    source_policy: SourceTextPolicy,
    _library: Library,
}

impl LoadedNativeAdapter {
    /// Reads the descriptor exported by a native package after its signer and content hash were
    /// verified.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `path` points to the exact trusted artifact accepted by the
    /// Adapter Registry. Loading native code can execute package initialization routines.
    pub unsafe fn inspect(path: &Path) -> Result<AdapterDescriptor, NativeHostError> {
        let (_, _, descriptor) = unsafe { open_package(path) }?;
        Ok(descriptor)
    }

    /// Same trust requirements as `inspect`; policy is optional metadata.
    ///
    /// # Safety
    /// The path must identify the exact trusted native artifact already verified by the caller.
    pub unsafe fn inspect_with_source_policy(
        path: &Path,
    ) -> Result<(AdapterDescriptor, SourceTextPolicy), NativeHostError> {
        let (library, _, descriptor) = unsafe { open_package(path) }?;
        Ok((descriptor, unsafe { read_source_policy(&library) }?))
    }

    /// Loads a native package only after its signer and content hash were verified.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `path` points to the exact trusted artifact accepted by the
    /// Adapter Registry. Loading native code can execute package initialization routines.
    pub unsafe fn load(path: &Path, expected: &AdapterDescriptor) -> Result<Self, NativeHostError> {
        let (library, api, descriptor) = unsafe { open_package(path) }?;
        if !descriptor_matches(&descriptor, expected) {
            return Err(NativeHostError::DescriptorMismatch);
        }
        Ok(Self {
            api,
            descriptor,
            source_policy: unsafe { read_source_policy(&library) }?,
            _library: library,
        })
    }

    #[must_use]
    pub const fn descriptor(&self) -> &AdapterDescriptor {
        &self.descriptor
    }

    pub const fn source_policy(&self) -> SourceTextPolicy {
        self.source_policy
    }

    pub fn negotiate(
        &self,
        requested: impl IntoIterator<Item = Feature>,
        granted: impl IntoIterator<Item = Feature>,
    ) -> Result<Vec<Feature>, NativeHostError> {
        let requested = requested.into_iter().collect::<Vec<_>>();
        let result = (self.api.negotiate_features)(
            feature_bits(requested.iter().copied()),
            feature_bits(granted),
        );
        match result.status {
            STATUS_OK => Ok(requested
                .into_iter()
                .filter(|feature| result.active_feature_bits & feature_bits([*feature]) != 0)
                .collect()),
            STATUS_UNSUPPORTED_FEATURE => Err(NativeHostError::UnsupportedFeature),
            STATUS_UNAUTHORIZED_FEATURE => Err(NativeHostError::UnauthorizedFeature),
            status => Err(NativeHostError::PackageFailure(status)),
        }
    }

    pub fn activate(
        &self,
        host: &'static NativeRuntimeHostV1,
        requested: impl IntoIterator<Item = Feature>,
        granted: impl IntoIterator<Item = Feature>,
    ) -> Result<Vec<Feature>, NativeHostError> {
        let requested = requested.into_iter().collect::<Vec<_>>();
        let result = (self.api.activate)(
            host,
            feature_bits(requested.iter().copied()),
            feature_bits(granted),
        );
        match result.status {
            STATUS_OK => Ok(requested
                .into_iter()
                .filter(|feature| result.active_feature_bits & feature_bits([*feature]) != 0)
                .collect()),
            STATUS_UNSUPPORTED_FEATURE => Err(NativeHostError::UnsupportedFeature),
            STATUS_UNAUTHORIZED_FEATURE => Err(NativeHostError::UnauthorizedFeature),
            status => Err(NativeHostError::PackageFailure(status)),
        }
    }

    pub fn deactivate(&self) -> Result<(), NativeHostError> {
        match (self.api.deactivate)() {
            STATUS_OK => Ok(()),
            status => Err(NativeHostError::PackageFailure(status)),
        }
    }

    /// Asks the package to enqueue any framework-specific invalidation needed after a Runtime
    /// lifecycle change. This is deliberately best-effort, like the host's generic redraw request.
    pub fn request_refresh(&self) {
        (self.api.request_refresh)();
    }
}

unsafe fn read_source_policy(library: &Library) -> Result<SourceTextPolicy, NativeHostError> {
    let Ok(policy) = (unsafe {
        library.get::<extern "C" fn() -> u32>(b"glyphshift_adapter_source_policy_v1\0")
    }) else {
        return Ok(SourceTextPolicy::Exact);
    };
    SourceTextPolicy::from_code(policy()).ok_or(NativeHostError::InvalidSourcePolicy)
}

unsafe fn open_package(
    path: &Path,
) -> Result<(Library, NativeAdapterApiV1, AdapterDescriptor), NativeHostError> {
    let library = unsafe { Library::new(path) }.map_err(|_| NativeHostError::LoadFailed)?;
    let api = {
        let entry = unsafe { library.get::<NativeAdapterEntryV1>(ENTRY_SYMBOL_V1) }
            .map_err(|_| NativeHostError::EntryMissing)?;
        entry()
    };
    if api.struct_size != std::mem::size_of::<NativeAdapterApiV1>() as u32 {
        return Err(NativeHostError::ApiSizeMismatch);
    }
    if api.descriptor.struct_size
        != std::mem::size_of::<glyphshift_adapter_native_abi::NativeAdapterDescriptorV1>() as u32
    {
        return Err(NativeHostError::DescriptorSizeMismatch);
    }
    let descriptor = api
        .descriptor
        .to_descriptor()
        .map_err(NativeHostError::InvalidDescriptor)?;
    Ok((library, api, descriptor))
}
