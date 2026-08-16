# xmltodict_rs Review Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
> Ledger: `docs/plans/2026-08-12-review-fixes-ledger.md` — update Status/Commit after every task.

**Goal:** Fix the correctness bugs and the segfault found in the 2026-08-12 code review, make the API honest (raise instead of silently diverging from xmltodict), remove dead code and redundant unsafe, and take allocations off the hot path.

**Architecture:** The library is a PyO3 binding on top of quick-xml: `parse` (event loop in `lib.rs` + the `XmlParser` state machine in `parser.rs`), `unparse` (`XmlWriter` in `unparser.rs`). Every behavioral change is verified differentially against the `xmltodict==0.13.0` reference (the `compare_parsers` pattern in tests). Task order: bugs first (TDD), then API honesty, then cleanup, then performance (benchmark-gated), then test gaps.

**Tech Stack:** Rust (pyo3 0.26 → 0.29, quick-xml 0.31 → 0.41, memchr, stacker — upgraded in Task 7), maturin, pytest + xmltodict 0.13.0 as the reference.

**Working commands:**

```bash
# rebuild after every Rust change (mandatory before pytest)
.venv/bin/maturin develop --release

# tests
.venv/bin/python -m pytest tests/ -q

# Rust lints (zero warnings — Cargo.toml has warnings = "deny")
cargo clippy --all-targets

# benchmark for perf tasks
.venv/bin/python benches/accurate_benchmark.py
```

**Commit rules:** no co-author trailers, straight quotes, messages in English.

**Rule for optimizations (Tasks 7, 11-14):** every perf change is gated by `benches/perf_gate.py` (added in Task 0): run `measure` BEFORE and AFTER the change, then `compare`; record the per-case numbers in the ledger ("Benchmarks" section). A delta counts as a win/regression only when it exceeds the noise band (per-case spread of repeat medians, floor 2%). Single-run `accurate_benchmark.py` speedups are NOT gate evidence — its run-to-run P50 drift reaches ~9% (measured 2026-08-13); it stays only for README-facing speedup-vs-xmltodict numbers. No confirmed win and no other benefit (simplification, unsafe removal) — revert. The final "what was optimized and by how much" summary is assembled in Task 16 (the "Optimization results" section of the ledger).

---

## Task 0: Benchmark gate tool (benches/perf_gate.py)

`benches/accurate_benchmark.py` is unsuitable for gating small deltas: run-to-run P50 drift of xmltodict_rs reaches ~9% at identical code (measured 2026-08-13), the speedup metric divides by a concurrently-noisy xmltodict measurement, the headline number is an outlier-sensitive mean, the large-XML document embeds `time.time()` (non-deterministic data), and there is no machine-readable output. It stays as the README-facing comparison against the reference. For gating perf tasks, add a dedicated A/B tool that measures only absolute xmltodict_rs timings.

**Files:**
- Create: `benches/perf_gate.py`

**Step 1: Implement**

```python
"""A/B performance gate: absolute timings of xmltodict_rs only.

Methodology: fixed deterministic documents; per case, REPEATS independent
runs of LOOP_SECONDS each, median per run; the case metric is the median of
run medians, and the min..max spread of run medians is the noise band.

Usage:
    python benches/perf_gate.py measure out.json
    python benches/perf_gate.py compare before.json after.json
"""

import gc
import json
import statistics
import sys
import time

import xmltodict_rs

REPEATS = 5
LOOP_SECONDS = 1.0
WARMUP_SECONDS = 0.5
NOISE_FLOOR_PCT = 2.0


def build_docs() -> dict[str, str]:
    """Deterministic documents: small config, medium catalog, large records."""
    small = (
        '<?xml version="1.0" encoding="utf-8"?>'
        '<root><item id="1">Small test</item>'
        "<config><debug>true</debug><timeout>30</timeout></config></root>"
    )
    medium_items = "".join(
        f'<product id="{i}" category="test">'
        f"<name>Product {i}</name><price>{10.0 + i}</price>"
        f"<description>Description {i}</description>"
        f'<available>{"true" if i % 2 == 0 else "false"}</available></product>'
        for i in range(50)
    )
    medium = f'<?xml version="1.0" encoding="utf-8"?><catalog>{medium_items}</catalog>'
    large_items = "".join(
        f'<record id="{i}" type="data" priority="{i % 5}" category="cat{i % 10}" '
        f'status="active" created="2024-01-{(i % 30) + 1:02d}">'
        f"<title>Record Title {i}</title>"
        f'<content>{"Long content text " * 5} for record {i}</content>'
        f"<tags><tag>tag{i % 7}</tag><tag>category{i % 5}</tag></tags>"
        f"</record>"
        for i in range(200)
    )
    large = (
        '<?xml version="1.0" encoding="utf-8"?>'
        f'<database version="2.0">{large_items}</database>'
    )
    return {"small": small, "medium": medium, "large": large}


def run_median(func, arg, seconds: float) -> float:
    """One run: tight loop for `seconds`, returns the median call time."""
    times = []
    deadline = time.perf_counter() + seconds
    while time.perf_counter() < deadline:
        t0 = time.perf_counter()
        func(arg)
        times.append(time.perf_counter() - t0)
    return statistics.median(times)


def measure(out_path: str) -> None:
    """Measure all cases, print a summary, write JSON to out_path."""
    results = {}
    for name, xml in build_docs().items():
        parsed = xmltodict_rs.parse(xml)
        for kind, func, arg in (
            ("parse", xmltodict_rs.parse, xml),
            ("unparse", xmltodict_rs.unparse, parsed),
        ):
            run_median(func, arg, WARMUP_SECONDS)
            gc.disable()
            try:
                medians = [run_median(func, arg, LOOP_SECONDS) for _ in range(REPEATS)]
            finally:
                gc.enable()
            med = statistics.median(medians)
            spread_pct = (max(medians) - min(medians)) / med * 100
            results[f"{name}-{kind}"] = {"median_us": med * 1e6, "spread_pct": spread_pct}
            print(f"{name}-{kind}: {med * 1e6:.2f}us (spread {spread_pct:.1f}%)")
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"written: {out_path}")


def compare(before_path: str, after_path: str) -> None:
    """Per-case delta with a noise verdict; negative delta means faster."""
    with open(before_path) as f:
        before = json.load(f)
    with open(after_path) as f:
        after = json.load(f)
    print(f"{'case':<18} {'before us':>10} {'after us':>10} {'delta':>8}  verdict")
    for case, b in before.items():
        a = after.get(case)
        if a is None:
            print(f"{case:<18} missing in after")
            continue
        delta_pct = (a["median_us"] - b["median_us"]) / b["median_us"] * 100
        threshold = max(b["spread_pct"], a["spread_pct"], NOISE_FLOOR_PCT)
        if delta_pct <= -threshold:
            verdict = "FASTER"
        elif delta_pct >= threshold:
            verdict = "SLOWER"
        else:
            verdict = f"noise (±{threshold:.1f}%)"
        print(
            f"{case:<18} {b['median_us']:>10.2f} {a['median_us']:>10.2f} "
            f"{delta_pct:>+7.1f}%  {verdict}"
        )


def main() -> None:
    if len(sys.argv) == 3 and sys.argv[1] == "measure":
        measure(sys.argv[2])
    elif len(sys.argv) == 4 and sys.argv[1] == "compare":
        compare(sys.argv[2], sys.argv[3])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
```

