import argparse
import os
from pathlib import Path
import fnmatch


def parse_lcov(path: Path):
    files = {}
    cur = None

    def ensure(sf: str):
        if sf not in files:
            files[sf] = {"LF": 0, "LH": 0, "FNF": 0, "FNH": 0, "BF": 0, "BH": 0}
        return files[sf]

    with path.open("r", encoding="utf-8", errors="ignore") as f:
        for raw in f:
            line = raw.strip()
            if line.startswith("SF:"):
                cur = line[3:]
                ensure(cur)
                continue
            if cur is not None:
                for k in ("LF", "LH", "FNF", "FNH", "BF", "BH"):
                    if line.startswith(k + ":"):
                        try:
                            ensure(cur)[k] += int(line.split(":", 1)[1])
                        except Exception:
                            pass
                        break
            if line == "end_of_record":
                cur = None

    return files


def pct(hit: int, total: int) -> float:
    return (100.0 * hit / total) if total else 0.0


def main() -> int:
    ap = argparse.ArgumentParser(description="Summarize an LCOV coverage file")
    ap.add_argument(
        "--lcov",
        default=str(Path("target") / "coverage.lcov"),
        help="Path to LCOV file (default: target/coverage.lcov)",
    )
    ap.add_argument(
        "--root",
        default=os.getcwd(),
        help="Project root for path normalization (default: CWD)",
    )
    ap.add_argument(
        "--top",
        type=int,
        default=15,
        help="Number of lowest-covered files to show (default: 15)",
    )
    ap.add_argument(
        "--focus",
        action="append",
        default=[],
        help="Show detailed stats for a specific file path (can be repeated)",
    )
    ap.add_argument(
        "--min-file-line",
        type=float,
        default=None,
        help="Fail if any file has line coverage below this percentage",
    )
    ap.add_argument(
        "--include",
        action="append",
        default=[],
        help="Only consider files whose normalized relative path matches this glob (can be repeated)",
    )
    ap.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="Exclude files whose normalized relative path matches this glob (can be repeated)",
    )

    args = ap.parse_args()

    root = Path(args.root).resolve()
    lcov_path = Path(args.lcov)
    if not lcov_path.is_absolute():
        lcov_path = (root / lcov_path).resolve()

    if not lcov_path.exists():
        print(f"LCOV file not found: {lcov_path}")
        return 2

    files = parse_lcov(lcov_path)

    focus_set = set()
    for f in args.focus:
        p = Path(f)
        if not p.is_absolute():
            p = (root / p).resolve()
        focus_set.add(str(p))

    if focus_set:
        print("\nfocus:")
        for sf, v in sorted(files.items()):
            if str(Path(sf).resolve()) in focus_set:
                rel = sf
                try:
                    rel = str(Path(sf).resolve().relative_to(root))
                except Exception:
                    pass
                print(
                    f"  {rel}: lines {v['LH']}/{v['LF']} ({pct(v['LH'], v['LF']):.2f}%), "
                    f"funcs {v['FNH']}/{v['FNF']} ({pct(v['FNH'], v['FNF']):.2f}%)"
                )

    include_globs = args.include
    exclude_globs = args.exclude
    have_filters = bool(include_globs or exclude_globs)

    def should_consider(rel_path: str) -> bool:
        rel_norm = rel_path.replace("\\", "/")
        if include_globs and not any(fnmatch.fnmatch(rel_norm, g) for g in include_globs):
            return False
        if exclude_globs and any(fnmatch.fnmatch(rel_norm, g) for g in exclude_globs):
            return False
        return True

    items = []
    considered = []
    for sf, v in files.items():
        if v["LF"] <= 0:
            continue
        rel = sf
        try:
            rel = str(Path(sf).resolve().relative_to(root))
        except Exception:
            pass

        if not should_consider(rel):
            continue

        considered.append(v)

        items.append((pct(v["LH"], v["LF"]), v["LH"], v["LF"], rel))

    totals = {k: sum(v[k] for v in (considered if have_filters else files.values())) for k in ("LF", "LH", "FNF", "FNH", "BF", "BH")}

    print(f"files: {len(considered) if have_filters else len(files)}")
    print(f"lines: {totals['LH']}/{totals['LF']} ({pct(totals['LH'], totals['LF']):.2f}%)")
    print(f"functions: {totals['FNH']}/{totals['FNF']} ({pct(totals['FNH'], totals['FNF']):.2f}%)")
    if totals["BF"]:
        print(f"branches: {totals['BH']}/{totals['BF']} ({pct(totals['BH'], totals['BF']):.2f}%)")
    else:
        print("branches: n/a")

    items.sort(key=lambda t: (t[0], -t[2], t[3]))
    print(f"\nlowest line coverage (top {args.top}):")
    for cov, lh, lf, rel in items[: args.top]:
        print(f"  {cov:6.2f}%  {lh:5d}/{lf:<5d}  {rel}")

    if args.min_file_line is not None:
        offenders = [t for t in items if t[0] + 1e-9 < args.min_file_line]
        if offenders:
            print(f"\nfiles under min-file-line={args.min_file_line:.2f}%:")
            for cov, lh, lf, rel in offenders:
                print(f"  {cov:6.2f}%  {lh:5d}/{lf:<5d}  {rel}")
            return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
