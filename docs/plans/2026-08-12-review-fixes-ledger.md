# Ledger: xmltodict_rs review fixes

Plan: [2026-08-12-review-fixes.md](2026-08-12-review-fixes.md)
Started: 2026-08-12. Rules: one task — one commit; update the status right after the commit; record any deviation from the plan with a reason in Notes.

Statuses: `pending` → `in_progress` → `done` | `skipped (<reason>)` | `blocked (<reason>)`

## Tasks

| # | Task | Priority | Status | Commit | Notes |
|---|------|----------|--------|--------|-------|
| 0 | Benchmark gate tool (benches/perf_gate.py) | high | done | 35d88af, 64bff36 | REPEATS=10 |
| 1 | Bug: postprocessor lost from the 3rd repeated element on | critical | done | 37ab8bb | |
| 2 | Bug: junk before/after the root element is accepted | critical | done | b513313 | |
| 3 | Bug: `\n`/`\t`/`\r` in attributes are not escaped | critical | done | 92e2c29 | |
| 4 | Bug: `unparse(output=...)` is ignored | high | done | ec8d207 | .pyi+README synced |
| 5 | Bug: segfault on deeply nested unparse (stacker) | critical | done | e3bc454 | |
| 6 | API honesty: NotImplementedError instead of silent divergence | high | done | e0a9ffa | encoding renamed, item_callback added; .pyi+README synced |
| 7 | Dependency upgrade: pyo3 0.29, quick-xml 0.41, memchr 2.8 | high | done | 749bbb4, ca2ede1 | quick-xml 0.41 splits entity/char refs into `Event::GeneralRef`; reader-level `trim_text` dropped, whitespace stripping moved to `XmlParser::end_element` on the joined text; ca2ede1 adds regression tests for the discovered edge case (see Notes below) |
| 8 | panic=unwind, drop unsafe Sync, dead builder, classifier | medium | done | eebf3c7 | |
| 9 | Macro for config.rs newtypes | low | done | 91a403b | config.rs 212->108 lines |
| 10 | Reader deduplication (`PendingBytes::write_chunk`) | low | done | 3bb7a6d | -55 lines |
| 11 | Perf: escape via memchr3_iter, no unsafe | medium | done | 6bf8c8b | zero unsafe left in crate |
| 12 | Perf: apply_postprocessor without allocation (Cow) | medium | done | bf5b466 | |
| 13 | Perf: lazy namespace stack + reuse path.pop() | medium | done | f2031f1 | |
| 14 | Perf: PyString interning (gate: ≥5% on parse) | low | skipped (no win) | — | measured: medium-parse -1.5% (noise), large-parse -3.3% (<5%); reverted per gate |
| 15 | Tests: hypothesis roundtrip + deep parse | high | done | bea6b7f | 0 divergences in 400 examples |
| 17 | CI/CD update and improvements (runs before Task 16) | high | done | 861b4f4 | actionlint clean; release path untouched |
| 16 | Final verification + README known-limitations | high | done | (this commit) | perf_gate: all 6 cases FASTER vs Task 0 baseline, no SLOWER; full gate + smoke green |

## Benchmarks

Gate numbers come from `benches/perf_gate.py` (per-case median_us + spread, JSON in /tmp, printed numbers copied here); `accurate_benchmark.py` speedups vs xmltodict are recorded only as README-facing numbers. Run-to-run P50 drift of accurate_benchmark.py reaches ~9% on identical code (measured 2026-08-13) — never use it as a gate.

| Checkpoint | parse | unparse | Comment |
|------------|-------|---------|---------|
| README baseline | 6.23x | 8.76x | avg speedup vs xmltodict, accurate_benchmark.py, 2026-08-13 (README-facing only) |
| README final (Task 16) | 6.08x | 10.07x | avg speedup vs xmltodict, accurate_benchmark.py, 2026-08-13; parse delta is within the ~9% run-to-run drift noted above, unparse improved; per-size breakdown: parse 7.80x/5.57x/4.87x, unparse 9.66x/10.75x/9.80x (small/medium/large) |
| perf_gate baseline (Task 0) | 1.25us (0.0%), 61.52us (1.4%), 404.73us (1.6%) | 0.62us (0.0%), 26.94us (1.1%), 188.10us (2.5%) | /tmp/perf-baseline.json; small-parse, medium-parse, large-parse (left); small-unparse, medium-unparse, large-unparse (right); values with spread % |
| Before Task 7 (this run) | 1.21us (6.9%), 63.02us (1.4%), 399.75us (1.8%) | 0.63us (0.0%), 27.50us (0.9%), 191.81us (1.1%) | /tmp/perf-before-t7.json |
| After Task 7 (upgrade) | 1.12us (3.6%), 58.38us (1.1%), 368.40us (1.6%) | 0.54us (0.2%), 23.40us (1.2%), 157.94us (1.6%) | /tmp/perf-after-t7.json; all 6 cases FASTER (-6.9% .. -17.7%), no SLOWER |
| After Task 11 | noise | noise | perf_gate: all 6 cases noise (0.0..+0.4%); justified by unsafe removal |
| After Task 12 | 1.12us (3.6%), 57.33us (1.5%), 360.05us (1.7%) | 0.54us (0.0%), 23.60us (3.4%), 156.56us (1.2%) | /tmp/perf-after-t12.json; vs /tmp/perf-before-t12.json (=perf-after-t11.json): medium-parse -4.3% FASTER, large-parse -4.4% FASTER, rest noise; no SLOWER. Baseline of the perf series for Task 13/14. |
| After Task 13 | 1.04us (0.1%), 54.44us (1.8%), 345.42us (1.5%) | 0.54us (0.0%), 23.08us (3.1%), 155.60us (2.3%) | /tmp/perf-after-t13.json; vs /tmp/perf-after-t12.json: small-parse -7.4% FASTER, medium-parse -5.1% FASTER, large-parse -4.1% FASTER, unparse cases noise; no SLOWER. |
| After Task 14 | gate failed | — | interning gave medium -1.5% (noise), large -3.3% (<5%) — reverted; state = Task 13 |
| Final (Task 16) | 1.04us (3.9%), 53.50us (1.8%), 338.52us (2.4%) | 0.54us (0.2%), 23.08us (2.2%), 155.81us (3.2%) | /tmp/perf-final.json; vs /tmp/perf-baseline.json (Task 0): all 6 cases FASTER (-13.0% .. -17.2%), no SLOWER |

