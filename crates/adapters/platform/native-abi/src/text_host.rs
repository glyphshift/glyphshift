//! Optional text-evidence extension. NativeRuntimeHostV1 retains its original size.
use super::{NativeDecisionV1, NativeRuntimeHostV1};
use core::ffi::c_void;

pub const TEXT_HOST_BIND_SYMBOL_V1: &[u8] = b"glyphshift_adapter_bind_text_host_v1\0";
pub const TEXT_HOST_VERSION_V1: u32 = 1;
pub const TEXT_EVENT_DRAW: u32 = 1;
pub const TEXT_EVENT_RETAINED: u32 = 2;
pub const TEXT_EVENT_OBSERVE: u32 = 3;
pub const TEXT_EVENT_FRAGMENT: u32 = 4;
pub const TEXT_EVENT_FINISH: u32 = 5;
pub const TEXT_EVENT_CANCEL: u32 = 6;
pub const TEXT_EVENT_GLYPH_RASTER: u32 = 7;
pub const TEXT_EVENT_METADATA_VERSION_V1: u32 = 1;
pub const TEXT_EVENT_METADATA_CONTEXT: u32 = 1 << 0;
pub const TEXT_EVENT_METADATA_DISAMBIGUATION: u32 = 1 << 1;
pub const TEXT_EVENT_METADATA_PLURAL_N: u32 = 1 << 2;
pub const TEXT_EVENT_METADATA_KNOWN_FLAGS: u32 = TEXT_EVENT_METADATA_CONTEXT
    | TEXT_EVENT_METADATA_DISAMBIGUATION
    | TEXT_EVENT_METADATA_PLURAL_N;
pub const MAX_TEXT_EVENT_CONTEXT_UNITS: usize = 1024;
pub const STATUS_INVALID_TEXT_EVENT: i32 = 6;
pub type EnterTextScopeV1 = extern "C" fn(*mut c_void) -> u64;
pub type LeaveTextScopeV1 = extern "C" fn(*mut c_void, u64);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeTextEventV1 {
    pub struct_size: u32,
    pub kind: u32,
    pub surface: u64,
    pub run: u64,
    pub epoch: u64,
    /// Fragment ordinal, or expected part count for Finish.
    pub ordinal: u32,
    pub source: *const u16,
    pub source_len: u32,
}

/// Versioned extension of [`NativeTextEventV1`] for framework translation calls.
///
/// `base` is the exact V1 prefix and `base.struct_size` identifies this V2 layout. The host
/// callback signature remains V1 so old producers and V2 producers can share one binding.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeTextEventV2 {
    pub base: NativeTextEventV1,
    pub metadata_version: u32,
    pub metadata_flags: u32,
    pub context: *const u16,
    pub context_len: u32,
    pub disambiguation: *const u16,
    pub disambiguation_len: u32,
    pub plural_n: i32,
    pub reserved: u32,
}

impl NativeTextEventV1 {
    #[must_use]
    pub fn complete_draw(source: &[u16]) -> Self {
        Self {
            struct_size: size_of::<Self>() as u32,
            kind: TEXT_EVENT_DRAW,
            surface: 0,
            run: 0,
            epoch: 0,
            ordinal: 0,
            source: source.as_ptr(),
            source_len: u32::try_from(source.len()).unwrap_or(u32::MAX),
        }
    }
}

pub type DecideTextV1 = extern "C" fn(
    context: *mut c_void,
    event: *const NativeTextEventV1,
    text_out: *mut u16,
    text_capacity: u32,
    font_out: *mut u16,
    font_capacity: u32,
) -> NativeDecisionV1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeTextHostV1 {
    pub struct_size: u32,
    pub version: u32,
    pub context: *mut c_void,
    pub decide_text: DecideTextV1,
    pub enter_scope: Option<EnterTextScopeV1>,
    pub leave_scope: Option<LeaveTextScopeV1>,
}

/// A null argument clears a previous extension binding (legacy host compatibility).
/// A non-null host must remain alive for every Adapter callback using the binding.
pub type BindTextHostV1 = unsafe extern "C" fn(*const NativeTextHostV1) -> i32;

/// Copyable, thread-shareable view; ownership stays with the supervising Runtime.
#[derive(Clone, Copy)]
pub struct NativeTextHostBinding {
    context: usize,
    decide: DecideTextV1,
    enter_scope: Option<EnterTextScopeV1>,
    leave_scope: Option<LeaveTextScopeV1>,
}

impl NativeTextHostBinding {
    #[must_use]
    pub fn new(host: NativeTextHostV1) -> Option<Self> {
        (host.struct_size == size_of::<NativeTextHostV1>() as u32
            && host.version == TEXT_HOST_VERSION_V1
            && host.enter_scope.is_some() == host.leave_scope.is_some())
        .then_some(Self {
            context: host.context as usize,
            decide: host.decide_text,
            enter_scope: host.enter_scope,
            leave_scope: host.leave_scope,
        })
    }

    #[must_use]
    pub fn matches(&self, host: &NativeRuntimeHostV1) -> bool {
        self.context == host.context as usize
    }


    pub fn decide(
        &self,
        event: &NativeTextEventV1,
        text: &mut [u16],
        font: &mut [u16],
    ) -> NativeDecisionV1 {
        (self.decide)(
            self.context as *mut c_void,
            event,
            text.as_mut_ptr(),
            text.len() as u32,
            font.as_mut_ptr(),
            font.len() as u32,
        )
    }

    pub fn decide_v2(
        &self,
        event: &NativeTextEventV2,
        text: &mut [u16],
        font: &mut [u16],
    ) -> NativeDecisionV1 {
        (self.decide)(
            self.context as *mut c_void,
            core::ptr::from_ref(&event.base),
            text.as_mut_ptr(),
            text.len() as u32,
            font.as_mut_ptr(),
            font.len() as u32,
        )
    }

    /// Claims the current complete-text drawing call, including subordinate
    /// rasterization callbacks. The guard cannot move to a different thread.
    pub fn enter_scope(&self) -> Option<NativeTextScope> {
        let token = self.enter_scope?(self.context as *mut c_void);
        (token != 0).then_some(NativeTextScope {
            context: self.context,
            token,
            leave: self.leave_scope?,
            _thread: core::marker::PhantomData,
        })
    }
}

pub struct NativeTextScope {
    context: usize,
    token: u64,
    leave: LeaveTextScopeV1,
    _thread: core::marker::PhantomData<*mut ()>,
}
impl Drop for NativeTextScope {
    fn drop(&mut self) {
        (self.leave)(self.context as *mut c_void, self.token);
    }
}
