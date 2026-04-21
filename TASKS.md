# compact_str — ETNA Tasks

Total tasks: 8

ETNA tasks are **mutation/property/witness triplets**. Each row below is one runnable task.

## Task Index

| Task | Variant | Framework | Property | Witness | Command |
|------|---------|-----------|----------|---------|---------|
| 001  | `try_reserve_overflow_silent_ae5f2bc_1`      | proptest   | `property_try_reserve_overflow_returns_err` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max`     | `cargo run --release --bin etna -- proptest TryReserveOverflowReturnsErr` |
| 002  | `try_reserve_overflow_silent_ae5f2bc_1`      | quickcheck | `property_try_reserve_overflow_returns_err` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max`     | `cargo run --release --bin etna -- quickcheck TryReserveOverflowReturnsErr` |
| 003  | `try_reserve_overflow_silent_ae5f2bc_1`      | crabcheck  | `property_try_reserve_overflow_returns_err` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max`     | `cargo run --release --bin etna -- crabcheck TryReserveOverflowReturnsErr` |
| 004  | `try_reserve_overflow_silent_ae5f2bc_1`      | hegel      | `property_try_reserve_overflow_returns_err` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max`     | `cargo run --release --bin etna -- hegel TryReserveOverflowReturnsErr` |
| 005  | `retain_leaves_stale_len_on_panic_042b64a_1` | proptest   | `property_retain_panic_preserves_utf8`      | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii`            | `cargo run --release --bin etna -- proptest RetainPanicPreservesUtf8` |
| 006  | `retain_leaves_stale_len_on_panic_042b64a_1` | quickcheck | `property_retain_panic_preserves_utf8`      | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii`            | `cargo run --release --bin etna -- quickcheck RetainPanicPreservesUtf8` |
| 007  | `retain_leaves_stale_len_on_panic_042b64a_1` | crabcheck  | `property_retain_panic_preserves_utf8`      | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii`            | `cargo run --release --bin etna -- crabcheck RetainPanicPreservesUtf8` |
| 008  | `retain_leaves_stale_len_on_panic_042b64a_1` | hegel      | `property_retain_panic_preserves_utf8`      | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii`            | `cargo run --release --bin etna -- hegel RetainPanicPreservesUtf8` |

## Witness catalog

Each witness is a deterministic concrete test. Base build: passes. Variant-active build: fails.

- `witness_try_reserve_overflow_returns_err_case_from_nonempty_max` — `CompactString::from("x").try_reserve(usize::MAX)`. Base returns `Err(ReserveError)` because `1.checked_add(usize::MAX)` overflows. Variant wraps the addition to `0`, which trivially fits capacity, and returns `Ok(())` — so the property detects the silent overflow.
- `witness_try_reserve_overflow_returns_err_case_from_16char_near_max` — `CompactString::from("abcdefghijklmnop").try_reserve(usize::MAX - 8)`. Base surfaces overflow via `checked_add`. Variant wraps to `7`, passes the `needed_capacity <= capacity` fast path, returns `Ok(())`.
- `witness_retain_panic_preserves_utf8_case_panic_mid_ascii` — `CompactString::from("abcdef").retain(|c| { if seen == 2 { panic!() } else { seen += 1; true } })`. Base: `SetLenOnDrop` runs during unwind, final state is `len=2` with valid UTF-8 `"ab"`. Variant: the function-ending `set_len(dest_idx)` is skipped by the unwind, leaving `len=6` and the original `"abcdef"` bytes — contents diverge from `std::string::String::retain`'s behaviour.
- `witness_retain_panic_preserves_utf8_case_panic_first_char` — `retain` with a predicate that panics on the very first char of `"abcdef"`. Base truncates to `len=0`; variant leaves the string untouched at `len=6` — the property's cross-check against `String::retain` flags the difference.