Reference point from the review (Python 3.14, macOS arm64, 20k elements): parse 15.5 ms vs 76.8 ms (xmltodict), unparse 6.7 ms vs 64.8 ms.

## Optimization results

_(filled in during Task 16 from the "Benchmarks" table; every perf task must have a before/after measurement — an optimization without a measurement is not accepted)_

| Task | What was optimized | parse | unparse |
|------|--------------------|-------|---------|
| 7 | quick-xml 0.31 → 0.41 upgrade | -7.4% (medium), -7.8% (large) | -14.9% (medium), -17.7% (large) |
| 11 | escape via memchr3_iter, no unsafe | noise (all 3 cases, ~0%); justified by unsafe removal, not perf | noise (all 3 cases, ~0%); justified by unsafe removal, not perf |
| 12 | apply_postprocessor without per-element allocation (Cow) | -4.3% (medium), -4.4% (large), noise (small) | noise (all 3 cases) |
| 13 | lazy namespace stack (skip when `process_namespaces` is off) + reuse `path.pop()` result | -7.4% (small), -5.1% (medium), -4.1% (large) | noise (all 3 cases) |
| 14 | `PyString` interning — gated on ≥5% parse win | skipped: medium -1.5% (noise), large -3.3% (below the 5% gate) — reverted, no change shipped | not measured (gate failed on parse) |
| **Total vs Task 0 baseline** | `/tmp/perf-baseline.json` → `/tmp/perf-final.json`, all 6 cases FASTER, no SLOWER | **-16.7% (small), -13.0% (medium), -16.4% (large)** | **-13.4% (small), -14.3% (medium), -17.2% (large)** |

## Deviations found during execution

- Task 0: raised REPEATS from 5 to 10 during self-validation (identical-code runs showed false FASTER/SLOWER verdicts at 5 and 8 repeats; clean noise all cases at 10). Final perf_gate config: REPEATS=10, LOOP_SECONDS=1.0, WARMUP_SECONDS=0.5, NOISE_FLOOR_PCT=2.0.
- Task 7: quick-xml 0.41 introduced `Event::GeneralRef` (not present in 0.31) — entity/char refs (`&amp;`, `&#65;`, ...) now arrive as standalone events instead of being embedded in `Event::Text`. Required unplanned work beyond the brief's checklist: (1) add an `Event::GeneralRef` match arm in `parse_xml_with_reader` resolving predefined entities + numeric char refs via `resolve_general_ref`; (2) drop reader-level `trim_text` entirely (previously `trim_text(strip_whitespace)`) because per-event trimming now eats whitespace adjacent to every entity; (3) move `strip_whitespace` handling into `XmlParser::end_element`, trimming the fully-joined element text once, matching xmltodict's own `text.strip()` on its joined buffer. Verified against real xmltodict for interior-whitespace-around-child-element and interior-whitespace-around-entity cases (both previously untested edge cases) — see task-7-report.md for the comparison script. `Attribute::unescape_value` (deprecated) replaced with `normalized_value(XmlVersion::Implicit1_0)`, which additionally normalizes literal `\t`/`\r`/`\n` in attribute values to spaces per the XML spec (expat does this too; no test regression observed). `pyo3` 0.29 renamed `downcast*` → `cast*` (returns `CastError`, converts to `PyErr` via `From`) and dropped `From<std::str::Utf8Error> for PyErr` (replaced with a local `utf8_str` helper producing the same `PyUnicodeDecodeError`).
- Task 7: local `.venv` had maturin 1.9.3 pinned in `uv.lock` despite `pyproject.toml` requiring `>=1.9.3` pre-upgrade; ran `uv lock && uv sync` to pick up maturin 1.14.1 matching the new `>=1.14` requirement (uv.lock diff is part of this change but excluded from the dependency-upgrade commit per the task's file list).

## Out of scope (from the plan, do not lose)

- Streaming `item_depth`/`item_callback` — a separate feature.
- Non-UTF-8 encodings (quick-xml `encoding` feature) — a separate feature.
- The reference's `expat=` kwarg — not accepted (TypeError), acceptable.
- Single-pass attributes in `start_element`, `SmallVec` for `text_stack` — if perf goals are missed by Tasks 12-14.
- `should_force_list`: swallowing an exception from `__contains__` — cosmetic.
