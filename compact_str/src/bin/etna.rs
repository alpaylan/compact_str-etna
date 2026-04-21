// ETNA workload runner for compact_str.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: TryReserveOverflowReturnsErr
//           | RetainPanicPreservesUtf8
//           | All
//
// Each invocation emits one JSON line on stdout and exits 0 (usage errors
// exit 2). Adapters drive their framework crate directly — no subprocess
// dispatch.

use compact_str::etna::{
    property_retain_panic_preserves_utf8, property_try_reserve_overflow_returns_err,
    PropertyResult,
};

use crabcheck::quickcheck as crabcheck_qc;
use crabcheck::quickcheck::Arbitrary as CcArbitrary;
use hegel::{generators as hgen, HealthCheck, Hegel, Settings as HegelSettings, TestCase};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestError};
use quickcheck::{Arbitrary as QcArbitrary, Gen, QuickCheck, ResultStatus, TestResult};
use rand_etna::Rng;

use std::fmt;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &[
    "TryReserveOverflowReturnsErr",
    "RetainPanicPreservesUtf8",
];

fn cases_budget() -> u64 {
    std::env::var("ETNA_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000)
}

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if let Err(e) = r {
            return (Err(e), total);
        }
    }
    (Ok(()), total)
}

// ============================================================================
// Input wrappers
// ============================================================================

#[derive(Clone)]
struct ReserveInput {
    initial: String,
    // Choose `additional` to be within MAX-64 .. MAX so overflow is guaranteed
    // for any non-empty initial string. We represent the knob compactly.
    additional: usize,
}

impl fmt::Debug for ReserveInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {}", self.initial, self.additional)
    }
}

impl fmt::Display for ReserveInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Clone)]
struct RetainInput {
    input: String,
    panic_after: usize,
}

impl fmt::Debug for RetainInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {}", self.input, self.panic_after)
    }
}

impl fmt::Display for RetainInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// ============================================================================
// Canonical witness inputs — used by `tool=etna` to replay frozen cases.
// ============================================================================

fn check_try_reserve_overflow_returns_err() -> Result<(), String> {
    to_err(property_try_reserve_overflow_returns_err("x".into(), usize::MAX))?;
    to_err(property_try_reserve_overflow_returns_err(
        "abcdefghijklmnop".into(),
        usize::MAX - 8,
    ))?;
    Ok(())
}

fn check_retain_panic_preserves_utf8() -> Result<(), String> {
    to_err(property_retain_panic_preserves_utf8("abcdef".into(), 2))?;
    to_err(property_retain_panic_preserves_utf8("abcdef".into(), 0))?;
    Ok(())
}

fn panic_msg(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "panic with non-string payload".to_string()
    }
}

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| match property {
        "TryReserveOverflowReturnsErr" => check_try_reserve_overflow_returns_err(),
        "RetainPanicPreservesUtf8" => check_retain_panic_preserves_utf8(),
        _ => Err(format!("Unknown property for etna: {}", property)),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let status = match result {
        Ok(r) => r,
        Err(payload) => Err(panic_msg(payload)),
    };
    (
        status,
        Metrics {
            inputs: 1,
            elapsed_us,
        },
    )
}

// ============================================================================
// Shared generator helpers
// ============================================================================

fn printable_ascii_string_from_u8s(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for b in bytes {
        let c = ((b % 94) + 32) as char;
        s.push(c);
    }
    s
}

// Carve a usize in [usize::MAX - 255, usize::MAX] from a single byte. For any
// non-empty string, len + additional is guaranteed to overflow.
fn near_max_usize_from_u8(k: u8) -> usize {
    usize::MAX - (k as usize)
}

// ============================================================================
// quickcheck Arbitrary
// ============================================================================

impl QcArbitrary for ReserveInput {
    fn arbitrary(g: &mut Gen) -> Self {
        let len = (<u8 as QcArbitrary>::arbitrary(g) % 32).max(1) as usize;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            bytes.push(<u8 as QcArbitrary>::arbitrary(g));
        }
        let initial = printable_ascii_string_from_u8s(&bytes);
        let k = <u8 as QcArbitrary>::arbitrary(g);
        ReserveInput {
            initial,
            additional: near_max_usize_from_u8(k),
        }
    }
}

