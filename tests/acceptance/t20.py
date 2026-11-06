"""T20: real public metadata/playback plus controlled product and history regressions."""
import argparse
import os
from pathlib import Path
import n02
import t01
import t02
import t12
import t14
import t18
from t00 import require
from t10 import resources
ROOT = Path(__file__).resolve().parents[2]
IDS = ['U02','U03','U04','U05','U08','U09','R07','R10','R16','R20','R40','R41','R42','R43','R50','R62','R63','R66','R67','REQ-T20']
class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.24', 'T20', IDS
    DEPLOYMENT = 'real Host, SQLite and packaged WKWebView; public Chinese TMDB and supported JSON sources; controlled network/media regressions separately'
    def redact_output(self, output):
        return t02.redact(output)
def validate_public(e):
    p = e['public']
    require(e['recommendations'] > 0 and all(p[k] is True for k in ['chinese','confirmed','played','next','gateway','saved','poster','label']), 'Public metadata-to-playback chain incomplete: '+str(p))
    require(e['errors'] == [], 'Unhandled public page errors')
def public(run):
    os.environ['T12_MIN_OPENS'] = '1'
    try:
        return t14.packaged(run, n02.LIVE_TEST, 't20', validate_public, 'public_product', lambda e: e['public'], bridge_class=n02.Bridge, prepare_media=t12.owned_media, command_timeout=1500)
    finally:
        os.environ.pop('T12_MIN_OPENS', None)
def isolated(run, name, action):
    previous = run.folder
    run.folder = previous / name
    run.folder.mkdir()
    try:
        return action(run)
    finally:
        run.folder = previous
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--suite', choices=['all'], required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--junit', type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT/'evidence/acceptance/T20-all.json' and args.junit.resolve() == ROOT/'evidence/acceptance/T20-all.xml', 'Exact T20 report paths required')
    require(t01.native_gui_session()['screen_locked'] is not True, 'Unlock the desktop before T20 packaged acceptance')
    t02.NATIVE |= set(n02.REGRESSIONS) | t18.NATIVE | set(t18.SETTINGS) | set(t18.REGRESSIONS) | {n02.LIVE_TEST.split('::')[-1]}
    run = Acceptance(args.report, args.junit)
    if run.case('resource-ownership', ['REQ-T20'], 'project scope and resource ownership', lambda: resources(run, card='T0.24')):
        run.case('history-behaviors', ['U09','R40','R41','R42','R43','REQ-T20'], 'history identity, queue, fractional episodes and transient metadata posters', lambda: t18.frontend(run))
        run.case('sqlite-regressions', ['U09','R40','R41','R42','R43','REQ-T20'], 'real SQLite, restart, failures and identity', lambda: t18.native(run))
        run.case('network-session-regressions', ['U02','U05','R07','R10','R16','R20','R50','R63','REQ-T20'], 'media formats, conditional DASH, subtitles, faults and source isolation', lambda: n02.native(run))
        run.case('public-product-chain', ['U02','U03','U04','U08','R10','R16','R50','R62','R63','REQ-T20'], 'public metadata recommendation/details/confirmation/playback/next/history/poster', lambda: isolated(run, 'public-product', public))
        run.case('controlled-product-chain', ['U02','U03','U04','U05','U08','R07','R10','R16','R20','R50','R63','R66','R67','REQ-T20'], 'controlled source management, independent preferences, ambiguity and next pause', lambda: isolated(run, 'controlled-product', n02.webview))
        run.case('controlled-history-window', ['U09','R40','R41','R42','R43','REQ-T20'], 'real SQLite restart, resume and failed-save notice in product', lambda: isolated(run, 'controlled-history', t18.webview))
        run.case('public-source-capabilities', ['U02','U05','R07','R10','R16','REQ-T20'], 'all supported source public JSON and media observations, separately from fixture evidence', lambda: isolated(run, 'public-sources', t02.webview))
    return run.save()
if __name__ == '__main__':
    raise SystemExit(main())
