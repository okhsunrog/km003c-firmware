"""Compare a custom CDC capture against the preserved stock line samples.

Reports observed differences, not absolute calibration error: reads are
sequential and the reference is stock firmware, not an external voltmeter.
"""

import argparse
import csv
import json
from pathlib import Path
from statistics import mean, pstdev


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--input", choices=[1, 2], type=int, default=1)
    parser.add_argument("--stock", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    stock_path = args.stock or (
        Path(__file__).resolve().parents[1]
        / "hardware/line-adc"
        / f"stock-{'original' if args.input == 1 else 'flipped'}.csv"
    )
    stock = list(csv.DictReader(stock_path.open()))
    custom = [
        record
        for line in args.capture.read_text().splitlines()
        if (record := json.loads(line))["type"] == "lines"
        and record["cc_input"] == args.input
        and record[f"cc{args.input}_uv"] > 1_000_000
    ]
    # Exclude insertion/PD negotiation immediately after changing inputs. With
    # several visits to an input, apply the bound separately to each visit.
    stable = []
    since = 0
    previous = None
    for record in custom:
        if previous is None or record["ms"] - previous > 1000:
            since = record["ms"]
        previous = record["ms"]
        if record["ms"] - since >= 2000:
            stable.append(record)
    if not stock or not stable:
        parser.error(
            "Need stock samples and at least two seconds of stable custom data"
        )
    result = {
        "stock": str(stock_path),
        "capture": str(args.capture),
        "input": args.input,
        "stock_samples": len(stock),
        "custom_samples": len(stable),
        "units": "mV",
        "fields": {},
    }
    for name in ("cc1", "cc2", "dp", "dm", "vdd"):
        before = [float(row[f"{name}_mv"]) for row in stock]
        after = [row[f"{name}_uv"] / 1000 for row in stable]
        result["fields"][name] = {
            "stock_mean": mean(before),
            "custom_mean": mean(after),
            "delta": mean(after) - mean(before),
            "custom_min": min(after),
            "custom_max": max(after),
            "custom_stddev": pstdev(after),
        }
    text = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()
