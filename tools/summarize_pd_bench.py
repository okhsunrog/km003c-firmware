"""Validate and aggregate raw DWT samples from one or more `pd bench` captures.

Run with uv run tools/summarize_pd_bench.py CAPTURE... [--json FILE].
Only benchmark records are used; analog/USB telemetry is left untouched.
"""

import argparse
import json
from collections import defaultdict
from pathlib import Path


def quantiles(values):
    ordered = sorted(values)
    return {
        "min": ordered[0],
        "p50": ordered[len(ordered) // 2],
        "p95": ordered[len(ordered) * 95 // 100],
        "max": ordered[-1],
    }


def read_capture(path):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    assert sum(r["type"] == "pd_bench_done" for r in rows) == 1, (
        f"{path}: incomplete run"
    )
    gates = [r for r in rows if r["type"] == "pd_bench_gate"]
    assert len(gates) == 1 and gates[0]["cases"] == 322, f"{path}: gate missing"
    chunks = defaultdict(dict)
    for row in rows:
        if row["type"] == "pd_bench_samples":
            key = row["fixture"], row["candidate"]
            offset = row["offset"]
            assert offset not in chunks[key], f"{path}: duplicate sample chunk"
            chunks[key][offset] = row["cycles"]
    for row in rows:
        if row["type"] != "pd_bench":
            continue
        key = row["fixture"], row["candidate"]
        parts = chunks.pop(key)
        assert sorted(parts) == [0, 32, 64, 96], f"{path}: incomplete samples {key}"
        samples = [v for offset in sorted(parts) for v in parts[offset]]
        assert len(samples) == row["samples"] == 128
        assert quantiles(samples) == {k: row[k] for k in ("min", "p50", "p95", "max")}
        assert row["hz"] == 192_000_000
        yield key, samples
    assert not chunks, f"{path}: samples without summary"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", type=Path, nargs="+")
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    groups = defaultdict(list)
    for path in args.captures:
        for key, samples in read_capture(path):
            groups[key].append(samples)
    result = []
    print("fixture candidate runs samples min_us p50_us p95_us max_us")
    for (fixture, candidate), runs in groups.items():
        samples = [v for run in runs for v in run]
        q = quantiles(samples)
        row = {
            "fixture": fixture,
            "candidate": candidate,
            "runs": len(runs),
            "samples": len(samples),
            "cycles": q,
            "us": {k: round(v / 192, 3) for k, v in q.items()},
            "run_p50_cycles": [quantiles(run)["p50"] for run in runs],
        }
        result.append(row)
        print(
            f"{fixture} {candidate} {len(runs)} {len(samples)} "
            + " ".join(f"{v / 192:.2f}" for v in q.values())
        )
    if args.json:
        args.json.write_text(
            json.dumps(
                {
                    "clock_hz": 192_000_000,
                    "captures": [str(p) for p in args.captures],
                    "results": result,
                },
                indent=2,
            )
            + "\n"
        )


if __name__ == "__main__":
    main()
