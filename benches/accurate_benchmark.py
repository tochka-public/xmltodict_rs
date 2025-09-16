import gc
import statistics
import time
from typing import Any, Callable

import xmltodict
import xmltodict_rs


def format_time(seconds: float) -> str:
    """Formats time in readable units"""
    if seconds >= 1.0:
        return f"{seconds:.3f}s"
    elif seconds >= 0.001:
        return f"{seconds * 1000:.3f}ms"
    elif seconds >= 0.000001:
        return f"{seconds * 1000000:.3f}us"
    else:
        return f"{seconds * 1000000000:.3f}ns"


def format_throughput(bytes_per_second: float) -> str:
    """Formats throughput in readable units (power of 2)"""
    if bytes_per_second >= 1024**3:
        return f"{bytes_per_second / (1024**3):.2f} GB/s"
    elif bytes_per_second >= 1024**2:
        return f"{bytes_per_second / (1024**2):.2f} MB/s"
    elif bytes_per_second >= 1024:
        return f"{bytes_per_second / 1024:.2f} KB/s"
    else:
        return f"{bytes_per_second:.2f} B/s"


class AccurateBenchmark:
    def __init__(self):
        self.warmup_duration = 1.0  # seconds
        self.target_duration = 2.0  # seconds per benchmark
        self.min_iterations = 3  # minimum iterations
        self.max_iterations = 2_000_000  # maximum iterations

    def warmup_function(self, func: Callable, *args, **kwargs) -> float:
        """Warms up function for specified duration"""
        print(f"    🔥 Warmup ({self.warmup_duration}s)...", end=" ", flush=True)

        start_time = time.perf_counter()
        iterations = 0

        while (time.perf_counter() - start_time) < self.warmup_duration:
            func(*args, **kwargs)
            iterations += 1

        avg = (time.perf_counter() - start_time) / iterations
        print(f"{iterations} iterations")
        return avg

    def measure_function(
        self, func: Callable, avg: float, *args, **kwargs
    ) -> tuple[dict[float], int]:
        """Measures function execution time in tight loop"""
        iterations = int(self.target_duration / avg)
        iterations = max(self.min_iterations, iterations)
        iterations = min(self.max_iterations, iterations)
        print(f"    📊 Measuring {iterations} iterations...", end=" ", flush=True)
        times = []
        gc.disable()
        try:
            for _ in range(iterations):
                start_time = time.perf_counter()
                func(*args, **kwargs)
                end_time = time.perf_counter()
                times.append(end_time - start_time)
        finally:
            gc.enable()

        print("completed")
        return times, iterations

    def calculate_statistics(self, times: dict[float]) -> dict[str, float]:
        """Calculates statistical metrics"""
        if not times:
            return {}

        sorted_times = sorted(times)
        n = len(sorted_times)

        def percentile(p: float) -> float:
            k = (n - 1) * p / 100
            f = int(k)
            c = k - f
            if f == n - 1:
                return sorted_times[f]
            return sorted_times[f] * (1 - c) + sorted_times[f + 1] * c

        return {
            "count": n,
            "mean": statistics.mean(times),
            "median": statistics.median(times),
            "stddev": statistics.stdev(times) if n > 1 else 0,
            "min": min(times),
            "max": max(times),
            "p50": percentile(50),
            "p75": percentile(75),
            "p95": percentile(95),
            "p99": percentile(99),
        }

    def benchmark_function(self, name: str, func: Callable, *args, **kwargs) -> dict[str, Any]:
        """Complete benchmark cycle for one function"""
        print(f"  🎯 {name}")
        avg = self.warmup_function(func, *args, **kwargs)
        times, iterations = self.measure_function(func, avg, *args, **kwargs)
        stats = self.calculate_statistics(times)

        return {"name": name, "iterations": iterations, "times": times, "stats": stats}

    def compare_functions(
        self,
        test_name: str,
        func1: Callable,
        func2: Callable,
        name1: str,
        name2: str,
        data_size_bytes: int,
        *args,
        **kwargs,
    ) -> dict[str, Any]:
        """Compares two functions"""
        print(f"\n📋 Benchmark: {test_name}")
        print("-" * 60)

        # Benchmark first function
        result1 = self.benchmark_function(name1, func1, *args, **kwargs)

        # Benchmark second function
        result2 = self.benchmark_function(name2, func2, *args, **kwargs)

        # Comparison
        stats1 = result1["stats"]
        stats2 = result2["stats"]

        speedup = stats1["mean"] / stats2["mean"]

        # Calculate throughput
        throughput1 = data_size_bytes / stats1["mean"]
        throughput2 = data_size_bytes / stats2["mean"]

        print("\n  📈 Results:")
        print(
            f"    {name1:>15}: {format_time(stats1['mean']):>10} ± {format_time(stats1['stddev']):>8}"
        )
        print(
            f"    {name2:>15}: {format_time(stats2['mean']):>10} ± {format_time(stats2['stddev']):>8}"
        )
        print(f"    {'Speedup':>15}: {speedup:>8.2f}x")
        print("  📊 Throughput:")
        print(f"    {name1:>15}: {format_throughput(throughput1):>12}")
        print(f"    {name2:>15}: {format_throughput(throughput2):>12}")

        return {
            "test_name": test_name,
            "func1": result1,
            "func2": result2,
            "speedup": speedup,
            "throughput1": throughput1,
            "throughput2": throughput2,
            "faster": name2 if speedup > 1.0 else name1,
        }

    def print_detailed_stats(
        self, name: str, stats: dict[str, float], data_size_bytes: int = 0
    ) -> None:
        """Prints detailed statistics"""
        print(f"\n  📊 Detailed Statistics - {name}")
        print(f"    Iterations: {stats['count']:,}")
        print(f"    Mean:      {format_time(stats['mean']):>10}")
        print(f"    Median:    {format_time(stats['median']):>10}")
        print(f"    StdDev:    {format_time(stats['stddev']):>9}")
        print(f"    Min:       {format_time(stats['min']):>10}")
        print(f"    Max:       {format_time(stats['max']):>10}")
        if data_size_bytes > 0:
            throughput = data_size_bytes / stats["mean"]
            print(f"    Throughput: {format_throughput(throughput):>10}")
        print("    Percentiles:")
        print(f"      P50:     {format_time(stats['p50']):>10}")
        print(f"      P75:    {format_time(stats['p75']):>10}")
        print(f"      P95:    {format_time(stats['p95']):>10}")
        print(f"      P99:    {format_time(stats['p99']):>10}")