**Step 2: Validate the tool itself**

Run `measure` twice on the unchanged build and `compare` the two JSONs — every case must land in the noise band (no FASTER/SLOWER verdicts on identical code):

```bash
.venv/bin/python benches/perf_gate.py measure /tmp/perf-selftest-a.json
.venv/bin/python benches/perf_gate.py measure /tmp/perf-selftest-b.json
.venv/bin/python benches/perf_gate.py compare /tmp/perf-selftest-a.json /tmp/perf-selftest-b.json
```

If any case reports FASTER/SLOWER on identical code, raise REPEATS/LOOP_SECONDS until self-comparison is clean, and note the final constants in the ledger.

Record the baseline JSON for the whole plan: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-baseline.json` and copy the printed numbers into the ledger ("Benchmarks" section).

**Step 3: Commit**

```bash
git add benches/perf_gate.py
git commit -m "test: add perf_gate benchmark for A/B gating of optimizations"
```

---

## Task 1: Bug — postprocessor lost from the 3rd repeated element on

`push_data`, when appending to an existing list, pushes the raw `data` instead of the processed `final_value` ([parser.rs:144](../../src/parser.rs)).

**Files:**
- Modify: `src/parser.rs:141-158` (`push_data`)
- Test: `tests/test_parse_callbacks.py`

**Step 1: Write the failing test**

Add to `tests/test_parse_callbacks.py`:

```python
def test_postprocessor_applied_to_all_repeated_elements():
    """Regression: third and later repeats must also get the postprocessed value."""

    def pp(path, key, data):
        if key == "i" and isinstance(data, str):
            return key, int(data)
        return key, data

    xml = "<r><i>1</i><i>2</i><i>3</i><i>4</i></r>"
    expected = xmltodict.parse(xml, postprocessor=pp)
    assert expected == {"r": {"i": [1, 2, 3, 4]}}
    assert xmltodict_rs.parse(xml, postprocessor=pp) == expected
```

**Step 2: Run test to verify it fails**

Run: `.venv/bin/python -m pytest tests/test_parse_callbacks.py::test_postprocessor_applied_to_all_repeated_elements -v`
Expected: FAIL — `[1, 2, '3', '4'] != [1, 2, 3, 4]`

**Step 3: Fix `push_data`**

In `src/parser.rs` replace the body of `match item.get_item(...)`:

```rust
        match item.get_item(final_key.as_str())? {
            Some(existing) => {
                if let Ok(list) = existing.downcast::<PyList>() {
                    list.append(final_value)?;
                } else {
                    let new_list = PyList::new(py, [existing, final_value])?;
                    item.set_item(final_key, &new_list)?;
                }
            }
            None => {
                if self.should_force_list(py, final_key.as_str(), final_value.as_ref())? {
                    let new_list = PyList::new(py, [final_value])?;
                    item.set_item(final_key, &new_list)?;
                } else {
                    item.set_item(final_key, final_value)?;
                }
            }
        }
```

Notes: `data` in the append is replaced with `final_value`; redundant `.clone()` calls are dropped along the way — the match arms do not overlap, moving is safe. If the borrow checker objects, keep `final_value.clone()` in the `Some` arm (cloning a `Bound` is only a refcount bump).

**Step 4: Rebuild, run tests**

Run: `.venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: all PASS, including the new test.

**Step 5: Commit**

```bash
git add src/parser.rs tests/test_parse_callbacks.py
git commit -m "fix: apply postprocessor value to 3rd+ repeated element"
```

---

## Task 2: Bug — junk after the root element is accepted

`<a>1</a>junk` and `<a/><b/>` parse without an error; expat raises `ExpatError: junk after document element`. Text BEFORE the root must also be an error.

**Files:**
- Modify: `src/lib.rs:89-133` (event loop in `parse_xml_with_reader`)
- Test: `tests/test_parse_special.py`

**Step 1: Write the failing tests**

Add to `tests/test_parse_special.py` (there are `compare_parsers`-style helpers already; if missing, import `xmltodict`):

```python
@pytest.mark.parametrize(
    "xml",
    [
        "<a>1</a>junk",
        "<a/><b/>",
        "<a/>text",
        "junk<a/>",
    ],
)
def test_junk_outside_root_raises(xml):
    with pytest.raises(ExpatError):
        xmltodict.parse(xml)
    with pytest.raises(ExpatError):
        xmltodict_rs.parse(xml)


@pytest.mark.parametrize("xml", ["<a/>\n", "  <a/>  ", "<a>1</a>\t\n"])
def test_whitespace_outside_root_is_legal(xml):
    assert xmltodict_rs.parse(xml) == xmltodict.parse(xml)
    # and with whitespace stripping disabled
    assert xmltodict_rs.parse(xml, strip_whitespace=False) == xmltodict.parse(
        xml, strip_whitespace=False
    )
```

Import `ExpatError` via `from xml.parsers.expat import ExpatError`.

**Step 2: Run tests to verify failure**

Run: `.venv/bin/python -m pytest tests/test_parse_special.py -k "outside_root" -v`
Expected: `test_junk_outside_root_raises` FAIL (rs does not raise), `test_whitespace_outside_root_is_legal` PASS.

**Step 3: Implement**

