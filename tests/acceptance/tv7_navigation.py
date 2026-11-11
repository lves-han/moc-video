"""Diagnostic only: existing TV5 real window, preserved pre-fix/retest evidence."""
import argparse
import os
import subprocess
import sys
from pathlib import Path

import t01
import t12
import t14
import tv5
from t00 import require
from t10 import resources


class Acceptance(tv5.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.34', 'TV7-navigation', ['REQ-TV7-navigation']


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--expect', choices=['loop', 'order'], required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--junit', type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    command = t01.NativeSession.command

    def navigation(session, op, **kwargs):
        if op == 'tv5': kwargs['action'] = 'navigation'
        return command(session, op, **kwargs)

    def validate(row):
        require(row['chain'] == (['B', 'A', 'B'] if args.expect == 'loop' else ['B', 'A', 'home']), 'ordered return chain: ' + str(row))
        require(not row['errors'] and row['clean'], 'window errors or transition residue')
        if args.expect == 'order':
            require(row['original'] > 0 and abs(row['original'] - row['restored']) < 8, 'home scroll lost')
            require(row['browser'] == ['A', 'B', 'A', 'home'], 'browser Forward/Back lost visit order')

    if run.case('resource-ownership', run.REQUIREMENTS, 'original T0.34 runner', lambda: resources(run, card=run.CARD)):
        t01.NativeSession.command = navigation
        try:
            run.case('return-chain', run.REQUIREMENTS, 'home A B A then ordered returns; pre-fix mode preserves confirmed loop',
                     lambda: t14.packaged(run, tv5.LIVE_TEST, 'tv5', validate, 'navigation', lambda row: row,
                                          prepare_media=lambda runner, folder: t12.owned_media(runner, folder, seconds=20)))
        finally:
            t01.NativeSession.command = command
    return run.save()


if __name__ == '__main__':
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-di', '-w', str(os.getpid())]) if sys.platform == 'darwin' else None
    try:
        raise SystemExit(main())
    finally:
        if awake is not None: awake.terminate(); awake.wait(timeout=5)
