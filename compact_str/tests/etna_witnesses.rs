// Deterministic witness tests for every ETNA variant. Each witness calls
// `property_<name>` with frozen inputs and asserts the `Pass` variant.
//
// Base HEAD: all witnesses pass.
// Variant-active (M_<variant>=active or etna/<variant>): at least one witness
// per variant must fail, proving the mutation is detected.

#![cfg(feature = "etna")]

use compact_str::etna::{
    property_retain_panic_preserves_utf8, property_try_reserve_overflow_returns_err,
    PropertyResult,
};

fn assert_pass(label: &str, r: PropertyResult) {
    match r {
        PropertyResult::Pass => {}
        PropertyResult::Discard => panic!("{label}: discarded (bad witness input)"),
        PropertyResult::Fail(m) => panic!("{label}: property failed: {m}"),
    }
}

// --- Variant: try_reserve_overflow_silent_ae5f2bc_1 --------------------------

#[test]
fn witness_try_reserve_overflow_returns_err_case_from_nonempty_max() {
    // A one-character string + additional=usize::MAX overflows usize;
    // try_reserve must surface that as an Err, not silently wrap.
    let r = property_try_reserve_overflow_returns_err("x".into(), usize::MAX);
    assert_pass("case_from_nonempty_max", r);
}

#[test]
fn witness_try_reserve_overflow_returns_err_case_from_16char_near_max() {
    // 16-char string + (usize::MAX - 8) overflows; fixed code returns Err.
    let r = property_try_reserve_overflow_returns_err(
        "abcdefghijklmnop".into(),
        usize::MAX - 8,
    );
    assert_pass("case_from_16char_near_max", r);
}

// --- Variant: retain_leaves_stale_len_on_panic_042b64a_1 ---------------------

#[test]
fn witness_retain_panic_preserves_utf8_case_panic_mid_ascii() {
    // Predicate panics after retaining 2 ASCII chars; fixed retain truncates
    // to exactly those 2 chars. Buggy retain leaves the original length and
    // the original bytes trailing past the retained prefix.
    let r = property_retain_panic_preserves_utf8("abcdef".into(), 2);
    assert_pass("case_panic_mid_ascii", r);
}

#[test]
fn witness_retain_panic_preserves_utf8_case_panic_first_char() {
    // Predicate panics on the very first char. Fixed retain yields "" (len 0).
    // Buggy retain leaves the full original string intact (len 6).
    let r = property_retain_panic_preserves_utf8("abcdef".into(), 0);
    assert_pass("case_panic_first_char", r);
}