In `src/lib.rs`, inside `parse_xml_with_reader`, declare before the `loop`:

```rust
    let mut root_closed = false;
```

Change the loop arms:

```rust
            Ok(Event::Start(ref e)) => {
                if root_closed {
                    return Err(expat_error(py, "junk after document element".to_owned()));
                }
                // ... existing body unchanged
            }
            Ok(Event::End(ref e)) => {
                // ... existing body unchanged, then:
                if parser.path.is_empty() {
                    root_closed = true;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if root_closed {
                    return Err(expat_error(py, "junk after document element".to_owned()));
                }
                // ... existing body unchanged, then:
                if parser.path.is_empty() {
                    root_closed = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().map_err(|e| expat_error(py, e.to_string()))?;
                if parser.path.is_empty() && !text.trim().is_empty() {
                    let msg = if root_closed {
                        "junk after document element"
                    } else {
                        "syntax error"
                    };
                    return Err(expat_error(py, msg.to_owned()));
                }
                parser.characters(&text);
            }
            Ok(Event::CData(ref e)) => {
                if parser.path.is_empty() {
                    return Err(expat_error(py, "junk after document element".to_owned()));
                }
                parser.characters(std::str::from_utf8(e.as_ref())?);
            }
```

Logic: outside the root (`parser.path.is_empty()`), non-whitespace text and CDATA are errors; whitespace is legal per the XML spec (`trim_text(true)` swallows it anyway — the branch matters for `strip_whitespace=False`).

**Step 4: Rebuild, run full suite**

Run: `.venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: all PASS. If an existing test expected the old (incorrect) behavior — check the test against `xmltodict` and fix the test, not the implementation.

**Step 5: Commit**

```bash
git add src/lib.rs tests/test_parse_special.py
git commit -m "fix: reject junk before/after document element like expat"
```

---

## Task 3: Bug — control characters in attributes are not escaped

`\n`, `\t`, `\r` in attribute values must be written as `&#10;`, `&#9;`, `&#13;` (otherwise XML attribute-value normalization turns them into spaces on re-parse). Reference: `xmltodict.unparse({'root': {'@a': 'x\ny'}}, full_document=False)` → `'<root a="x&#10;y"></root>'`.

**Files:**
- Modify: `src/escape.rs:68-106` (`escape_xml_attr`)
- Test: `tests/test_unparse.py`, unit test in `src/escape.rs`

**Step 1: Write the failing tests**

In `tests/test_unparse.py`:

```python
def test_attr_control_chars_escaped():
    d = {"root": {"@a": "x\ny\tz\rw"}}
    ref = xmltodict.unparse(d, full_document=False)
    rs = xmltodict_rs.unparse(d, full_document=False)
    assert rs == ref
    assert "&#10;" in rs and "&#9;" in rs and "&#13;" in rs


def test_attr_control_chars_roundtrip():
    d = {"root": {"@a": "line1\nline2"}}
    assert xmltodict_rs.parse(xmltodict_rs.unparse(d)) == d
```

In `src/escape.rs` `mod tests`:

```rust
    #[test]
    fn test_escape_xml_attr_control_chars() {
        assert_eq!(
            "a&#10;b&#9;c&#13;d",
            escape_xml_attr("a\nb\tc\rd")
        );
    }
```

**Step 2: Run tests to verify failure**

Run: `.venv/bin/python -m pytest tests/test_unparse.py -k control_chars -v && cargo test escape_xml_attr_control`
Expected: both FAIL.

**Step 3: Implement**

In `escape_xml_attr` extend both `match ch`:

```rust
            '&' | '<' | '>' | '"' | '\n' | '\t' | '\r' => {
```

and the replacement table:

```rust
                let escaped = match ch {
                    '&' => "&amp;",
                    '<' => "&lt;",
                    '>' => "&gt;",
                    '"' => "&quot;",
                    '\n' => "&#10;",
                    '\t' => "&#9;",
                    _ => "&#13;",
                };
```

**Step 4: Rebuild, run tests**

Run: `cargo test && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: all PASS.

**Step 5: Commit**

```bash
git add src/escape.rs tests/test_unparse.py
git commit -m "fix: escape newline/tab/CR in attribute values as char refs"
```

---

## Task 4: Bug — `unparse(output=...)` is ignored

The reference writes the result into the file-like object and returns `None`; rs silently returns the string. Also rename the parameter `_output` → `output` (the kwarg does not match the reference at all right now).

**Files:**
- Modify: `src/lib.rs:278-346` (`unparse`)
- Test: `tests/test_unparse.py`

**Step 1: Write the failing test**

```python
def test_unparse_output_file_like():
    import io

    d = {"a": "1"}
    ref_buf, rs_buf = io.StringIO(), io.StringIO()
    ref_ret = xmltodict.unparse(d, output=ref_buf)
    rs_ret = xmltodict_rs.unparse(d, output=rs_buf)
    assert rs_ret is None and ref_ret is None
    assert rs_buf.getvalue() == ref_buf.getvalue()
```

**Step 2: Run test to verify it fails**

Run: `.venv/bin/python -m pytest tests/test_unparse.py::test_unparse_output_file_like -v`
Expected: FAIL — `TypeError: ... unexpected keyword argument 'output'`.

**Step 3: Implement**

In `src/lib.rs`: in `#[pyo3(signature = (...))]` and the parameters replace `_output = None` → `output = None`, `_output: Option<&Bound<'_, PyAny>>` → `output: Option<&Bound<'_, PyAny>>`. End of the function:

```rust
    let result = writer.finish();

    if let Some(out) = output {
        out.call_method1("write", (result,))?;
        return Ok(py.None());
    }

    Ok(result.into_pyobject(py)?.into_any().unbind())
}
```

**Step 4: Rebuild, run tests**

Run: `.venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: all PASS.

**Step 5: Commit**

```bash
git add src/lib.rs tests/test_unparse.py
git commit -m "fix: support output= file-like argument in unparse"
```

---

## Task 5: Bug — segfault on deeply nested unparse

`write_element`/`write_dict_element` are recursive; a dict ~100k levels deep → stack overflow → SIGSEGV of the whole interpreter. Fix: `stacker::maybe_grow` — grow the stack in heap-allocated segments, keeping unlimited depth.

**Files:**
- Modify: `Cargo.toml` (dependency), `src/unparser.rs:92` (`write_element`)
- Test: `tests/test_unparse.py`

**Step 1: Write the failing test**

```python
def test_deeply_nested_unparse_does_not_crash():
    depth = 100_000
    root = {}
    cur = root
    for _ in range(depth):
        nxt = {}
        cur["n"] = nxt
        cur = nxt
    cur["n"] = "x"
    result = xmltodict_rs.unparse({"root": root})
    assert result.count("<n>") == depth + 1
