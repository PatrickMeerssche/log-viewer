#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

try:
	from pymavlink import mavutil
except Exception as exc:  # pragma: no cover
	print("Missing dependency: pymavlink", file=sys.stderr)
	print("Install it with: pip install pymavlink", file=sys.stderr)
	raise SystemExit(1) from exc


def _sanitize_value(value: Any) -> Any:
	if isinstance(value, (dict, list, tuple)):
		return json.dumps(value, ensure_ascii=True)
	return value


def _safe_stem(path: Path) -> str:
	return "".join(ch if ch.isalnum() or ch in "-_" else "_" for ch in path.stem)


def _iter_log_messages(log_path: Path):
	conn = mavutil.mavlink_connection(str(log_path), notimestamps=False)
	while True:
		msg = conn.recv_match(blocking=False)
		if msg is None:
			break
		yield msg


def convert_log(log_path: Path, output_dir: Path) -> tuple[int, int]:
	rows_by_type: dict[str, list[dict[str, Any]]] = defaultdict(list)
	keys_by_type: dict[str, list[str]] = defaultdict(list)
	total_messages = 0

	for msg in _iter_log_messages(log_path):
		total_messages += 1
		msg_type = msg.get_type()
		row = msg.to_dict() if hasattr(msg, "to_dict") else {}
		row.pop("mavpackettype", None)

		msg_ts = getattr(msg, "_timestamp", None)
		if msg_ts is not None and "_timestamp" not in row:
			row["_timestamp"] = msg_ts

		clean_row = {k: _sanitize_value(v) for k, v in row.items()}

		known_keys = keys_by_type[msg_type]
		for key in clean_row.keys():
			if key not in known_keys:
				known_keys.append(key)

		rows_by_type[msg_type].append(clean_row)

	if total_messages == 0:
		return 0, 0

	output_dir.mkdir(parents=True, exist_ok=True)
	base = _safe_stem(log_path)
	files_written = 0

	for msg_type, rows in rows_by_type.items():
		keys = keys_by_type[msg_type]
		out_file = output_dir / f"{base}__{msg_type}.csv"
		with out_file.open("w", newline="", encoding="utf-8") as f:
			writer = csv.DictWriter(f, fieldnames=keys, extrasaction="ignore")
			writer.writeheader()
			for row in rows:
				writer.writerow(row)
		files_written += 1

	return total_messages, files_written


def discover_logs(paths: list[Path]) -> list[Path]:
	if paths:
		return [p for p in paths if p.suffix.lower() in {".bin", ".tlog"} and p.exists()]

	cwd = Path.cwd()
	return sorted([*cwd.glob("*.bin"), *cwd.glob("*.tlog")])


def parse_args() -> argparse.Namespace:
	parser = argparse.ArgumentParser(
		description="Convert ArduPilot/QGroundControl .bin and .tlog logs into CSV files."
	)
	parser.add_argument(
		"inputs",
		nargs="*",
		type=Path,
		help="Input .bin/.tlog files. If omitted, convert all .bin/.tlog in current directory.",
	)
	parser.add_argument(
		"-o",
		"--output-dir",
		type=Path,
		default=Path("csv_out"),
		help="Directory where CSV files will be created (default: ./csv_out).",
	)
	return parser.parse_args()


def main() -> int:
	args = parse_args()
	logs = discover_logs(args.inputs)
	if not logs:
		print("No .bin or .tlog files found.", file=sys.stderr)
		return 1

	total_logs = 0
	total_msgs = 0
	total_csv = 0

	for log_path in logs:
		try:
			msgs, csv_files = convert_log(log_path, args.output_dir)
		except Exception as exc:
			print(f"Failed to convert {log_path.name}: {exc}", file=sys.stderr)
			continue

		total_logs += 1
		total_msgs += msgs
		total_csv += csv_files
		print(f"Converted {log_path.name}: {msgs} messages -> {csv_files} CSV files")

	if total_logs == 0:
		print("No logs were converted.", file=sys.stderr)
		return 2

	print(
		f"Done. Converted {total_logs} logs, {total_msgs} messages total, {total_csv} CSV files in {args.output_dir}"
	)
	return 0


if __name__ == "__main__":
	raise SystemExit(main())