impl QcArbitrary for RetainInput {
    fn arbitrary(g: &mut Gen) -> Self {
        let len = ((<u8 as QcArbitrary>::arbitrary(g) % 15) + 1) as usize;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            bytes.push(<u8 as QcArbitrary>::arbitrary(g));
        }
        let input = printable_ascii_string_from_u8s(&bytes);
        let panic_after = (<u8 as QcArbitrary>::arbitrary(g) as usize) % len;
        RetainInput { input, panic_after }
    }
}

// ============================================================================
// crabcheck Arbitrary
// ============================================================================

impl<R: Rng> CcArbitrary<R> for ReserveInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        let len = (rng.random::<u8>() % 32).max(1) as usize;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            bytes.push(rng.random::<u8>());
        }
        let initial = printable_ascii_string_from_u8s(&bytes);
        let k = rng.random::<u8>();
        ReserveInput {
            initial,
            additional: near_max_usize_from_u8(k),
        }
    }
}

impl<R: Rng> CcArbitrary<R> for RetainInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        let len = ((rng.random::<u8>() % 15) + 1) as usize;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            bytes.push(rng.random::<u8>());
        }
        let input = printable_ascii_string_from_u8s(&bytes);
        let panic_after = (rng.random::<u8>() as usize) % len;
        RetainInput { input, panic_after }
    }
}

// ============================================================================
// proptest strategies
// ============================================================================

fn reserve_strategy() -> BoxedStrategy<ReserveInput> {
    (
        proptest::string::string_regex("[ -~]{1,32}").unwrap(),
        0u8..=255u8,
    )
        .prop_map(|(initial, k)| ReserveInput {
            initial,
            additional: near_max_usize_from_u8(k),
        })
        .boxed()
}

fn retain_strategy() -> BoxedStrategy<RetainInput> {
    (
        proptest::string::string_regex("[ -~]{1,16}").unwrap(),
        0usize..16,
    )
        .prop_map(|(input, k)| {
            let len = input.chars().count().max(1);
            RetainInput {
                panic_after: k % len,
                input,
            }
        })
        .boxed()
}

// ============================================================================
// proptest adapter
// ============================================================================

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let cfg = ProptestConfig {
        cases: cases_budget().min(u32::MAX as u64) as u32,
        max_shrink_iters: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(cfg);
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "TryReserveOverflowReturnsErr" => runner
            .run(&reserve_strategy(), move |v| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex = format!("({:?})", v);
                let out = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_try_reserve_overflow_returns_err(v.initial.clone(), v.additional)
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(cex)),
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        "RetainPanicPreservesUtf8" => runner
            .run(&retain_strategy(), move |v| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex = format!("({:?})", v);
                let out = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_retain_panic_preserves_utf8(v.input.clone(), v.panic_after)
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(cex)),
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {}", property)),
                Metrics::default(),
            );
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (result, Metrics { inputs, elapsed_us })
}

// ============================================================================
// quickcheck adapter (fork with `etna` feature — fn-pointer API)
// ============================================================================

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_try_reserve_overflow_returns_err(v: ReserveInput) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = std::panic::catch_unwind(AssertUnwindSafe(|| {
        property_try_reserve_overflow_returns_err(v.initial.clone(), v.additional)
    }));
    match out {
        Ok(PropertyResult::Pass) => TestResult::passed(),
        Ok(PropertyResult::Discard) => TestResult::discard(),
        Ok(PropertyResult::Fail(_)) | Err(_) => TestResult::failed(),
    }
}

