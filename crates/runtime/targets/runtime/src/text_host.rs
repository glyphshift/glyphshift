//! Resolve source evidence before capture deduplication and translation lookup.
use super::*;
use glyphshift_adapter_native_abi::*;
use glyphshift_domain::{
    TextRunEvent, TextRunKey, TextRunOutcome, TextUse, TranslationContext, MAX_TEXT_RUN_UNITS,
};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

static SCOPE_EPOCH: AtomicU64 = AtomicU64::new(1);
#[derive(Default)]
struct Scopes {
    epoch: u64,
    next: u64,
    stack: Vec<(usize, u64, bool)>,
}
thread_local! { static SCOPES: RefCell<Scopes> = RefCell::new(Scopes::default()); }

fn with_scopes<T>(call: impl FnOnce(&mut Scopes) -> T) -> T {
    SCOPES.with(|state| {
        let mut state = state.borrow_mut();
        let epoch = SCOPE_EPOCH.load(Ordering::Acquire);
        if state.epoch != epoch {
            state.epoch = epoch;
            state.stack.clear();
        }
        call(&mut state)
    })
}

pub(super) extern "C" fn enter_scope(context: *mut core::ffi::c_void) -> u64 {
    if context.is_null()
        || !runtime_state()
            .lock()
            .is_ok_and(|state| state.as_ref().is_some_and(|r| r.active))
    {
        return 0;
    }
    with_scopes(|state| {
        if state.stack.len() >= 32 {
            return 0;
        }
        let Some(next) = state.next.checked_add(1) else {
            return 0;
        };
        state.next = next;
        state.stack.push((context as usize, next, false));
        next
    })
}

pub(super) extern "C" fn leave_scope(context: *mut core::ffi::c_void, token: u64) {
    with_scopes(|state| {
        if let Some(index) = state
            .stack
            .iter()
            .position(|(owner, id, _)| (*owner, *id) == (context as usize, token))
        {
            state.stack.truncate(index);
        }
    });
}

pub(super) fn scope_blocks(context: &NativeDecisionContext) -> bool {
    with_scopes(|state| {
        state
            .stack
            .iter()
            .any(|(owner, _, replaced)| *replaced && *owner != std::ptr::from_ref(context) as usize)
    })
}

// Entering a draw scope does not establish ownership of the underlying text.
// Only a successful replacement protects its resulting nested draw calls.
pub(super) fn mark_scope_replaced(context: &NativeDecisionContext) {
    with_scopes(|state| {
        if let Some((_, _, replaced)) = state
            .stack
            .iter_mut()
            .rev()
            .find(|(owner, _, _)| *owner == std::ptr::from_ref(context) as usize)
        {
            *replaced = true;
        }
    });
}

pub(super) fn invalidate_scopes() {
    SCOPE_EPOCH.fetch_add(1, Ordering::AcqRel);
}

pub(super) extern "C" fn decide_text(
    context: *mut core::ffi::c_void,
    event: *const NativeTextEventV1,
    text_out: *mut u16,
    text_capacity: u32,
    font_out: *mut u16,
    font_capacity: u32,
) -> NativeDecisionV1 {
    if context.is_null() {
        return decision_error(STATUS_INVALID_TEXT_EVENT);
    }
    let context = unsafe { &*context.cast::<NativeDecisionContext>() };
    if event.is_null() {
        return reject_event(context);
    }
    let event = unsafe { &*event };
    if event.source_len as usize > MAX_TEXT_RUN_UNITS {
        return reject_event(context);
    }
    let translation_context = match translation_context(event) {
        Ok(context) => context,
        Err(()) => return reject_event(context),
    };
    let key = TextRunKey {
        surface: event.surface,
        run: event.run,
        epoch: event.epoch,
    };
    let source = if event.source_len == 0 {
        &[][..]
    } else {
        if event.source.is_null() {
            return reject_event(context);
        }
        unsafe { std::slice::from_raw_parts(event.source, event.source_len as usize) }
    };
    let input = match event.kind {
        TEXT_EVENT_DRAW => TextRunEvent::Complete {
            text: source,
            usage: TextUse::Draw,
        },
        TEXT_EVENT_RETAINED => TextRunEvent::Complete {
            text: source,
            usage: TextUse::Retained,
        },
        TEXT_EVENT_OBSERVE => TextRunEvent::Complete {
            text: source,
            usage: TextUse::Observe,
        },
        TEXT_EVENT_FRAGMENT => TextRunEvent::Fragment {
            key,
            ordinal: event.ordinal,
            text: source,
        },
        TEXT_EVENT_FINISH if source.is_empty() => TextRunEvent::Finish {
            key,
            parts: event.ordinal,
        },
        TEXT_EVENT_CANCEL if source.is_empty() => TextRunEvent::Cancel { key },
        TEXT_EVENT_GLYPH_RASTER if source.is_empty() => TextRunEvent::GlyphRaster,
        _ => return reject_event(context),
    };
    if translation_context.is_some()
        && !matches!(event.kind, TEXT_EVENT_DRAW | TEXT_EVENT_RETAINED | TEXT_EVENT_OBSERVE)
    {
        return reject_event(context);
    }
    // Never carry incomplete runs across a paused/restarted Runtime lifecycle.
    let (generation, outcome) = match runtime_state().lock() {
        Ok(state) => match state.as_ref().filter(|runtime| runtime.active) {
            Some(runtime) => {
                if scope_blocks(context) {
                    return NativeDecisionV1 {
                        generation: runtime.publication.generation().value(),
                        ..decision_error(STATUS_OK)
                    };
                }
                let outcome = match context.text_runs.lock() {
                    Ok(mut runs) => runs.accept(input),
                    Err(_) => return decision_error(STATUS_INVALID_TEXT_EVENT),
                };
                (runtime.publication.generation().value(), outcome)
            }
            None => return decision_error(STATUS_INVALID_TEXT_EVENT),
        },
        Err(_) => return decision_error(STATUS_INVALID_TEXT_EVENT),
    };
    match outcome {
        TextRunOutcome::Resolved(text) => decide_source(
            context,
            &text.text,
            text_out,
            text_capacity,
            font_out,
            font_capacity,
            text.can_replace(),
            translation_context.as_ref(),
        ),
        TextRunOutcome::Rejected => decision_error(STATUS_INVALID_TEXT_EVENT),
        TextRunOutcome::Pending | TextRunOutcome::Ignored => NativeDecisionV1 {
            generation,
            ..decision_error(STATUS_OK)
        },
    }
}