def generate_test_data() -> dict[str, str]:
    """Generates test XML data of different sizes"""
    # Small XML
    small_xml = """<?xml version="1.0" encoding="utf-8"?>
<root>
    <item id="1">Small test</item>
    <config>
        <debug>true</debug>
        <timeout>30</timeout>
    </config>
</root>"""

    # Medium XML
    medium_items = []
    for i in range(50):
        medium_items.append(f'''
    <product id="{i}" category="test">
        <name>Product {i}</name>
        <price>{10.0 + i}</price>
        <description>Description {i}</description>
        <available>{"true" if i % 2 == 0 else "false"}</available>
    </product>''')

    medium_xml = f"""<?xml version="1.0" encoding="utf-8"?>
<catalog>
    <metadata>
        <created>2024-01-01</created>
        <version>1.0</version>
    </metadata>
    <products>{"".join(medium_items)}
    </products>
</catalog>"""

    # Large XML with attributes
    large_items = []
    for i in range(200):
        large_items.append(f'''
    <record id="{i}" type="data" priority="{i % 5}"
            category="cat{i % 10}" status="active"
            created="2024-01-{(i % 30) + 1:02d}"
            weight="{0.5 + i * 0.1}" size="{100 + i * 5}">
        <title>Record Title {i}</title>
        <content>{"Long content text " * 5} for record {i}</content>
        <tags>
            <tag>tag{i % 7}</tag>
            <tag>category{i % 5}</tag>
        </tags>
        <metadata>
            <author>Author{i % 20}</author>
            <revision>{i % 3 + 1}</revision>
        </metadata>
    </record>''')

    large_xml = f"""<?xml version="1.0" encoding="utf-8"?>
<database xmlns="http://example.com" version="2.0">
    <header>
        <generated>{time.time()}</generated>
        <total>{len(large_items)}</total>
    </header>
    <data>{"".join(large_items)}
    </data>
</database>"""

    return {
        "Small XML (0.3KB)": small_xml,
        "Medium XML (15KB)": medium_xml,
        "Large XML (150KB)": large_xml,
    }