fn qc_retain_panic_preserves_utf8(v: RetainInput) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = std::panic::catch_unwind(AssertUnwindSafe(|| {
        property_retain_panic_preserves_utf8(v.input.clone(), v.panic_after)
    }));
    match out {
        Ok(PropertyResult::Pass) => TestResult::passed(),
        Ok(PropertyResult::Discard) => TestResult::discard(),
        Ok(PropertyResult::Fail(_)) | Err(_) => TestResult::failed(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let budget = cases_budget();
    let mut qc = QuickCheck::new()
        .tests(budget)
        .max_tests(budget.saturating_mul(4))
        .max_time(Duration::from_secs(86_400));
    let result = match property {
        "TryReserveOverflowReturnsErr" => qc.quicktest(
            qc_try_reserve_overflow_returns_err as fn(ReserveInput) -> TestResult,
        ),
        "RetainPanicPreservesUtf8" => qc.quicktest(
            qc_retain_panic_preserves_utf8 as fn(RetainInput) -> TestResult,
        ),
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {}", property)),
                Metrics::default(),
            );
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("quickcheck aborted: {:?}", err)),
        ResultStatus::TimedOut => Err("quickcheck timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "quickcheck gave up after {} tests",
            result.n_tests_passed
        )),
    };
    (status, Metrics { inputs, elapsed_us })
}

// ============================================================================
// crabcheck adapter
// ============================================================================

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_try_reserve_overflow_returns_err(v: ReserveInput) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_try_reserve_overflow_returns_err(v.initial, v.additional) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_retain_panic_preserves_utf8(v: RetainInput) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_retain_panic_preserves_utf8(v.input, v.panic_after) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cfg = crabcheck_qc::Config {
        tests: cases_budget(),
    };
    let result = match property {
        "TryReserveOverflowReturnsErr" => {
            crabcheck_qc::quickcheck_with_config(cfg, cc_try_reserve_overflow_returns_err)
        }
        "RetainPanicPreservesUtf8" => {
            crabcheck_qc::quickcheck_with_config(cfg, cc_retain_panic_preserves_utf8)
        }
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {}", property)),
                Metrics::default(),
            );
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("crabcheck timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "crabcheck gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => {
            Err(format!("crabcheck aborted: {}", error))
        }
    };
    (status, Metrics { inputs, elapsed_us })
}

// ============================================================================
// hegel adapter (hegeltest 0.3.7 — panic-on-cex API)
// ============================================================================

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(cases_budget())
        .suppress_health_check(HealthCheck::all())
}

fn hg_draw_u8(tc: &TestCase) -> u8 {
    let v = tc.draw(hgen::integers::<u32>().min_value(0).max_value(255));
    v as u8
}

fn hg_draw_printable_string(tc: &TestCase, min_len: usize, max_len: usize) -> String {
    let len_range =
        hgen::integers::<usize>().min_value(min_len).max_value(max_len);
    let len = tc.draw(len_range);
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(hg_draw_u8(tc));
    }
    printable_ascii_string_from_u8s(&bytes)
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(AssertUnwindSafe(|| match property {
        "TryReserveOverflowReturnsErr" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let initial = hg_draw_printable_string(&tc, 1, 32);
                let k = hg_draw_u8(&tc);
                let additional = near_max_usize_from_u8(k);
                let cex = format!("({:?} {})", initial, additional);
                let out = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_try_reserve_overflow_returns_err(initial.clone(), additional)
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{}", cex),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "RetainPanicPreservesUtf8" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let input = hg_draw_printable_string(&tc, 1, 16);
                let k = hg_draw_u8(&tc) as usize;
                let len = input.chars().count().max(1);
                let panic_after = k % len;
                let cex = format!("({:?} {})", input, panic_after);
                let out = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    property_retain_panic_preserves_utf8(input.clone(), panic_after)
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{}", cex),
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{}", property),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = panic_msg(e);
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {}", rest)),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (Err(format!("Unknown tool: {}", tool)), Metrics::default()),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!(
            "Properties: TryReserveOverflowReturnsErr | RetainPanicPreservesUtf8 | All"
        );
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (result, metrics) = match caught {
        Ok(outcome) => outcome,
        Err(payload) => {
            emit_json(
                tool,
                property,
                "aborted",
                Metrics::default(),
                None,
                Some(&format!("adapter panic: {}", panic_msg(payload))),
            );
            return;
        }
    };

    match result {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(msg) => emit_json(tool, property, "failed", metrics, Some(&msg), None),
    }
}
