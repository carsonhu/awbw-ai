"""Enforce per-file line budgets on the docs.

Documentation that grows without bound stops being read: an agent session loads
CLAUDE.md every time, and a bloated set of docs crowds out the code it is meant
to explain. Budgets are deliberately tight -- when one is hit, the fix is
usually to cut something stale rather than to raise the number.

Exit code 1 if any file is over budget.
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Loaded every session, so it gets the tightest budget of all.
BUDGETS = {
    "CLAUDE.md": 45,
    "README.md": 60,
    "docs/architecture.md": 90,
    "docs/rules.md": 90,
    "docs/verification.md": 90,
    "docs/decisions.md": 110,
    "docs/workflow.md": 70,
}

# Everything under docs/ needs a budget, so a new file cannot slip in unbounded.
def main():
    failures = []
    rows = []

    for name, budget in sorted(BUDGETS.items()):
        path = ROOT / name
        if not path.exists():
            failures.append(f"{name}: missing")
            continue
        lines = len(path.read_text(encoding="utf-8").splitlines())
        rows.append((name, lines, budget))
        if lines > budget:
            failures.append(f"{name}: {lines} lines, budget {budget}")

    for doc in sorted((ROOT / "docs").glob("*.md")):
        rel = f"docs/{doc.name}"
        if rel not in BUDGETS:
            failures.append(f"{rel}: no budget set; add one to tools/check_docs.py")

    width = max((len(r[0]) for r in rows), default=0)
    total = sum(r[1] for r in rows)
    for name, lines, budget in rows:
        bar = "over" if lines > budget else f"{budget - lines} spare"
        print(f"  {name:<{width}}  {lines:>4} / {budget:<4} {bar}")
    print(f"  {'total':<{width}}  {total:>4} lines across {len(rows)} files")

    if failures:
        print("\nover budget:")
        for f in failures:
            print(f"  {f}")
        print("\nTrim stale content before raising a budget.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