```

**Step 2: Run test to verify it fails**

Run: `.venv/bin/python -m pytest tests/test_unparse.py::test_deeply_nested_unparse_does_not_crash -v`
Expected: CRASH (pytest dies with SIGSEGV) — run deliberately; this confirms the bug.

**Step 3: Implement**

`Cargo.toml`:

```toml
stacker = "0.1"
```

In `src/unparser.rs` rename the current `write_element` → `write_element_inner` (visibility `fn`, not `pub`) and add a wrapper:

```rust
    pub fn write_element(
        &mut self,
        py: Python,
        tag: &str,
        value: &Bound<'_, PyAny>,
        needs_newline: bool,
    ) -> PyResult<()> {
        // Grow the stack in heap-allocated segments: deeply nested input dicts
        // must not overflow the OS thread stack (that would kill the whole
        // Python process with SIGSEGV).
        stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
            self.write_element_inner(py, tag, value, needs_newline)
        })
    }
```

Keep the recursive calls inside `write_element_inner` and `write_dict_element` going through `self.write_element(...)` (via the wrapper — so the guard fires at every level).

**Step 4: Rebuild, run tests**

Run: `.venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q && cargo clippy --all-targets`
Expected: all PASS, clippy clean.

**Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/unparser.rs tests/test_unparse.py
git commit -m "fix: prevent stack overflow segfault on deeply nested unparse"
```

---

## Task 6: API honesty — errors instead of silent divergence

Unimplemented modes must raise `NotImplementedError` instead of silently producing a different result:
- `item_depth > 0` / `item_callback` (streaming is not implemented; `item_callback` is not even accepted right now → TypeError);
- `disable_entities=False` (DTD entities are not expanded);
- non-UTF-8 `encoding` (the kwarg is currently named `_encoding` — the reference call `parse(xml, encoding=...)` fails with TypeError).

**Files:**
- Modify: `src/lib.rs:150-211` (signature and start of `parse`)
- Test: `tests/test_parse_parameters.py`

**Step 1: Write the failing tests**

```python
def test_streaming_mode_not_implemented():
    with pytest.raises(NotImplementedError):
        xmltodict_rs.parse("<a><b>1</b></a>", item_depth=2, item_callback=lambda p, i: True)
    with pytest.raises(NotImplementedError):
        xmltodict_rs.parse("<a/>", item_depth=1)


def test_enabled_entities_not_implemented():
    with pytest.raises(NotImplementedError):
        xmltodict_rs.parse("<a/>", disable_entities=False)


def test_non_utf8_encoding_not_implemented():
    with pytest.raises(NotImplementedError):
        xmltodict_rs.parse("<a/>".encode("latin-1"), encoding="latin-1")


def test_utf8_encoding_accepted():
    for enc in (None, "utf-8", "UTF-8", "utf8"):
        assert xmltodict_rs.parse("<a>x</a>", encoding=enc) == {"a": "x"}
```

**Step 2: Run tests to verify failure**

Run: `.venv/bin/python -m pytest tests/test_parse_parameters.py -k "not_implemented or encoding_accepted" -v`
Expected: FAIL (TypeError instead of NotImplementedError; `encoding=` not accepted).

**Step 3: Implement**

In `src/lib.rs`, in `#[pyo3(signature)]` and the parameters:
- `_encoding = None` → `encoding = None` (type `Option<&str>`);
- add `item_callback = None` after `item_depth = 0` (type `Option<Py<PyAny>>`).

At the start of the `parse` body (before building the config):

```rust
    if item_depth > 0 || item_callback.is_some() {
        return Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
            "streaming mode (item_depth/item_callback) is not implemented in xmltodict_rs",
        ));
    }
    if !disable_entities {
        return Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
            "disable_entities=False (DTD entity expansion) is not implemented in xmltodict_rs",
        ));
    }
    if let Some(enc) = encoding {
        if !enc.eq_ignore_ascii_case("utf-8") && !enc.eq_ignore_ascii_case("utf8") {
            return Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
                format!("encoding '{enc}' is not supported, only UTF-8"),
            ));
        }
    }
```

Remove the `item_depth`/`disable_entities` fields from `ParseConfig` (together with `#[allow(dead_code)]`) — they no longer reach the parser; `process_comments` in the config is dead too (passed as a separate argument) — remove it as well. Update `Default for ParseConfig` and the construction site in `parse`.

**Step 4: Rebuild, run tests**

Run: `.venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q && cargo clippy --all-targets`
Expected: all PASS. Existing tests passing `item_depth=0` / `disable_entities=True` explicitly are unaffected.

**Step 5: Commit**

```bash
git add src/lib.rs src/config.rs tests/test_parse_parameters.py
git commit -m "feat: raise NotImplementedError for unimplemented modes instead of silent divergence"
```

---

## Task 7: Upgrade dependencies to current versions

Current versions as of 2026-08-12 (crates.io): **pyo3 0.29.2**, **quick-xml 0.41.0**, **memchr 2.8.3**, **mimalloc 0.1.52**, **stacker 0.1.25**. The main work is the quick-xml 0.31 → 0.41 migration (Reader config and the error enum were restructured). This task comes after the bug fixes so the differential test base already contains the regression cases. Rust toolchain 1.91.0 and `maturin-action@v1` in CI are compatible.

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`
- Modify: `src/lib.rs` (Reader config), `src/error.rs` (`map_quick_xml_error`), spot fixes wherever the compiler points
- Modify: `pyproject.toml` (`build-system.requires`, dev-group maturin)

**Step 0: Benchmark baseline (pre-upgrade build)**

Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-before-t7.json` — numbers into the ledger.

**Step 1: Bump versions**

`Cargo.toml`:

```toml
[dependencies]
mimalloc = { version = "0.1.52", optional = true, features = ["local_dynamic_tls"] }
pyo3 = { version = "0.29", features = ["extension-module", "generate-import-lib"] }
quick-xml = "0.41"
memchr = { version = "2.8", default-features = false }
stacker = "0.1"
```

