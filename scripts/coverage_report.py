"""Render `cargo llvm-cov`'s JSON summary as a per-file table, worst first.

Called by the justfile's `coverage` recipe. It exists because the shape of the
question is "where is the suite thin", and the shape of `cargo llvm-cov`'s own
text report is a wall of rows sorted by path — which answers "what is the
coverage of file X" and answers the other question only if you read all of it.

Reading order is deliberate and it is not the obvious one. The worst files are
printed **first** and the TOTAL **last**, because a terminal scrolls: whatever
prints last is what is on screen when the command returns. Printing every file
worst-first would put the worst rows off the top of the scrollback, which is why
the default truncates to the worst `--top N` and says how many rows it withheld.

THIS IS NOT A GATE. It exits 0 whatever the numbers are, and takes no threshold
flag, on purpose — see the justfile's `coverage` recipe for the argument.

Usage:
    python3 scripts/coverage_report.py <export.json> [SUBSTRING ...] [--all] [--top N]

`SUBSTRING` filters the table by path (`just coverage journal undo`). It never
changes the TOTAL line, which is always the whole-workspace figure, so the
headline number cannot be moved by how the question was asked.
"""

import json
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

DEFAULT_TOP = 20


def relative(filename: str) -> str:
    try:
        return str(pathlib.Path(filename).resolve().relative_to(REPO))
    except ValueError:
        return filename


def pct(summary: dict, key: str) -> float:
    part = summary.get(key) or {}
    # llvm-cov reports percent 0 for a file with nothing of that kind to
    # measure. Treat "no denominator" as 100 so a file of pure declarations does
    # not sit at the top of a worst-first list claiming to be uncovered.
    if not part.get("count"):
        return 100.0
    return float(part.get("percent", 0.0))


def main(argv: list[str]) -> int:
    args = list(argv)
    show_all = False
    top = DEFAULT_TOP
    substrings: list[str] = []
    path: str | None = None

    while args:
        arg = args.pop(0)
        if arg == "--all":
            show_all = True
        elif arg == "--top":
            if not args:
                print("coverage_report: --top needs a number", file=sys.stderr)
                return 2
            top = int(args.pop(0))
        elif arg.startswith("--"):
            print(f"coverage_report: unknown flag {arg}", file=sys.stderr)
            return 2
        elif path is None:
            path = arg
        else:
            substrings.append(arg)

    if path is None:
        print("coverage_report: needs the path to a cargo-llvm-cov JSON export", file=sys.stderr)
        return 2

    try:
        export = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"coverage_report: cannot read {path}: {exc}", file=sys.stderr)
        return 2

    data = (export.get("data") or [{}])[0]
    files = data.get("files") or []
    totals = data.get("totals") or {}

    if not files:
        print("coverage_report: the export names no files — did the run produce any coverage?")
        return 0

    rows = []
    for entry in files:
        summary = entry.get("summary") or {}
        lines = summary.get("lines") or {}
        count = int(lines.get("count") or 0)
        covered = int(lines.get("covered") or 0)
        rows.append(
            {
                "file": relative(entry.get("filename", "?")),
                "lines": pct(summary, "lines"),
                "uncovered": count - covered,
                "functions": pct(summary, "functions"),
                "regions": pct(summary, "regions"),
            }
        )

    shown = rows
    if substrings:
        shown = [r for r in rows if any(s in r["file"] for s in substrings)]
        if not shown:
            print(f"coverage_report: no file matches {' '.join(substrings)}")
            return 0

    # Worst first: lowest line coverage, and where two files tie, the one with
    # more uncovered lines — a 60%-covered 500-line file is a bigger hole than a
    # 60%-covered 10-line one, and the tie is common at round percentages.
    shown.sort(key=lambda r: (r["lines"], -r["uncovered"]))

    truncated = 0
    if not show_all and not substrings and len(shown) > top:
        truncated = len(shown) - top
        cutoff = shown[top]["lines"]
        shown = shown[:top]

    label = "coverage · worst first"
    if substrings:
        label += f" · matching {' '.join(substrings)}"
    print(f"── {label} ──")
    print()
    print(f"  {'lines':>7}  {'uncov':>6}  {'funcs':>7}  {'regions':>7}  file")
    for r in shown:
        print(
            f"  {r['lines']:>6.2f}%  {r['uncovered']:>6}  {r['functions']:>6.2f}%"
            f"  {r['regions']:>6.2f}%  {r['file']}"
        )
    print()
    if truncated:
        print(
            f"  ({truncated} more files at or above {cutoff:.2f}% lines — "
            f"`just coverage --all` for every row, `just coverage <substring>` for one)"
        )
        print()

    print(
        f"  TOTAL  {pct(totals, 'lines'):.2f}% lines · "
        f"{pct(totals, 'functions'):.2f}% functions · "
        f"{pct(totals, 'regions'):.2f}% regions   ({len(rows)} files)"
    )
    print()
    print("  Coverage is an input, not a target: it points at code nobody exercised, and")
    print("  the next question is whether that code states an invariant worth a test.")
    print("  Nothing here gates — see the justfile's `coverage` recipe for why.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
