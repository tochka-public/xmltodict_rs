# Ledger: xmltodict_rs review fixes

Plan: [2026-08-12-review-fixes.md](2026-08-12-review-fixes.md)
Started: 2026-08-12. Rules: one task — one commit; update the status right after the commit; record any deviation from the plan with a reason in Notes.

Statuses: `pending` → `in_progress` → `done` | `skipped (<reason>)` | `blocked (<reason>)`

## Tasks

| # | Task | Priority | Status | Commit | Notes |
|---|------|----------|--------|--------|-------|
| 1 | Bug: postprocessor lost from the 3rd repeated element on | critical | pending | — | |
| 2 | Bug: junk before/after the root element is accepted | critical | pending | — | |
| 3 | Bug: `\n`/`\t`/`\r` in attributes are not escaped | critical | pending | — | |
| 4 | Bug: `unparse(output=...)` is ignored | high | pending | — | |
| 5 | Bug: segfault on deeply nested unparse (stacker) | critical | pending | — | |
| 6 | API honesty: NotImplementedError instead of silent divergence | high | pending | — | |
| 7 | Dependency upgrade: pyo3 0.29, quick-xml 0.41, memchr 2.8 | high | pending | — | |
| 8 | panic=unwind, drop unsafe Sync, dead builder, classifier | medium | pending | — | |
| 9 | Macro for config.rs newtypes | low | pending | — | |
| 10 | Reader deduplication (`PendingBytes::write_chunk`) | low | pending | — | |
| 11 | Perf: escape via memchr3_iter, no unsafe | medium | pending | — | |
| 12 | Perf: apply_postprocessor without allocation (Cow) | medium | pending | — | |
| 13 | Perf: lazy namespace stack + reuse path.pop() | medium | pending | — | |
| 14 | Perf: PyString interning (gate: ≥5% on parse) | low | pending | — | |
| 15 | Tests: hypothesis roundtrip + deep parse | high | pending | — | |
| 16 | Final verification + README known-limitations | high | pending | — | |

## Benchmarks

Record the output of `benches/accurate_benchmark.py` (parse/unparse medians) at the checkpoints:

| Checkpoint | parse | unparse | Comment |
|------------|-------|---------|---------|
| Baseline (before Task 1) | — | — | measure before starting |
| After Task 7 (upgrade) | — | — | quick-xml 0.41 must not regress |
| After Task 11 | — | — | zero delta acceptable (unsafe removal) |
| After Task 12 | — | — | baseline of the perf series |
| After Task 13 | — | — | |
| After Task 14 | — | — | gate: ≥5% vs Task 13, otherwise revert |
| Final (Task 16) | — | — | not worse than the Task 12 baseline |

Reference point from the review (Python 3.14, macOS arm64, 20k elements): parse 15.5 ms vs 76.8 ms (xmltodict), unparse 6.7 ms vs 64.8 ms.

## Optimization results

_(filled in during Task 16 from the "Benchmarks" table; every perf task must have a before/after measurement — an optimization without a measurement is not accepted)_

| Task | What was optimized | parse | unparse |
|------|--------------------|-------|---------|
| 7 | quick-xml 0.31 → 0.41 upgrade | — | — |
| 11 | escape via memchr3_iter, no unsafe | — | — |
| 12 | apply_postprocessor without allocation | — | — |
| 13 | lazy namespace stack + reuse path.pop() | — | — |
| 14 | PyString interning | — | — |
| **Total vs baseline** | | **—** | **—** |

## Deviations found during execution

_(empty — fill in during execution: new bugs from hypothesis, quick-xml 0.41 error-text changes, pyo3 0.29 incompatibilities, etc.)_

## Out of scope (from the plan, do not lose)

- Streaming `item_depth`/`item_callback` — a separate feature.
- Non-UTF-8 encodings (quick-xml `encoding` feature) — a separate feature.
- The reference's `expat=` kwarg — not accepted (TypeError), acceptable.
- Single-pass attributes in `start_element`, `SmallVec` for `text_stack` — if perf goals are missed by Tasks 12-14.
- `should_force_list`: swallowing an exception from `__contains__` — cosmetic.