Note: drop the `serialize` feature of quick-xml — serde is not used in the crate (verify: `grep -rn serde src/` is empty).

`pyproject.toml`: `requires = ["maturin>=1.14"]` in `[build-system]`; `"maturin>=1.14"` in the dev group.

**Step 2: Compiler-driven migration**

Run: `cargo check 2>&1 | head -50` — fix down the list. Known spots:

1. **Reader config** (`src/lib.rs:81-85`): the `trim_text`/`check_end_names`/`check_comments`/`expand_empty_elements` methods on `Reader` are gone, settings moved into `Config`:

```rust
    let mut xml_reader = Reader::from_reader(reader);
    let reader_config = xml_reader.config_mut();
    reader_config.trim_text(strip_whitespace);
    reader_config.check_end_names = true;
    reader_config.check_comments = true;
    reader_config.expand_empty_elements = true;
```

(with `expand_empty_elements = true` the `Event::Empty` branch never fires — keep it, the code of both branches stays correct).

2. **Error enum** (`src/error.rs:68-86`): the variant set of `quick_xml::Error` was restructured (`Syntax(SyntaxError)`, `IllFormed(IllFormedError)`, `Io(Arc<io::Error>)` etc. — the compiler will give the exact list; wildcard is banned by the lint). The mapping rule stays: the `Io` arm extracts `WrappedPyErr` via `pyerr_from_io(&io_err)` (mind the `Arc`: pass `&io_err`), every other variant → `expat_error(py, err.to_string())`. Also check the error types of `attributes()` / `unescape()` / `unescape_value()` — signatures may have moved to subtypes (`AttrError`, `EscapeError`); fix conversions per compiler hints.

3. **pyo3 0.26 → 0.29**: the Bound API is stable, expect mostly deprecation warnings (the project has `warnings = "deny"` — fix immediately per hints). Check that `#[pymodule(gil_used = false)]` and `Python::attach` are still current; if `gil_used` became default/deprecated — drop the parameter.

**Step 3: Full verification**

Run: `cargo clippy --all-targets && cargo test && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: all green. Watch out — parse error texts may differ (quick-xml formats messages differently): tests compare the exception **type**, not the text; if some test compared text — relax it to the type.

Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-after-t7.json && .venv/bin/python benches/perf_gate.py compare /tmp/perf-before-t7.json /tmp/perf-after-t7.json` — no SLOWER verdicts allowed (quick-xml 0.41 must not regress); numbers go into the ledger.

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock pyproject.toml src/
git commit -m "chore: upgrade pyo3 to 0.29, quick-xml to 0.41, memchr to 2.8"
```

---

## Task 8: panic=unwind + drop unsafe Sync + dead code + classifier

Mechanical cleanup, no behavior change — existing tests cover it.

**Files:**
- Modify: `Cargo.toml` (remove `panic = "abort"`)
- Modify: `src/error.rs:32-35` (remove `unsafe impl Sync`, fix the comment)
- Modify: `src/config.rs:212-326` (delete `ParseConfig::builder` and `ParseConfigBuilder`)
- Modify: `pyproject.toml:12` (remove the `Python :: 3.9` classifier)

**Step 1: Cargo.toml**

Remove the `panic = "abort"` line from `[profile.release]`. Rationale: with abort, a panic in a dependency kills the whole Python process; with unwind, PyO3 converts it into a catchable `pyo3_runtime.PanicException`.

**Step 2: error.rs**

Delete `unsafe impl Sync for WrappedPyErr {}` together with its SAFETY comment. Replace the doc comment on `WrappedPyErr`:

```rust
/// Wrapper to store `PyErr` inside `io::Error` while preserving the original
/// exception type. `PyErr` is already `Send + Sync` in pyo3 0.26+, so the
/// wrapper needs no unsafe impls.
```

**Step 3: config.rs, pyproject.toml**

Delete `impl ParseConfig { builder() }` and the whole `ParseConfigBuilder` (zero callers in the crate). Delete the `"Programming Language :: Python :: 3.9",` line from classifiers (contradicts `requires-python = ">=3.10"`).

**Step 4: Verify**

Run: `cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: clippy clean (incl. no new dead_code), all tests PASS.

**Step 5: Commit**

```bash
git add Cargo.toml src/error.rs src/config.rs pyproject.toml
git commit -m "refactor: unwind panics, drop redundant unsafe Sync, remove dead builder"
```

---

## Task 9: Collapse config.rs newtypes into a macro

4 identical newtypes × ~65 lines → one `macro_rules!`. No behavior change.

**Files:**
- Modify: `src/config.rs:1-167`

**Step 1: Implement**

Replace the four blocks (`AttrPrefix`, `CdataKey`, `CommentKey`, `NamespaceSeparator`) with:

```rust
macro_rules! config_newtype {
    ($(#[$doc:meta])* $name:ident, $default:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self($default.to_owned())
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

config_newtype!(
    /// Attribute prefix (e.g., "@" for "@id", "@class")
    AttrPrefix, "@"
);
config_newtype!(
    /// Key for text content (e.g., "#text")
    CdataKey, "#text"
);
config_newtype!(
    /// Key for comment content (e.g., "#comment")
    CommentKey, "#comment"
);
config_newtype!(
    /// Separator between namespace and local name (e.g., ":")
    NamespaceSeparator, ":"
);
```

Of the four `PartialEq` impls on `CdataKey`, keep only the actually used one (`key_str == self.config.cdata_key` in `unparser.rs:185`, where `key_str: String`):

```rust
impl PartialEq<CdataKey> for String {
    fn eq(&self, other: &CdataKey) -> bool {
        *self == other.0
    }
}
```

If clippy/the compiler points at other used comparisons — keep those too, do not add unused ones.

**Step 2: Verify**

