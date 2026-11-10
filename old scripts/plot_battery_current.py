#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
from pathlib import Path

import matplotlib.pyplot as plt


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Generate a current-vs-timestamp graph from a BATTERY_STATUS CSV "
            "for battery id=0 and id=1."
        )
    )
    parser.add_argument(
        "-i",
        "--input",
        type=Path,
        default=None,
        help="Path to a BATTERY_STATUS CSV file (default: first match in ./csv_out).",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("csv_out/battery_current_vs_timestamp.png"),
        help="Output image path (default: csv_out/battery_current_vs_timestamp.png).",
    )
    return parser.parse_args()


def find_default_input() -> Path:
    matches = sorted(Path("csv_out").glob("*__BATTERY_STATUS.csv"))
    if not matches:
        raise FileNotFoundError("No BATTERY_STATUS CSV found in ./csv_out")
    return matches[0]


def parse_float(value: str) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def load_series(csv_path: Path) -> dict[int, list[tuple[float, float]]]:
    series: dict[int, list[tuple[float, float]]] = {0: [], 1: []}

    with csv_path.open("r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            try:
                battery_id = int(row.get("id", ""))
            except ValueError:
                continue

            if battery_id not in (0, 1):
                continue

            timestamp = parse_float(row.get("_timestamp", ""))
            current = parse_float(row.get("current_battery", ""))
            if timestamp is None or current is None:
                continue

            series[battery_id].append((timestamp, current))

    for battery_id in series:
        series[battery_id].sort(key=lambda item: item[0])

    return series


def plot_series(series: dict[int, list[tuple[float, float]]], output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)

    plt.figure(figsize=(11, 6))
    colors = {0: "tab:blue", 1: "tab:orange"}

    for battery_id in (0, 1):
        points = series[battery_id]
        if not points:
            continue
        x = [p[0] for p in points]
        y = [p[1] for p in points]
        plt.plot(x, y, label=f"Battery id={battery_id}", color=colors[battery_id], linewidth=1.5)

    plt.title("BATTERY_STATUS Current vs Timestamp")
    plt.xlabel("timestamp (_timestamp)")
    plt.ylabel("current_battery")
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()


def main() -> int:
    args = parse_args()
    input_path = args.input if args.input else find_default_input()
    if not input_path.exists():
        print(f"Input file not found: {input_path}")
        return 1

    series = load_series(input_path)
    if not series[0] and not series[1]:
        print("No valid rows found for battery id=0 or id=1")
        return 2

    plot_series(series, args.output)
    print(f"Graph saved to: {args.output}")
    print(f"Points: id=0 -> {len(series[0])}, id=1 -> {len(series[1])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())