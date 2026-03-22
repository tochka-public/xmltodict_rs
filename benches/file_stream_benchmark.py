from __future__ import annotations

import argparse
import gc
import os
import platform
import resource
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

import xmltodict

import xmltodict_rs


def format_bytes(num: float) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    value = float(num)
    for unit in units:
        if value < 1024 or unit == units[-1]:
            return f"{value:.2f} {unit}"
        value /= 1024
    return f"{value:.2f} TB"


def format_time(seconds: float) -> str:
    if seconds >= 1.0:
        return f"{seconds:.3f}s"
    if seconds >= 0.001:
        return f"{seconds * 1000:.3f}ms"
    return f"{seconds * 1_000_000:.3f}us"


def format_throughput(byte_count: int, seconds: float) -> str:
    if seconds <= 0:
        return "n/a"
    return f"{format_bytes(byte_count / seconds)}/s"


def current_max_rss_bytes() -> int:
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if platform.system() == "Darwin":
        return int(rss)
    return int(rss) * 1024

# This is a throughput-oriented stream benchmark, not a realism benchmark.
# The generated XML is intentionally comment-heavy and repetitive so we can
# isolate file I/O + decode/parse streaming cost without building a huge
# Python object tree that would dominate timing and memory.
def generate_xml_file(path: Path, target_bytes: int, encoding: str) -> int:
    header = f'<?xml version="1.0" encoding="{encoding}"?>\n<root>\n'.encode(encoding)
    # Keep payload as comments so parser does not build a huge result tree.
    line = "<!-- Привет payload line for streaming benchmark -->\n".encode(encoding)
    footer = "</root>\n".encode(encoding)

    min_size = len(header) + len(footer)
    if target_bytes < min_size:
        raise ValueError(f"target size is too small: {target_bytes} < minimum {min_size} bytes")

    body_budget = target_bytes - min_size
    line_count = body_budget // len(line)
    block_lines = 8192
    block = line * block_lines

    with path.open("wb") as f:
        f.write(header)
        full_blocks = line_count // block_lines
        rem_lines = line_count % block_lines

        for _ in range(full_blocks):
            f.write(block)
        for _ in range(rem_lines):
            f.write(line)

        f.write(footer)

    return path.stat().st_size


def benchmark_cases_fresh_file(
    target_bytes: int,
    encoding: str,
    cases: list[tuple[str, Callable[..., Any], dict[str, Any]]],
    repeats: int,
) -> None:
    bytes_per_file = 0
    for name, parser, parse_kwargs in cases:
        times: list[float] = []
        for _ in range(repeats):
            fd, temp_path = tempfile.mkstemp(prefix="xmltodict_rs_bench_", suffix=".xml")
            os.close(fd)
            file_path = Path(temp_path)

            bytes_per_file = generate_xml_file(file_path, target_bytes, encoding)
            gc.collect()
            start = time.perf_counter()
            with file_path.open("rb") as f:
                parser(f, **parse_kwargs)
            elapsed = time.perf_counter() - start
            times.append(elapsed)
            file_path.unlink(missing_ok=True)

        avg = sum(times) / len(times)
        print(
            f"{name:<28} avg={format_time(avg):>10}  min={format_time(min(times)):>10}  "
            f"max={format_time(max(times)):>10}  throughput={format_throughput(bytes_per_file, avg):>16}"
        )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Benchmark large file parsing for stream decoding paths."
    )
    parser.add_argument("--size-gb", type=float, default=1.0)
    parser.add_argument("--encoding", default="windows-1251")
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--compare-xmltodict", action="store_true")
    args = parser.parse_args()

    if args.repeats < 1:
        raise ValueError("--repeats must be >= 1")

    target_bytes = int(args.size_gb * (1024**3))
    if target_bytes <= 0:
        raise ValueError("--size-gb must be > 0")

    print("Large-file benchmark")
    print("  Mode: fresh-file per measured run")
    print(f"  Target size: {format_bytes(target_bytes)}")
    print(f"  Declared encoding: {args.encoding}")
    print(f"  Repeats: {args.repeats}")
    print(f"  Process max RSS (pre-run): {format_bytes(current_max_rss_bytes())}")
    print()

    cases: list[tuple[str, Callable[..., Any], dict[str, Any]]] = [
        ("xmltodict_rs (auto detect)", xmltodict_rs.parse, {}),
        ("xmltodict_rs (explicit enc)", xmltodict_rs.parse, {"encoding": args.encoding}),
    ]

    if args.compare_xmltodict:
        cases.extend(
            [
                ("xmltodict (auto detect)", xmltodict.parse, {}),
                ("xmltodict (explicit enc)", xmltodict.parse, {"encoding": args.encoding}),
            ]
        )

    benchmark_cases_fresh_file(
        target_bytes=target_bytes,
        encoding=args.encoding,
        cases=cases,
        repeats=args.repeats,
    )

    print()
    print(f"  Process max RSS (post-run): {format_bytes(current_max_rss_bytes())}")


if __name__ == "__main__":
    main()