Run: `cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: clean, all PASS.

**Step 3: Commit**

```bash
git add src/config.rs
git commit -m "refactor: collapse config newtypes into a macro"
```

---

## Task 10: Deduplicate the readers

`file_like.rs` and `generator.rs` duplicate the "copy bytes into out, stash the tail into pending" block (3 copies of ~18 lines).

**Files:**
- Modify: `src/reader/pending.rs`, `src/reader/file_like.rs`, `src/reader/generator.rs`

**Step 1: Add helper to PendingBytes**

In `src/reader/pending.rs`:

```rust
    /// Copy as much of `bytes` as fits into `out`; stash the tail as pending.
    /// Returns the number of bytes written into `out`.
    pub fn write_chunk(&mut self, bytes: &[u8], out: &mut [u8]) -> usize {
        let to_copy = bytes.len().min(out.len());
        let (head, tail) = bytes.split_at(to_copy);
        if let Some(dst) = out.get_mut(..to_copy) {
            dst.copy_from_slice(head);
        }
        if !tail.is_empty() {
            self.fill_from_slice(tail);
        }
        to_copy
    }
```

**Step 2: Use it**

In `file_like.rs` replace the tail of `read` (lines 70-87) with:

```rust
            Ok(self.pending.write_chunk(bytes, out))
```

In `generator.rs` — same for both places (lines 123-140 for the str branch: `Ok(self.pending.write_chunk(text.as_bytes(), out))`; lines 169-186 for the bytes branch). Simplify the `bytearray_buffer` branches with the unreachable `else`:

```rust
            } else if let Ok(chunk_bytearray) = chunk.downcast::<PyByteArray>() {
                self.bytearray_buffer = Some(chunk_bytearray.to_vec());
                self.bytearray_buffer.as_deref().unwrap_or(&[])
            } else {
```

**Step 3: Verify**

Run: `cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/test_parse_input.py -q && .venv/bin/python -m pytest tests/ -q`
Expected: clean, all PASS (test_parse_input.py covers file-like and generators, including partial reads).

**Step 4: Commit**

```bash
git add src/reader/
git commit -m "refactor: deduplicate chunk-copy logic in Python readers"
```

---

## Task 11: Perf — escape without unsafe, via memchr3_iter

Removes 4 unsafe blocks, the byte-by-byte loop and the 6x over-allocation. Unit tests already exist in the module (+ the one added in Task 3). The task is justified by unsafe removal even at zero perf delta, but the benchmark is mandatory — a regression is unacceptable.

**Files:**
- Modify: `src/escape.rs:13-66` (`escape_xml`)

**Step 0: Benchmark baseline**

Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-before-t11.json` — numbers into the ledger (if Task 7 already measured `/tmp/perf-after-t7.json`, reuse it as the before-point).

**Step 1: Add edge-case unit tests first**

```rust
    #[test]
    fn test_escape_xml_all_special() {
        assert_eq!("&amp;&lt;&gt;", escape_xml("&<>"));
    }

    #[test]
    fn test_escape_xml_multibyte_around_special() {
        assert_eq!("привет &amp; мир", escape_xml("привет & мир"));
    }

    #[test]
    fn test_escape_xml_empty() {
        assert_eq!("", escape_xml(""));
    }
```

Run: `cargo test` — they must PASS on the old implementation (equivalence safety net).

**Step 2: Rewrite**

```rust
pub fn escape_xml(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let mut positions = memchr::memchr3_iter(AMPERSAND, LT, GT, bytes).peekable();
    if positions.peek().is_none() {
        return Cow::Borrowed(text);
    }

    let mut result = String::with_capacity(text.len() + 24);
    let mut last = 0;
    for pos in positions {
        // memchr positions are on ASCII bytes, hence valid char boundaries
        if let Some(chunk) = text.get(last..pos) {
            result.push_str(chunk);
        }
        let escaped = match bytes.get(pos) {
            Some(&AMPERSAND) => ESCAPED_AMP,
            Some(&LT) => ESCAPED_LT,
            _ => ESCAPED_GT,
        };
        result.push_str(escaped);
        last = pos + 1;
    }
    if let Some(tail) = text.get(last..) {
        result.push_str(tail);
    }
    Cow::Owned(result)
}
```

Remove the now-unused `from_raw_parts` / `from_utf8_unchecked` imports.

**Step 3: Verify + benchmark**

Run: `cargo test && cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Expected: all PASS, no unsafe left in the crate at all.

Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-after-t11.json && .venv/bin/python benches/perf_gate.py compare /tmp/perf-before-t11.json /tmp/perf-after-t11.json` — delta into the ledger. A SLOWER verdict on any unparse case is unacceptable (escape is on the unparse hot path); noise verdicts are acceptable (the task is justified by unsafe removal).

**Step 4: Commit**

```bash
git add src/escape.rs
git commit -m "perf: rewrite escape_xml with memchr iterator, drop unsafe"
```

---

## Task 12: Perf — apply_postprocessor without per-element allocation

Currently `key.to_owned()` (a String allocation) runs for every element/attribute even without a postprocessor.

**Files:**
- Modify: `src/parser.rs:104-128` (`apply_postprocessor`) and all call sites (`push_data`, `start_element`, `end_element`)

**Step 1: Benchmark baseline**

Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-before-t12.json` (reuse `/tmp/perf-after-t11.json` if fresh).

**Step 2: Implement**

```rust
    #[inline]
    fn apply_postprocessor<'a, 'py>(
        &self,
        py: Python<'py>,
        key: &'a str,
        data: &Bound<'py, PyAny>,
    ) -> PyResult<Option<(std::borrow::Cow<'a, str>, Bound<'py, PyAny>)>> {
        let Some(proc) = &self.postprocessor else {
            return Ok(Some((std::borrow::Cow::Borrowed(key), data.clone())));
        };

        let path_list = PyList::new(py, &self.path)?;
        let result = proc.call1(py, (path_list, key, data))?;

        if result.is_none(py) {
            return Ok(None);
        }

        let tuple = result.bind(py).downcast::<PyTuple>()?;
        let final_key = tuple.get_item(0)?.extract::<String>()?;
        let final_value = tuple.get_item(1)?;
        Ok(Some((std::borrow::Cow::Owned(final_key), final_value)))
    }
```

Call sites: replace `final_key.as_str()` → `final_key.as_ref()`; `item.set_item(final_key, ...)` → `item.set_item(final_key.as_ref(), ...)` (PyO3 accepts `&str` as a key). The compiler will point out all the spots.

**Step 3: Verify + benchmark**

Run: `cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-after-t12.json && .venv/bin/python benches/perf_gate.py compare /tmp/perf-before-t12.json /tmp/perf-after-t12.json` — record the deltas in the ledger. No SLOWER verdicts allowed.

**Step 4: Commit**

```bash
git add src/parser.rs
git commit -m "perf: avoid per-element key allocation when postprocessor is absent"
```

---

## Task 13: Perf — namespace stack and repeated build_name

Two items in one task (both in `start_element`/`end_element`):
1. `namespace_stack.last().cloned()` clones a HashMap for every element even with `process_namespaces=false` — maintain the stack only when namespaces are on.
2. `end_element` recomputes `build_name(name)` although the same value sits in `path` and gets discarded — use `path.pop()`.

**Files:**
- Modify: `src/parser.rs` (`start_element`, `end_element`), `src/lib.rs` (call sites of `end_element`)

**Step 1: Implement — lazy namespace stack**

In `start_element` wrap the stack work:

```rust
        let mut current_ns_map = if self.config.process_namespaces {
            self.namespace_stack.last().cloned().unwrap_or_default()
        } else {
            HashMap::new()
        };
```

(creating an empty HashMap without inserts does not allocate), and make the push conditional:

```rust
        if self.config.process_namespaces {
            self.namespace_stack.push(current_ns_map);
        }
```

In `end_element` make the pop conditional and do not treat an empty stack as an error when NS are off:

```rust
        if self.config.process_namespaces && self.namespace_stack.pop().is_none() {
            return Err(expat_error(py, "unexpected closing tag".to_owned()));
        }
```

**Step 2: Implement — reuse path.pop()**

In `end_element` delete `let element_name = self.build_name(name);` and the `let Some(_) = self.path.pop()` block; instead:

```rust
        let Some(element_name) = self.path.pop() else {
            return Err(expat_error(py, "unexpected closing tag".to_owned()));
        };
```

(NB: the pop must run in the same pop-sequence position as before — after `stack.pop()` and `text_stack.pop()` the order does not matter, what matters is that all three run.) The `name: &str` parameter of `end_element` becomes unused — remove it from the signature and from both call sites in `lib.rs` (`parser.end_element(py)?`); keep the `validate_element_name` check in `lib.rs` as is. Open/close tag-name matching is guaranteed by quick-xml's `check_end_names(true)`.

**Step 3: Verify + benchmark**

Run: `cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Namespace tests: `.venv/bin/python -m pytest tests/test_parse_namespaces.py -v` — special attention.
Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-after-t13.json && .venv/bin/python benches/perf_gate.py compare /tmp/perf-after-t12.json /tmp/perf-after-t13.json` — deltas into the ledger. No SLOWER verdicts allowed.

**Step 4: Commit**

```bash
git add src/parser.rs src/lib.rs
git commit -m "perf: skip namespace stack without namespaces, reuse popped path name"
```

---

## Task 14: Perf — PyString interning for keys (benchmark-gated)

Every tag creates a new `PyString` per element; distinct names are usually few. A `HashMap<String, Py<PyString>>` cache lives in `XmlParser` (one parse call). **Gate: keep only if the benchmark shows ≥5% on parse; otherwise revert.**

**Files:**
- Modify: `src/parser.rs`

**Step 1: Implement**

Add a `key_cache: HashMap<String, Py<pyo3::types::PyString>>` field to `XmlParser` (initialize with `HashMap::new()` in `new`). Method:

```rust
    fn intern_key<'py>(&mut self, py: Python<'py>, key: &str) -> Bound<'py, pyo3::types::PyString> {
        if let Some(cached) = self.key_cache.get(key) {
            return cached.bind(py).clone();
        }
        let s = pyo3::types::PyString::new(py, key);
        self.key_cache.insert(key.to_owned(), s.clone().unbind());
        s
    }
