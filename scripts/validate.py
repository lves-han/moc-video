#!/usr/bin/env python3
"""Validate the control scripts and record actual tests/skips; no model calls."""
import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import unittest

REPO = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=REPO / "evidence/script-validation.json")
    options = parser.parse_args()
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    suite = unittest.defaultTestLoader.discover(str(REPO / "scripts/tests"), pattern="test_*.py")
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    env = os.environ.copy()
    env.update(PYTHONUTF8="1", PYTHONIOENCODING="utf-8")
    checks = []
    for name, args, expected in (("task", ["check"], 0), ("drive", ["status"], 0), ("run", [], 2), ("verify", ["invalid"], 2)):
        command = [sys.executable, "-B", str(REPO / "scripts" / (name + ".py")), *args]
        check = subprocess.run(command, cwd=REPO, env=env, capture_output=True, text=True, encoding="utf-8", timeout=20)
        checks.append(dict(argv=command, exit=check.returncode, expected=expected, stderr=check.stderr))
    passed = result.wasSuccessful() and result.testsRun > 0 and all(check["exit"] == check["expected"] for check in checks)
    report = dict(created=dt.datetime.now(dt.timezone.utc).isoformat(), platform=platform.platform(),
                  python=sys.version, result="PASS" if passed else "FAIL", tests_run=result.testsRun,
                  failures=[str(test) for test, _ in result.failures], errors=[str(test) for test, _ in result.errors],
                  skipped=[dict(test=str(test), reason=reason) for test, reason in result.skipped],
                  entrypoints=checks, online_agent="NOT RUN; local fixtures only", business_validation="NOT RUN")
    options.output.parent.mkdir(parents=True, exist_ok=True)
    options.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"Control validation {report['result']}: {options.output}")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
