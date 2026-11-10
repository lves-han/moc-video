"""T20: real public metadata/playback plus controlled product and history regressions."""
import argparse
import os
import subprocess
import sys
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
CACHE_NATIVE = {
    't24_media_cache_complete_range_keys_and_restart',
    't24_media_cache_lru_limit_and_partial_reservations',
    't24_media_cache_clear_and_lower_limit_barrier_keeps_readers',
    't24_media_cache_http_policy_and_recovery',
    't24_media_cache_directory_ownership_and_corrupt_read',
    't24_cache_gateway_hits_ranges_headers_and_clear',
    't24_cache_gateway_hls_does_not_persist_keys_or_manifests',
    't24_cache_gateway_contexts_policy_and_unusable_disk',
    't24_cache_gateway_clear_while_cached_http_read_continues',
    't24_cache_gateway_clear_before_upstream_response_blocks_old_fill',
    't24_cache_host_settings_permissions_and_restart',
    't24_cache_host_limit_and_clear_keep_active_session',
}
def cache_native(run):
    import re
    output = run.command(['cargo','test','--locked','--offline','--manifest-path','src-tauri/Cargo.toml','--lib','t24_','--','--test-threads=1','--nocapture'], timeout=600)
    names = {name.split('::')[-1] for name in re.findall(r'^test (\S+) \.\.\.',output,re.M)}
    require(names == CACHE_NATIVE and f'test result: ok. {len(CACHE_NATIVE)} passed; 0 failed; 0 ignored;' in output, 'All cache request-count, disk, clear, restart and permission tests must execute')
    return dict(required=len(CACHE_NATIVE), passed=len(CACHE_NATIVE))
class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.24', 'T20', IDS
    DEFERRED = [dict(target='macOS x86_64 / Windows / Linux', status='NOT RUN', reason='ADR-005/006 local macOS arm64')]
    DEPLOYMENT = 'real Host, SQLite and packaged WKWebView; public Chinese TMDB and supported JSON sources; controlled network/media regressions separately'
    def redact_output(self, output):
        return t02.redact(output)
def validate_public(e):
    p = e['public']
    require(p['official_version'] == '10.3.0' and p['official_controls'] and p['native_fullscreen'], 'Official Video.js and native fullscreen controls must work')
    require(all(e['backup'][key] for key in ['registered','chinese_details','videos','primary_kept']), 'Backup must register and return Chinese video metadata without changing the primary')
    require(e['recommendations'] > 0 and all(p[k] is True for k in ['chinese','confirmed','played','next','gateway','saved','poster','label','icon_play_pause','icon_mute','keyboard_focus']), 'Public metadata-to-playback chain incomplete: '+str(p))
    require(len(p['icon_buttons']) == 5 and all(button['icon'] and button['tooltip'] and button['accessible'] and button['width'] >= 44 for button in p['icon_buttons']), 'Playback icon controls need hover text, accessible labels and usable hit targets')
    require(all(button['tooltip_open'] is True and button['tooltip_text'].strip() for button in p['icon_buttons'] if button['action'] in ['play','mute','fullscreen']), 'Actual official tooltips must open with visible text')
    require(e['errors'] == [], 'Unhandled public page errors')
    require(len(e['layouts']) == 8 and any(row['width'] <= 390 for row in e['layouts']) and all(not row['overflow'] for row in e['layouts']), 'Bilingual light/dark 390px layouts must fit')
def public(run, *, build_folder=None):
    os.environ['T12_MIN_OPENS'] = '1'
    try:
        return t14.packaged(run, n02.LIVE_TEST, 't20', validate_public, 'public_product', lambda e: e['public'], bridge_class=n02.Bridge, prepare_media=t12.owned_media, command_timeout=1500, build_folder=build_folder)
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
def controlled_history(run, *, build_folder=None):
    os.environ['T12_MIN_OPENS'] = '1'
    try:
        return t14.packaged(run, n02.LIVE_TEST, 't18', validate_history, 'history', lambda e: dict(cross_season=e['cross_season'], source_switch=e['source_switch'], eos=e['eos']), bridge_class=n02.Bridge, prepare_media=t12.owned_media, command_timeout=600, after_close=t18.closed_window_kept_everything, build_folder=build_folder)
    finally:
        os.environ.pop('T12_MIN_OPENS', None)