```

Use it in `push_data` (`item.set_item(self.intern_key(py, final_key.as_ref()), ...)`) and in `start_element` for attribute keys. `push_data` and `comment` will need `&mut self` — adjust signatures (all callers are already in `&mut self` contexts).

**Step 2: Verify + benchmark gate**

Run: `cargo clippy --all-targets && .venv/bin/maturin develop --release && .venv/bin/python -m pytest tests/ -q`
Run: `.venv/bin/python benches/perf_gate.py measure /tmp/perf-after-t14.json && .venv/bin/python benches/perf_gate.py compare /tmp/perf-after-t13.json /tmp/perf-after-t14.json`
Gate: keep only if at least medium-parse and large-parse report FASTER with delta ≥ 5%; otherwise `git checkout -- src/parser.rs`, mark the task `skipped (no win)` in the ledger.

**Step 3: Commit (if the gate passed)**

```bash
git add src/parser.rs
git commit -m "perf: intern repeated dict key strings during parse"
```

---

## Task 15: Test gaps — property-based roundtrip + deep parse

**Files:**
- Modify: `pyproject.toml` (dev group), Create: `tests/test_property_roundtrip.py`, Modify: `tests/test_parse_special.py`

**Step 1: Add hypothesis dependency**

In `pyproject.toml`, add `"hypothesis>=6.100",` to `[dependency-groups] dev`, then `uv sync --group dev` (or `.venv/bin/pip install hypothesis`).

**Step 2: Write property tests**

`tests/test_property_roundtrip.py`:

```python
"""Property-based differential tests against reference xmltodict."""

import hypothesis.strategies as st
import xmltodict
from hypothesis import given, settings

import xmltodict_rs

tag_names = st.from_regex(r"[a-z][a-z0-9_]{0,8}", fullmatch=True)
text_values = st.text(
    alphabet=st.characters(codec="utf-8", exclude_categories=("Cs", "Cc")),
    min_size=1,
    max_size=20,
)

xml_values = st.recursive(
    text_values | st.none(),
    lambda children: st.dictionaries(tag_names, children, min_size=1, max_size=4),
    max_leaves=15,
)


@settings(max_examples=200, deadline=None)
@given(tag=tag_names, value=xml_values)
def test_unparse_matches_reference(tag, value):
    doc = {tag: value}
    assert xmltodict_rs.unparse(doc) == xmltodict.unparse(doc)


@settings(max_examples=200, deadline=None)
@given(tag=tag_names, value=xml_values)
def test_roundtrip_matches_reference(tag, value):
    doc = {tag: value}
    xml = xmltodict.unparse(doc)
    assert xmltodict_rs.parse(xml) == xmltodict.parse(xml)
