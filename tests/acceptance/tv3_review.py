"""Run T0.30 review regressions/original behavior under T0.31's resource receipt."""
import argparse
import os
import subprocess
import sys
from pathlib import Path

import tv3
from t10 import resources


class Acceptance(tv3.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.31", "TV3-review", ["REQ-TV3"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["regressions", "baseline"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["REQ-TV3"], "original T0.31 runner and approved macOS arm64 budget", lambda: resources(run, card=run.CARD)):
        if args.suite == "baseline":
            run.case("original-native", ["REQ-TV3"], "unchanged four T0.30 native tests", lambda: tv3.native(run))
            run.case("original-frontend", ["REQ-TV3"], "unchanged five T0.30 frontend tests", lambda: tv3.frontend(run))
        run.case("packaged-pages", ["REQ-TV3"], "actual Host and packaged App review/original behavior with clean shutdown", lambda: tv3.windows(run, review_only=args.suite == "regressions"))
    return run.save()


if __name__ == "__main__":
    awake = subprocess.Popen(["/usr/bin/caffeinate", "-di", "-w", str(os.getpid())]) if sys.platform == "darwin" else None
    try:
        raise SystemExit(main())
    finally:
        if awake is not None:
            awake.terminate(); awake.wait(timeout=5)
