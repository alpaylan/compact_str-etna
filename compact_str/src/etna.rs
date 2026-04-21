//! ETNA property functions for compact_str.
//!
//! Each `property_*` function is a framework-neutral invariant. Adapters for
//! proptest / quickcheck / crabcheck / hegel and the deterministic witness
//! tests all call these functions directly. The functions themselves have no
//! RNG, no clock, and no I/O.

use alloc::string::String;
use core::cell::Cell;

use crate::CompactString;

/// Three-way result used by every property and adapter.
#[derive(Debug, Clone)]
pub enum PropertyResult {
    Pass,
    Fail(String),
    Discard,
}

/// Invariant: `CompactString::try_reserve(additional)` must return
/// [`Err`](crate::ReserveError) whenever `len() + additional` overflows
/// `usize`; it must not silently wrap and leave the string in a corrupt state.
///
/// Violated by ae5f2bc before the fix: `new_capacity = len() + additional`
/// wrapped on overflow, leading to silent under-allocation.
pub fn property_try_reserve_overflow_returns_err(
    initial: String,
    additional: usize,
) -> PropertyResult {
    let len = initial.len();
    let overflows = len.checked_add(additional).is_none();
    if !overflows {
        return PropertyResult::Discard;
    }
    let mut s = CompactString::from(&initial);
    match s.try_reserve(additional) {
        Ok(()) => PropertyResult::Fail(format!(
            "try_reserve({additional}) on len={len} returned Ok despite overflow"
        )),
        Err(_) => PropertyResult::Pass,
    }
}

/// Invariant: if the predicate passed to [`CompactString::retain`] panics
/// partway through iteration, any witnesses that observe the string after
/// panic must see valid UTF-8 content whose length reflects only the
/// successfully-retained prefix — exactly what `String::retain` promises.
///
/// Violated by 042b64a before the fix: `retain` wrote retained bytes into the
/// buffer in-place but set the length at the *end*. If the predicate panicked,
/// the length stayed at the original full value, leaving the stale (now
/// overwritten) bytes past the retained prefix observable as garbage.
///
/// Inputs:
/// - `input`: source string to retain from. Must contain at least
///   `panic_after + 1` chars; otherwise the test is discarded.
/// - `panic_after`: 0-based index of the char at which the predicate panics.
pub fn property_retain_panic_preserves_utf8(
    input: String,
    panic_after: usize,
) -> PropertyResult {
    let total_chars = input.chars().count();
    if total_chars == 0 || panic_after >= total_chars {
        return PropertyResult::Discard;
    }

    // Reference: what String::retain does on the same panic path.
    let mut reference = String::from(&input);
    let seen_ref: Cell<usize> = Cell::new(0);
    let res_ref = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reference.retain(|_| {
            let i = seen_ref.get();
            seen_ref.set(i + 1);
            if i == panic_after {
                panic!("boom");
            }
            true
        });
    }));
    if res_ref.is_ok() {
        return PropertyResult::Discard;
    }

    let mut compact = CompactString::from(&input);
    let seen_compact: Cell<usize> = Cell::new(0);
    let res_compact = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compact.retain(|_| {
            let i = seen_compact.get();
            seen_compact.set(i + 1);
            if i == panic_after {
                panic!("boom");
            }
            true
        });
    }));
    if res_compact.is_ok() {
        return PropertyResult::Fail(format!(
            "CompactString::retain did not propagate predicate panic (panic_after={panic_after})"
        ));
    }

    if compact.as_str() != reference.as_str() {
        return PropertyResult::Fail(format!(
            "after panic: CompactString={compact:?} but String={reference:?} (panic_after={panic_after})"
        ));
    }
    if compact.len() != reference.len() {
        return PropertyResult::Fail(format!(
            "after panic: CompactString::len={} but String::len={} (panic_after={panic_after})",
            compact.len(),
            reference.len()
        ));
    }
    PropertyResult::Pass
}