```

**Step 3: Deep parse test**

In `tests/test_parse_special.py`:

```python
def test_deeply_nested_parse():
    depth = 50_000
    xml = "<x>" * depth + "</x>" * depth
    result = xmltodict_rs.parse(xml)
    for _ in range(depth - 1):
        result = result["x"]
    assert result == {"x": None}
```

**Step 4: Run**

Run: `.venv/bin/python -m pytest tests/test_property_roundtrip.py tests/test_parse_special.py -q`
Expected: PASS. If hypothesis finds a divergence — that is a new bug: capture the minimal example as a separate regression test, check the behavior against the reference, fix the implementation (not the test), then continue.

**Step 5: Commit**

```bash
git add pyproject.toml tests/test_property_roundtrip.py tests/test_parse_special.py uv.lock
git commit -m "test: add property-based roundtrip tests and deep-nesting coverage"
```

---

## Task 17: CI/CD update and improvements

Execution order: after Task 15, BEFORE Task 16 (the final gate must see the updated pipeline). Numbered 17 to avoid renumbering the already-executed tasks.

Bring `.github/workflows/CI.yml` in line with the changes from Tasks 0-15 and harden the pipeline. Inspect the current workflow first and adapt — the items below are requirements, not literal patches.

**Files:**
- Modify: `.github/workflows/CI.yml` (and any sibling workflow files that reference the same tooling)

**Step 1: Inventory**

Read `.github/workflows/CI.yml`. Note: toolchain setup steps, maturin-action usage, python version matrix, which lint/test gates exist, caching.

**Step 2: Requirements**

1. **Toolchain**: pyo3 0.29 / quick-xml 0.41 may raise MSRV; `rust-toolchain.toml` pins 1.91.0. CI must build with the pinned toolchain (e.g. `dtolnay/rust-toolchain` reading `rust-toolchain.toml`), not an unpinned `@master` default.
2. **Gates**: a job must run `cargo fmt --check`, `cargo clippy --all-targets` (deny-warnings lives in Cargo.toml), `cargo test`, plus `ruff check .` and `ruff format --check .` (via uv). Add whatever is missing.
3. **maturin**: pyproject now requires `maturin>=1.14` — verify maturin-action resolves a compatible maturin on every matrix target; pin only if resolution fails.
4. **Tests**: pytest steps must install the dependency groups including `hypothesis` (added in Task 15) so property tests run in CI; keep the free-threaded 3.13t/3.14t jobs and the platform matrix intact.
5. **Caching**: add Rust build caching (`Swatinem/rust-cache` or actions/cache over ~/.cargo + target/) to lint/test jobs where it shortens runs; do not cache into release wheel builds if it risks stale artifacts.
6. **Benchmarks stay OUT of CI** — shared runners are too noisy for perf gating (see Task 0 rationale); if tempted, leave a YAML comment saying why not.

**Step 3: Validate**

`actionlint` if available (`brew install actionlint` or `npx actionlint`); otherwise at minimum parse-check the YAML and self-review the matrix consistency. Note in the report which validation ran.

**Step 4: Commit**

```bash
git add .github/workflows/
git commit -m "ci: pin toolchain, add lint gates and rust caching after review fixes"
```

---

## Task 16: Final verification

**Step 1: Full gate**

```bash
cargo fmt --check && cargo clippy --all-targets && cargo test
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests/ -q
.venv/bin/python benches/accurate_benchmark.py
```

Expected: all green. Additionally run `.venv/bin/python benches/perf_gate.py measure /tmp/perf-final.json && .venv/bin/python benches/perf_gate.py compare /tmp/perf-baseline.json /tmp/perf-final.json` — the final build must show no SLOWER verdicts vs the Task 0 baseline; record the final numbers in the ledger. `accurate_benchmark.py` output is recorded only as the README-facing speedup vs xmltodict.

**Step 2: Manual smoke of fixed bugs**

```bash
.venv/bin/python -c "
import xmltodict_rs as rs, xmltodict as ref, io
pp = lambda p, k, d: (k, int(d)) if k == 'i' and isinstance(d, str) else (k, d)
assert rs.parse('<r><i>1</i><i>2</i><i>3</i></r>', postprocessor=pp) == {'r': {'i': [1, 2, 3]}}
try: rs.parse('<a>1</a>junk'); raise SystemExit('junk accepted!')
except Exception as e: assert type(e).__name__ == 'ExpatError'
assert rs.unparse({'r': {'@a': 'x\ny'}}, full_document=False) == ref.unparse({'r': {'@a': 'x\ny'}}, full_document=False)
buf = io.StringIO(); assert rs.unparse({'a': '1'}, output=buf) is None and buf.getvalue()
print('smoke OK')
"
```

**Step 3: Optimization results**

Fill in the "Optimization results" section of the ledger: for every perf task (7, 11, 12, 13, 14) — what changed and the per-case deltas in percent (from the perf_gate JSONs recorded in "Benchmarks"); plus a total row "overall vs the Task 0 baseline" (`compare /tmp/perf-baseline.json /tmp/perf-final.json`). Format:

```markdown
| Task | What was optimized | parse | unparse |
|------|--------------------|-------|---------|
| 12   | apply_postprocessor without per-element String allocation | -X% | ~0 |
| ...  | ... | ... | ... |
| **Total** | | **-X%** | **-Y%** |
```

If the overall delta is notable — update the "Performance" table in README (measured via the `benches/accurate_benchmark.py` methodology).

**Step 4: Update ledger, update README known-limitations**

If README has no limitations section — add a short one: streaming (`item_depth`), `disable_entities=False`, non-UTF-8 encoding → `NotImplementedError`. Update the "Known limitations" item in `CLAUDE.md` (self-maintenance protocol).

**Step 5: Commit**

```bash
git add README.md CLAUDE.md docs/plans/
git commit -m "docs: document known limitations and optimization results"
```

---

## Out of scope (recorded, deliberately not done)

- Streaming `item_depth`/`item_callback` — a separate feature, not a bug fix.
- Non-UTF-8 encoding support (quick-xml `encoding` feature) — a separate feature.
- The reference's `expat=` kwarg — not accepted (TypeError); acceptable.
- Single-pass attribute writes into the dict in `start_element` and `SmallVec`/single-String for `text_stack` — micro-perf, revisit if Tasks 12-14 miss the goal.
- `should_force_list`: swallowing an exception from `__contains__` — cosmetic, behavior acceptable.
