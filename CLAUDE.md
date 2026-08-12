# xmltodict_rs — context for Claude Code

A living cheat sheet for this project: keep it up to date (see "Self-maintenance
protocol"). This is a **map**, not documentation — do not duplicate details,
reference them instead:

- API, versioning, development commands — `README.md`;
- active work plans and their ledgers — `docs/plans/`;
- the truth about behavior — the code itself and the `xmltodict` reference
  (the map can go stale, see "Working with the code").

## Language policy

All documentation, code comments, commit messages, and this file — in English.

## What this project is

A drop-in replacement for the Python `xmltodict` library, written in Rust
(PyO3 + quick-xml). One native module `xmltodict_rs.xmltodict_rs`, two
functions: `parse` (XML → dict) and `unparse` (dict → XML). Versioning:
major.minor match upstream `xmltodict` (0.13.x ↔ 0.13.x) — behavior must
match the same-numbered reference version. Public OSS
(github.com/tochka-public).

## Core principle: differential equivalence

- The behavioral reference is `xmltodict==0.13.0` (dev dependency). Every
  behavioral change is validated by comparison against the reference (the
  `compare_parsers` pattern in `tests/`: identical result or identical
  exception **type**).
- Parse errors are `xml.parsers.expat.ExpatError`, created dynamically in
  `src/error.rs::expat_error`. Error texts do not match the reference —
  tests compare the type, not the text.
- Deliberate deviations from the reference are recorded in README (known
  limitations) and in "Invariants" below.

## Architecture map

- `src/lib.rs` — pyfunctions `parse`/`unparse`; input dispatch: str (UTF-8
  bytes, zero-copy) → bytes → file-like (`PyFileLikeRead`) → generator
  (`PyGeneratorRead`) → fallback `extract::<&[u8]>`; quick-xml event loop →
  `XmlParser`; `Event::GeneralRef` (entity/char refs, quick-xml 0.41+) resolved
  via `resolve_general_ref`; final stack-balance validation.
- `src/parser.rs` — `XmlParser`: four synchronized stacks — `stack` (PyDict),
  `path` (element names), `text_stack`, `namespace_stack`; `push_data`
  (repeated key → list; `force_list`; `postprocessor`); `build_name`
  (namespace mapping via `namespaces`).
- `src/unparser.rs` — `XmlWriter`: recursive `write_element`/
  `write_dict_element`; value-type branch order: None → str → dict →
  iterable → bool → `str()` fallback; `preprocessor`.
- `src/escape.rs` — `escape_xml` (text: `&`, `<`, `>`) and `escape_xml_attr`
  (attributes: + `"`); text and attributes are escaped differently.
- `src/error.rs` — `WrappedPyErr`: PyErr ↔ `io::Error` tunnel (a Python
  exception raised in `read()`/generator passes through the quick-xml Reader
  and is recovered in `map_quick_xml_error`); `expat_error`.
- `src/config.rs` — `ParseConfig`/`UnparseConfig`, string newtypes.
- `src/reader/` — `Read` adapters for Python objects; `pending.rs` — buffer
  for the chunk tail that did not fit into `out`.
- `python/xmltodict_rs/` — `__init__.py` (re-export) and `__init__.pyi`
  (type stubs).
- `tests/*.py` — differential tests; `benches/accurate_benchmark.py` —
  benchmark against the reference.

## Invariants and gotchas

- **`parse`/`unparse` signatures live in three places**: `#[pyo3(signature)]`
  in `lib.rs`, stubs in `python/xmltodict_rs/__init__.pyi`, README "API
  Reference". Change all three in sync.
- **quick-xml Reader config**: `expand_empty_elements = true` — the
  `Event::Empty` branch never fires because of that, but its code must stay
  correct. `trim_text` is intentionally left at its default (off): quick-xml
  0.41 reports `&entity;`/`&#NN;` as standalone `Event::GeneralRef` events, so
  trimming per-event would eat whitespace adjacent to every entity.
  `strip_whitespace` is instead applied once in `XmlParser::end_element`, on
  the fully joined text of an element.
- **The four `XmlParser` stacks move strictly in sync** in
  `start_element`/`end_element`; desync → "unclosed element(s)" at the end of
  `parse_xml_with_reader`.
- **`gil_used = false`**: the module is declared safe for free-threaded
  CPython (3.13t/3.14t) — no global mutable state without synchronization;
  parser state is per-call only.
- **mimalloc** — global allocator only on linux-x86_64 / windows-x86_64 /
  macos (feature `mimalloc`, default on).
- **`panic = "abort"` in the release profile** — a panic in any dependency
  kills the whole Python process; the review plan schedules a switch to
  unwind (Task 8 of the 2026-08-12 plan).
- **Lints are a hard gate**: `warnings = deny`, clippy `all`+`pedantic` =
  deny, bans on `unwrap`/`expect`/`panic`/indexing/casts — see `[lints]` in
  `Cargo.toml`. Do not add `#[allow]` in production code — fix the cause.
- **Known limitations** (review 2026-08-12): streaming
  (`item_depth`/`item_callback`) is not implemented; non-UTF-8 encodings are
  not supported; `disable_entities=False` does not expand DTD entities.
  Confirmed bugs and the fix plan — `docs/plans/2026-08-12-review-fixes.md`
  + the ledger next to it; update this item once the plan is done.
- **Performance changes are not accepted without a benchmark**: measure
  `just bench` before and after, record the numbers in the active plan's
  ledger; no win — revert.

## Build, lints, tests

- `just dev` — `maturin develop --release`. **After any Rust code change,
  rebuild before running pytest** — otherwise you are testing the stale
  `.so`.
- `just test` — pytest + `cargo test`; `just check` — fmt + clippy (gate).
- `just bench` — rebuild + `benches/accurate_benchmark.py`.
- CI: `.github/workflows/CI.yml` — matrix of platforms and CPython versions,
  including free-threaded 3.13t/3.14t; PyPy is not supported.

## Commits

- Conventional commits in English (`fix:`/`feat:`/`perf:`/`ci:`/`docs:`/
  `chore:`/`test:`/`refactor:`), straight quotes, no `Co-Authored-By`
  trailer.
- Version bump is a separate commit "bump version to X.Y.Z" (`version` in
  `Cargo.toml`).

## Working with the code (evidence)

Do not change behavior from memory. The map above can lag behind the code —
before relying on a file/function/invariant mentioned here, verify it still
exists. A claim "behaves like xmltodict" without a differential run is an
opinion, not a fact.

## Self-maintenance protocol for this file

Update `CLAUDE.md` **in the same change** whenever any of these change:

- the set/names of `src/` modules, the parse/unparse data flow;
- `parse`/`unparse` signatures (together with `.pyi` and README);
- any invariant or limitation listed above (including the status of known
  bugs once the plan is executed);
- lint/build-profile policy or the set of `just` commands.

Rules: terse (one line — one fact); this is a map, not a docstring — leave
details to the code, README, and `docs/plans/`. Do not record transient task
state here.

## Before finishing a change

- `just check` is green; `just dev` + pytest pass; `cargo test` passes.
- Signatures changed → `.pyi` and README "API Reference" updated.
- Behavioral change → a differential test against `xmltodict` exists.
- Performance change → benchmark before/after, numbers in the ledger.
- `CLAUDE.md` updated per the self-maintenance protocol.
