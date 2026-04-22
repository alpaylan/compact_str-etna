# compact_str — ETNA Tasks

Total tasks: 8

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `retain_leaves_stale_len_on_panic_042b64a_1` | proptest | `RetainPanicPreservesUtf8` | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii` |
| 002 | `retain_leaves_stale_len_on_panic_042b64a_1` | quickcheck | `RetainPanicPreservesUtf8` | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii` |
| 003 | `retain_leaves_stale_len_on_panic_042b64a_1` | crabcheck | `RetainPanicPreservesUtf8` | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii` |
| 004 | `retain_leaves_stale_len_on_panic_042b64a_1` | hegel | `RetainPanicPreservesUtf8` | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii` |
| 005 | `try_reserve_overflow_silent_ae5f2bc_1` | proptest | `TryReserveOverflowReturnsErr` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max` |
| 006 | `try_reserve_overflow_silent_ae5f2bc_1` | quickcheck | `TryReserveOverflowReturnsErr` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max` |
| 007 | `try_reserve_overflow_silent_ae5f2bc_1` | crabcheck | `TryReserveOverflowReturnsErr` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max` |
| 008 | `try_reserve_overflow_silent_ae5f2bc_1` | hegel | `TryReserveOverflowReturnsErr` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max` |

## Witness Catalog

- `witness_retain_panic_preserves_utf8_case_panic_mid_ascii` — base passes, variant fails
- `witness_retain_panic_preserves_utf8_case_panic_first_char` — base passes, variant fails
- `witness_try_reserve_overflow_returns_err_case_from_nonempty_max` — base passes, variant fails
- `witness_try_reserve_overflow_returns_err_case_from_16char_near_max` — base passes, variant fails
