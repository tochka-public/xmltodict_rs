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

REPEATS = 10
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
        f"<available>{'true' if i % 2 == 0 else 'false'}</available></product>"
        for i in range(50)
    )
    medium = f'<?xml version="1.0" encoding="utf-8"?><catalog>{medium_items}</catalog>'
    large_items = "".join(
        f'<record id="{i}" type="data" priority="{i % 5}" category="cat{i % 10}" '
        f'status="active" created="2024-01-{(i % 30) + 1:02d}">'
        f"<title>Record Title {i}</title>"
        f"<content>{'Long content text ' * 5} for record {i}</content>"
        f"<tags><tag>tag{i % 7}</tag><tag>category{i % 5}</tag></tags>"
        f"</record>"
        for i in range(200)
    )
    large = (
        f'<?xml version="1.0" encoding="utf-8"?><database version="2.0">{large_items}</database>'
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
    """CLI entry point: measure or compare benchmark runs."""
    if len(sys.argv) == 3 and sys.argv[1] == "measure":
        measure(sys.argv[2])
    elif len(sys.argv) == 4 and sys.argv[1] == "compare":
        compare(sys.argv[2], sys.argv[3])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