def run_accurate_benchmarks():
    """Runs accurate benchmarks"""
    print("🎯 ACCURATE PERFORMANCE BENCHMARKS")
    print("=" * 70)
    print("Methodology: warmup 1s → calibration → measurement 5s each")
    print()

    benchmark = AccurateBenchmark()
    test_data = generate_test_data()

    all_results = []

    for test_name, xml_data in test_data.items():
        size_bytes = len(xml_data.encode("utf-8"))
        size_kb = size_bytes / 1024
        print(f"\n{'=' * 70}")
        print(f"🧪 TEST: {test_name} ({size_kb:.1f} KB)")
        print(f"{'=' * 70}")

        # Parse test
        parse_result = benchmark.compare_functions(
            f"Parse - {test_name}",
            xmltodict.parse,
            xmltodict_rs.parse,
            "xmltodict",
            "xmltodict_rs",
            size_bytes,
            xml_data,
        )

        # Detailed parse statistics
        benchmark.print_detailed_stats(
            "xmltodict (parse)", parse_result["func1"]["stats"], size_bytes
        )
        benchmark.print_detailed_stats(
            "xmltodict_rs (parse)", parse_result["func2"]["stats"], size_bytes
        )

        # Prepare data for unparse
        try:
            parsed_data = xmltodict_rs.parse(xml_data)
        except Exception as e:
            print(f"❌ Parse error for unparse test: {e}")
            continue

        # Unparse test (data size for unparse is the generated XML size)
        unparse_result = benchmark.compare_functions(
            f"Unparse - {test_name}",
            xmltodict.unparse,
            xmltodict_rs.unparse,
            "xmltodict",
            "xmltodict_rs",
            size_bytes,
            parsed_data,
        )

        # Detailed unparse statistics
        benchmark.print_detailed_stats(
            "xmltodict (unparse)", unparse_result["func1"]["stats"], size_bytes
        )
        benchmark.print_detailed_stats(
            "xmltodict_rs (unparse)", unparse_result["func2"]["stats"], size_bytes
        )

        all_results.append(
            {
                "test_name": test_name,
                "size_kb": size_kb,
                "parse": parse_result,
                "unparse": unparse_result,
            }
        )

    # Final summary
    print(f"\n{'=' * 70}")
    print("📊 FINAL SUMMARY")
    print(f"{'=' * 70}")

    parse_speedups = []
    unparse_speedups = []

    print(
        f"\n{'Test':<20} {'Size':<10} {'Parse':<12} {'Unparse':<12} {'Parse Throughput':<15} {'Unparse Throughput':<17}"
    )
    print("-" * 95)

    for result in all_results:
        test_name = result["test_name"].replace(" XML", "").replace(" (", "\n(")
        size = f"{result['size_kb']:.1f}KB"
        parse_speedup = result["parse"]["speedup"]
        unparse_speedup = result["unparse"]["speedup"]

        # Throughput for xmltodict_rs (second result)
        parse_throughput = format_throughput(result["parse"]["throughput2"])
        unparse_throughput = format_throughput(result["unparse"]["throughput2"])

        parse_speedups.append(parse_speedup)
        unparse_speedups.append(unparse_speedup)

        print(
            f"{test_name:<20} {size:<10} {parse_speedup:>8.2f}x {unparse_speedup:>8.2f}x {parse_throughput:>13} {unparse_throughput:>15}"
        )

    avg_parse = statistics.mean(parse_speedups)
    avg_unparse = statistics.mean(unparse_speedups)
    overall_avg = (avg_parse + avg_unparse) / 2

    print("-" * 95)
    print(f"{'AVERAGE':<20} {'':<10} {avg_parse:>8.2f}x {avg_unparse:>8.2f}x {'':<13} {'':<15}")
    print(f"\n🚀 Overall speedup: {overall_avg:.2f}x")
    print(f"📊 Parse on average: {avg_parse:.2f}x faster")
    print(f"📊 Unparse on average: {avg_unparse:.2f}x faster")

    return all_results


if __name__ == "__main__":
    results = run_accurate_benchmarks()
    print("\n✅ Completed!")
