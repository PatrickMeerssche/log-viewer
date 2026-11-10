#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
from collections import defaultdict
from pathlib import Path

import matplotlib.pyplot as plt


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Plot current vs timestamp from an ArduPilot BAT CSV, grouped by Inst."
    )
    parser.add_argument(
        "-i",
        "--input",
        type=Path,
        default=None,
        help="Path to log_...__BAT.csv (default: first match in ./csv_out).",
    )
    return parser.parse_args()


def find_default_input() -> Path:
    matches = sorted(Path("csv_out").glob("*__BAT.csv"))
    if not matches:
        raise FileNotFoundError("No BAT CSV found in ./csv_out")
    return matches[0]


def parse_float(value: str) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def load_series(csv_path: Path) -> dict[int, list[tuple[float, float]]]:
    series: dict[int, list[tuple[float, float]]] = defaultdict(list)

    with csv_path.open("r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            try:
                inst = int(row.get("Inst", ""))
            except ValueError:
                continue

            timestamp = parse_float(row.get("_timestamp", ""))
            current = parse_float(row.get("Curr", ""))
            if timestamp is None or current is None:
                continue

            series[inst].append((timestamp, current))

    for inst in series:
        series[inst].sort(key=lambda item: item[0])

    return series


def plot_series(series: dict[int, list[tuple[float, float]]]) -> None:
    plt.figure(figsize=(12, 6))
    palette = plt.cm.tab10.colors

    for index, inst in enumerate(sorted(series)):
        points = series[inst]
        if not points:
            continue
        x = [point[0] for point in points]
        y = [point[1] for point in points]
        plt.plot(
            x,
            y,
            label=f"Battery Inst={inst}",
            color=palette[index % len(palette)],
            linewidth=1.5,
        )

    plt.title("ArduPilot BAT Current vs Timestamp")
    plt.xlabel("timestamp (_timestamp)")
    plt.ylabel("Curr")
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    plt.show()
    plt.close()


def main() -> int:
    args = parse_args()
    input_path = args.input if args.input else find_default_input()
    if not input_path.exists():
        print(f"Input file not found: {input_path}")
        return 1

    series = load_series(input_path)
    if not series:
        print("No valid BAT rows found to plot")
        return 2

    plot_series(series)
    counts = ", ".join(f"Inst {inst}: {len(points)}" for inst, points in sorted(series.items()))
    print("Displayed interactive plot window")
    print(f"Points: {counts}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())