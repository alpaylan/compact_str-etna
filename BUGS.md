# compact_str — Injected Bugs

Total mutations: 2

## Bug Index

| # | Name | Variant | File | Injection | Fix Commit |
|---|------|---------|------|-----------|------------|
| 1 | `try_reserve_overflow_silent` | `try_reserve_overflow_silent_ae5f2bc_1` | `compact_str/src/repr/mod.rs:224` | `marauders` | `ae5f2bcfc67df52b6dc9e05a0a95edbd2a52dbd8` |
| 2 | `retain_leaves_stale_len_on_panic` | `retain_leaves_stale_len_on_panic_042b64a_1` | `compact_str/src/lib.rs:1217` | `marauders` | `042b64a4f8d91ea7bd3bce54ed086bf7b2f9bfdd` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `try_reserve_overflow_silent_ae5f2bc_1` | `property_try_reserve_overflow_returns_err` | `witness_try_reserve_overflow_returns_err_case_from_nonempty_max`, `witness_try_reserve_overflow_returns_err_case_from_16char_near_max` |
| `retain_leaves_stale_len_on_panic_042b64a_1` | `property_retain_panic_preserves_utf8` | `witness_retain_panic_preserves_utf8_case_panic_mid_ascii`, `witness_retain_panic_preserves_utf8_case_panic_first_char` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `property_try_reserve_overflow_returns_err` | ✓ | ✓ | ✓ | ✓ |
| `property_retain_panic_preserves_utf8`      | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. try_reserve_overflow_silent

- **Variant**: `try_reserve_overflow_silent_ae5f2bc_1`
- **Location**: `compact_str/src/repr/mod.rs:224` (inside `Repr::reserve`)
- **Property**: `property_try_reserve_overflow_returns_err`
- **Witness(es)**: `witness_try_reserve_overflow_returns_err_case_from_nonempty_max`, `witness_try_reserve_overflow_returns_err_case_from_16char_near_max`
- **Fix commit**: `ae5f2bcfc67df52b6dc9e05a0a95edbd2a52dbd8` — `Check for overflow in reserve()`
- **Invariant violated**: `CompactString::try_reserve(additional)` must return `Err(ReserveError)` whenever `self.len() + additional` would overflow `usize`, rather than silently wrapping the capacity computation. Downstream, the wrapped value lets `reserve` claim a tiny allocation is sufficient and return `Ok(())`, which means any subsequent write operation keyed on the requested size walks past the allocation.
- **How the mutation triggers**: the pre-ae5f2bc body computes `let needed_capacity = len + additional;`. In release builds `usize` addition wraps silently, so `needed_capacity` becomes a small value and the function returns `Ok(())`. The fix replaces the expression with `len.checked_add(additional).ok_or(ReserveError(()))?`. `case_from_nonempty_max` uses `len=1, additional=usize::MAX` → the wrap produces `0`, which trivially fits. `case_from_16char_near_max` uses `len=16, additional=usize::MAX-8` → wrap produces `7`, same story. The property function discards inputs that do not overflow, so framework-generated cases only ever exercise the true overflow branch.

### 2. retain_leaves_stale_len_on_panic

- **Variant**: `retain_leaves_stale_len_on_panic_042b64a_1`
- **Location**: `compact_str/src/lib.rs:1217` (inside `CompactString::retain`)
- **Property**: `property_retain_panic_preserves_utf8`
- **Witness(es)**: `witness_retain_panic_preserves_utf8_case_panic_mid_ascii`, `witness_retain_panic_preserves_utf8_case_panic_first_char`
- **Fix commit**: `042b64a4f8d91ea7bd3bce54ed086bf7b2f9bfdd` — `fix: retain not set len if predicate panics`
- **Invariant violated**: if the predicate passed to `CompactString::retain` panics, the string's observable state after the unwind must match `std::string::String::retain`'s guarantee: the bytes prior to the panic are retained (and the length reflects exactly those bytes), and the trailing bytes beyond the panic point are dropped from both view and length. Any other outcome leaves the CompactString holding stale length / bytes that can be re-exposed on a subsequent operation.
- **How the mutation triggers**: the pre-042b64a body tracks `dest_idx`/`src_idx` as locals and only calls `self.set_len(dest_idx)` as the last statement of the function. When `predicate` panics, the unwind skips the `set_len` call entirely, so the CompactString keeps its original length and its original bytes past the retained prefix. The fix wraps the loop in a `SetLenOnDrop` guard that runs `set_len` during unwind. `case_panic_mid_ascii` panics after two retained ASCII chars (expected post-condition: `len=2`, first two bytes intact, third byte unreachable; buggy: `len=6`, original "abcdef" still visible). `case_panic_first_char` panics before any retention (expected: `len=0`; buggy: `len=6`).