fn translation_context(event: &NativeTextEventV1) -> Result<Option<TranslationContext>, ()> {
    if event.struct_size == size_of::<NativeTextEventV1>() as u32 {
        return Ok(None);
    }
    if event.struct_size != size_of::<NativeTextEventV2>() as u32 {
        return Err(());
    }
    let event = unsafe { &*std::ptr::from_ref(event).cast::<NativeTextEventV2>() };
    if event.metadata_version != TEXT_EVENT_METADATA_VERSION_V1
        || event.metadata_flags & !TEXT_EVENT_METADATA_KNOWN_FLAGS != 0
        || event.reserved != 0
    {
        return Err(());
    }

    let context = read_optional_metadata(
        event.metadata_flags & TEXT_EVENT_METADATA_CONTEXT != 0,
        event.context,
        event.context_len,
    )?;
    let disambiguation = read_optional_metadata(
        event.metadata_flags & TEXT_EVENT_METADATA_DISAMBIGUATION != 0,
        event.disambiguation,
        event.disambiguation_len,
    )?;
    let plural_n = if event.metadata_flags & TEXT_EVENT_METADATA_PLURAL_N != 0 {
        (event.plural_n >= 0).then_some(event.plural_n).ok_or(())?
    } else {
        if event.plural_n != -1 {
            return Err(());
        }
        -1
    };
    Ok(Some(TranslationContext::new(
        context,
        disambiguation,
        (plural_n >= 0).then_some(plural_n),
    )))
}

fn read_optional_metadata(
    present: bool,
    pointer: *const u16,
    length: u32,
) -> Result<Option<Box<str>>, ()> {
    if !present {
        return (length == 0 && pointer.is_null()).then_some(None).ok_or(());
    }
    if length as usize > MAX_TEXT_EVENT_CONTEXT_UNITS {
        return Err(());
    }
    let source = if length == 0 {
        &[][..]
    } else {
        if pointer.is_null() {
            return Err(());
        }
        unsafe { std::slice::from_raw_parts(pointer, length as usize) }
    };
    String::from_utf16(source)
        .map(String::into_boxed_str)
        .map(Some)
        .map_err(|_| ())
}

fn reject_event(context: &NativeDecisionContext) -> NativeDecisionV1 {
    // Invalid metadata cannot leave a prefix that a later Finish would publish.
    if let Ok(mut runs) = context.text_runs.lock() {
        runs.reset();
    }
    decision_error(STATUS_INVALID_TEXT_EVENT)
}

pub(super) fn reset_runs(runtime: &RuntimeState) {
    for host in &runtime.native_hosts {
        let host = unsafe { &*(*host as *const NativeRuntimeHostV1) };
        let context = unsafe { &*host.context.cast::<NativeDecisionContext>() };
        if let Ok(mut runs) = context.text_runs.lock() {
            runs.reset();
        }
    }
}