def validate_history(e):
    t18.validate(e)
    require(e['cache_limit']['invalid_refused'] and e['cache_limit']['failed_save_kept_limit'] and e['cache_limit']['saved'] == 1048576 and e['cache_limit']['restored'] and e['cache_limit']['used'] <= 1048576, 'Cache limit must save, apply and restore')
    require(all(e['cache_clear'][key] for key in ['failed_clear_kept_files','cleared','disabled','playback_after_clear']), 'Cache clear/off must change real occupancy and preserve playback')
    require(e['cross_season'] == dict(season=2, episode=1, distinct_video=True), 'Cross-season video identity must survive SQLite restart and resume')
    require(e['eos'] == dict(current_video=True, completed=True), 'Current video EOS must persist completion')
    require(all(e['source_switch'][key] for key in ['rematched','confirmed_before_open','history_kept']), 'Source switch must rematch and keep history')
def videojs_lifecycle(run):
    previous = t01.DATA, t01.BUILD
    t01.DATA = run.folder / 'packaged'
    t01.DATA.mkdir()
    t01.BUILD = t01.DATA / 'build.json'
    try:
        return t12.webview(run)
    finally:
        t01.DATA, t01.BUILD = previous
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--suite', choices=['all'], required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--junit', type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT/'evidence/acceptance/T20-all.json' and args.junit.resolve() == ROOT/'evidence/acceptance/T20-all.xml', 'Exact T20 report paths required')
    require(t01.native_gui_session()['screen_locked'] is not True, 'Unlock the desktop before T20 packaged acceptance')
    t02.NATIVE |= CACHE_NATIVE | set(n02.REGRESSIONS) | t18.NATIVE | set(t18.SETTINGS) | set(t18.REGRESSIONS) | {n02.LIVE_TEST.split('::')[-1], t12.LIVE_TEST.split('::')[-1]}
    run = Acceptance(args.report, args.junit)
    if run.case('resource-ownership', ['REQ-T20'], 'project scope and resource ownership', lambda: resources(run, card='T0.24')):
        run.case('media-cache-storage-and-gateway', ['U09','R10','R16','REQ-T20'], 'real upstream request counts, disk files, ranges/context/auth, LRU, restart, in-flight clear and settings permissions', lambda: cache_native(run))
        run.case('cache-wire-contracts', ['U09','REQ-T20'], 'old IPC preserved and both new cache commands have positive/negative cases', lambda: t18.contracts(run))
        run.case('history-behaviors', ['U09','R40','R41','R42','R43','REQ-T20'], 'history identity, queue, fractional episodes and transient metadata posters', lambda: t18.frontend(run))
        run.case('sqlite-regressions', ['U09','R40','R41','R42','R43','REQ-T20'], 'real SQLite, restart, failures and identity', lambda: t18.native(run))
        run.case('network-session-regressions', ['U02','U05','R07','R10','R16','R20','R50','R63','REQ-T20'], 'media formats, conditional DASH, subtitles, faults and source isolation', lambda: n02.native(run))
        run.case('official-videojs-lifecycle', ['U08','U09','R10','R16','R50','REQ-T20'], 'official playback with fifty real switches, subtitles, recovery, stop and disposal', lambda: isolated(run, 'videojs-lifecycle', videojs_lifecycle))
        run.case('public-product-chain', ['U02','U03','U04','U08','R10','R16','R50','R62','R63','REQ-T20'], 'public metadata recommendation/details/confirmation/playback/next/history/poster', lambda: isolated(run, 'public-product', public))
        run.case('controlled-product-chain', ['U02','U03','U04','U05','U08','R07','R10','R16','R20','R50','R63','R66','R67','REQ-T20'], 'controlled source management, independent preferences, ambiguity and next pause', lambda: isolated(run, 'controlled-product', n02.webview))
        run.case('controlled-history-window', ['U09','R40','R41','R42','R43','REQ-T20'], 'real SQLite restart, resume and failed-save notice in product', lambda: isolated(run, 'controlled-history', controlled_history))
        run.case('public-source-capabilities', ['U02','U05','R07','R10','R16','REQ-T20'], 'all supported source public JSON and media observations, separately from fixture evidence', lambda: isolated(run, 'public-sources', t02.webview))
    return run.save()
if __name__ == '__main__':
    # Keep an unattended packaged run awake; the owned helper exits with this process.
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-di', '-w', str(os.getpid())]) if sys.platform == 'darwin' else None
    try:
        code = main()
    finally:
        if awake is not None:
            awake.terminate()
            awake.wait(timeout=10)
    raise SystemExit(code)
