"""TV5: actual packaged details/transitions and production Host SQLite watchlist."""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import t01
import t12
import t14
import tv1
from t00 import Acceptance as CommandRunner, require
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
LIVE_TEST = 'host::maccms::tests::t08::t09::tv4::tv5::tv5_packaged_host_bridge'
CHILDREN = {
    'TV-DETAILS': ['fullbleed-layout', 'resume-label', 'episode-cards', 'cast-related', 'transition-fallback', 'visual-spec'],
    'TV-WATCHLIST': ['add-remove', 'persist-restart', 'migration-keeps-history', 'home-row', 'migration-failure', 'permissions-exact'],
}
IDS = [*CHILDREN, *[f'{p}/{n}' for p, names in CHILDREN.items() for n in names], 'REQ-TV5']


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.32', 'TV5', IDS
    DEPLOYMENT = 'actual packaged Tauri WKWebView and production Rust Host/gateway/SQLite; controlled owned JSON metadata and media'
    DEFERRED = [dict(target=target, status='NOT RUN', reason='ADR-005/006 macOS arm64 required locally') for target in t01.DEFERRED] + [
        dict(target='real macOS 14', status='NOT RUN', reason='API absence is exercised by disabling the API in this actual WKWebView; no macOS 14 machine'),
        dict(target='1920px', status='NOT RUN', reason='ADR-057 display logical width 1512')]
    command = CommandRunner.command


def case(run, parent, name, expected, function):
    return run.case(name, [parent, f'{parent}/{name}', 'REQ-TV5'], expected, function)


def security():
    main = json.loads((ROOT / 'src-tauri/capabilities/main.json').read_text())
    expected = tv1.PERMISSIONS + ['core:window:allow-start-dragging', 'allow-list-watchlist', 'allow-add-watchlist', 'allow-remove-watchlist']
    require(len(main['permissions']) == len(expected) and set(main['permissions']) == set(expected), 'permissions must equal TV1 plus exactly three watchlist permissions')
    require(set(main) == {'identifier', 'description', 'windows', 'permissions'} and main['identifier'] == 'main' and main['windows'] == ['main'], 'capability scope differs')
    config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
    require(config['app']['security'] == dict(capabilities=['main'], csp=tv1.CSP), 'CSP/security must equal the TV1 baseline')
    return dict(permissions=len(expected), csp='exact baseline')


def native(run):
    output = run.command(['cargo', 'test', '--locked', '--offline', '--manifest-path', 'src-tauri/Cargo.toml', '--lib', 'tv5_', '--', '--test-threads=1', '--nocapture', '--skip', 'tv5_packaged_host_bridge'], timeout=600)
    expected = {'tv5_watchlist_crud_restart_and_closed_inputs', 'tv5_migration_v1_keeps_all_history_fields', 'tv5_migration_failure_rolls_back_and_old_database_remains_usable'}
    names = re.findall(r'^test (?:\w+::)*(tv5_\w+) \.\.\.', output, re.M)
    require(set(names) == expected and 'test result: ok. 3 passed; 0 failed; 0 ignored;' in output, 'three named native watchlist/upgrade/failure behaviors must pass')
    rows = {}
    for marker in ['TV5_CRUD', 'TV5_UPGRADE', 'TV5_MIGRATION_FAILURE']:
        values = [json.loads(line.split(marker + ':', 1)[1]) for line in output.splitlines() if line.startswith(marker + ':')]
        require(len(values) == 1, 'one real native observation required: ' + marker)
        rows[marker] = values[0]
    run.observed['native'] = rows
    require(rows['TV5_UPGRADE'] == dict(before=1, after=2, history_fields=19, history_kept=True), 'upgrade must preserve every v1 history field')
    require(rows['TV5_MIGRATION_FAILURE'] == dict(explicit_error=True, old_version=1, old_data_usable=True, retry=2), 'failed transaction must leave old data usable and support explicit retry')
    return rows


def styles(row):
    s, width, height = row['styles'], row['width'], row['height']
    require(width in [960, 1200, 1512] and not row['overflow'] and row['title'] and row['origin'].startswith('http://127.0.0.1:'), 'actual nonblank packaged page/geometry differs')
    require(not row['errors'] and row['more'], 'page errors or missing long-plot action')
    head, copy = s['.details-head'], s['.details-copy']
    tv1.near(head['rect']['width'], width, 'fullbleed width'); tv1.near(head['rect']['left'], 0, 'fullbleed left')
    tv1.near(head['rect']['height'], max(520, min(height * .78, 820)), 'header height')
    tv1.near(s['.details-bottom-mask']['rect']['height'], head['rect']['height'] * .65, 'bottom fade 65%')
    tv1.near(head['rect']['bottom'] - copy['rect']['bottom'], 56, 'copy bottom inset')
    require(copy['max-width'] == '620px' and copy['rect']['width'] <= 620.1, 'copy max width')
    require('rgb(11, 11, 13)' in s['.details-bottom-mask']['background-image'] and 'rgba(11, 11, 13, 0.75)' in s['.details-left-mask']['background-image'], 'Hero mask colors changed')
    tv1.near(s['.details-copy h1']['font-size'], max(40, min(width * .044, 72)), 'Hero title size')
    require(s['.details-copy h1']['font-weight'] == '700' and s['.details-backdrop']['object-fit'] == 'cover', 'title weight/backdrop crop')
    require(s['.details-plot']['font-size'] == '15px' and s['.details-plot']['-webkit-line-clamp'] == '3', 'synopsis typography/clamp')
    require(row['meta_order'] == ['8.2 ★', '2026', 'Drama', '45 min', 'China / UK'], 'rating/year/genre/duration/region order differs')
    play = s['.details-play']; require(play['height'] == '48px' and play['min-width'] == '160px' and play['background-color'] == 'rgb(245, 245, 247)' and play['color'] == 'rgb(11, 11, 13)', 'primary button geometry/palette')
    require(s['.details-play-progress']['height'] == '3px', 'embedded progress must be 3px')
    tv1.near(s['.details-play-progress']['rect']['width'] / play['rect']['width'], .35, 'progress from actual history', .01)
    watch = s['.details-watchlist']; require(watch['width'] == watch['height'] == '48px' and watch['border-radius'] == '50%' and watch['background-color'] == 'rgba(30, 30, 34, 0.62)' and 'blur(30px)' in watch['-webkit-backdrop-filter'], '48px round glass watchlist control')
    require(s['.watchlist-symbol']['animation-duration'] == '0.2s', 'watchlist icon fade duration')
    selected, plain = s['.details-group[aria-pressed="true"]'], s['.details-group[aria-pressed="false"]']
    require(s['.details-groups']['overflow-x'] == 'auto' and selected['background-color'] == 'rgb(245, 245, 247)' and selected['color'] == 'rgb(11, 11, 13)' and plain['height'] == '36px' and plain['font-size'] == '14px' and plain['border-radius'] == '999px' and plain['background-color'] == 'rgba(255, 255, 255, 0.1)', 'season capsules/current state')
    tv1.near(s['.details-episode']['rect']['width'], max(260, min(width * .24, 380)), 'episode clamp width')
    require(s['.details-episode-art']['border-radius'] == '12px' and s['.details-episode strong']['font-size'] == '15px' and s['.details-episode strong']['font-weight'] == '600', 'episode image/title')
    require(s['.details-episode-plot']['font-size'] == '13px' and s['.details-episode-plot']['-webkit-line-clamp'] == '2' and s['.details-episode-duration']['font-size'] == '13px' and s['.details-episode-duration']['color'] == 'rgba(235, 235, 245, 0.62)', 'episode plot/duration typography')
    require(s['.details-episode-fallback']['filter'] == 'blur(12px)' and s['.details-episode-number']['font-size'] == '48px' and row['episodes'][2]['video'] == 'video:1:3', 'episode image fallback or video mapping')
    require(s['.details-watching']['background-color'] == s['.details-play-progress']['background-color'] and s['.details-episode-progress']['height'] == '3px', 'current badge and saved progress accent')
    actor = s['.details-actor img']; require(actor['width'] == actor['height'] == '88px' and actor['border-radius'] == '50%' and s['.details-actor-initial']['width'] == '88px' and s['.details-actor p']['font-size'] == '13px' and s['.details-actor p']['-webkit-line-clamp'] == '2' and s['.details-actor-role']['font-size'] == '12px' and s['.details-actor-role']['color'] == 'rgba(235, 235, 245, 0.62)', 'actor image/initial/name/role')
    require(len(s['.details-information dl']['grid-template-columns'].split()) == (2 if width == 960 else 3) and s['.details-information dl']['column-gap'] == '32px' and s['.details-information dt']['font-size'] == '12px' and s['.details-information dt']['color'] == 'rgba(235, 235, 245, 0.62)' and s['.details-information dd']['font-size'] == '14px', 'information responsive grid/type')
    require(s['.details-content']['animation-duration'] == '0.3s' and s['.details-content']['animation-delay'] == '0.1s', 'content delayed fade')
    require(row['cast'] == 2 and row['related'] > 0, 'actual cast/related rows missing')
    return dict(width=width, cast=row['cast'], related=row['related'])


def resume_label(row):
    record = row['history']; require(record['video_id'] == 'video:1:3' and record['episode_number'] == 3 and record['position_ms'] == 7000 and record['duration_ms'] == 20000 and row['label'] == 'Continue playing · Episode 3 · 1 min left', 'primary continuation must match durable video/episode/position/duration')


def modal_states(row):
    for dialog in [row['plot']['style'], row['confirm']['style']]:
        require(dialog['border-radius'] == '20px' and dialog['max-width'] == '640px' and dialog['background-color'] == 'rgba(30, 30, 34, 0.62)' and 'blur(30px)' in dialog['-webkit-backdrop-filter'], 'glass dialog width/radius/background')
    require(row['plot']['content']['padding-left'] == row['plot']['content']['padding-right'] == '28px', 'synopsis dialog padding')
    require(row['season']['episodes'] == 1 and row['confirm']['candidate']['height'] == '36px' and row['confirm']['candidate']['border-radius'] == '999px' and row['confirm']['candidate']['font-size'] == '14px' and row['confirm']['candidate']['background-color'] == 'rgb(245, 245, 247)' and not row['errors'], 'season/candidate selection or confirmation cancel')


def transitions(row):
    require(not row['errors'] and row['clean'] and abs(row['restored'] - row['native']['original']) < 8 and abs(row['native']['restored'] - row['native']['original']) < 8 and row['native']['original'] > 0, 'return transition must restore scroll and clean temporary DOM')
    if row['native']['supported']:
        observations = [r for r in row['native']['observations'] if r['ready']]
        require(len(observations) >= 2 and all(r['before'] == r['after'] == 1 and r['duration'] == '0.3s' for r in observations), 'real native poster/detail shared-element transition must run at 300ms')
    fallback = row['fallback']; require(any(a['duration'] == 300 and a['frames'] == ['1', '0'] for a in fallback['old']) and any(a['duration'] == 300 and a['frames'] == ['0', '1'] for a in fallback['next']), 'disabled API must crossfade both old and new pages for 300ms')


def window(run):
    previous = t01.DATA, t01.BUILD, t12.LIVE_TEST
    t01.DATA = run.folder / 'packaged'; t01.DATA.mkdir(); t01.BUILD = t01.DATA / 'build.json'; t12.LIVE_TEST = LIVE_TEST
    session = bridge = None
    try:
        require(t01.native_gui_session()['screen_locked'] is not True, 'Unlock macOS for required real-window acceptance')
        t01.check_media(); t01.build(run); receipt = t01.check_build(run.source)
        executable = ROOT / receipt['automation']['path'] / 'Contents/MacOS/yingxiang-video'
        run.command(['clang', '-fobjc-arc', '-framework', 'Cocoa', '-framework', 'ApplicationServices', 'tests/fixtures/tv1_native.m', '-o', str(run.folder / 'tv1-native')])
        with tempfile.TemporaryDirectory(prefix='tv5-live-', dir=run.folder) as temporary:
            folder = Path(temporary); media = folder / 'media'; media.mkdir(); t12.owned_media(run, media, seconds=20)
            bridge = t14.Bridge(run, folder, media); ready = bridge.wait(folder / 'ready.json', timeout=180)
            with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                session = t01.NativeSession(run, fixture, executable, 1); session.wait(0, 30); session.command('identity')
                layouts, screenshots = [], []
                for width in [960, 1200, 1512]:
                    session.command('layout', value=width); row = session.command('tv5', action='styles', payload=ready, timeout=120); layouts.append(row)
                    screenshots.append(tv1.capture(run, session.process.pid, width, row['native']['scale_factor']))
                run.observed.update(layouts=layouts, screenshots=screenshots)
                for name in ['fullbleed-layout', 'episode-cards', 'cast-related', 'visual-spec']:
                    case(run, 'TV-DETAILS', name, 'ADR-057 exact computed geometry/colors/type/states at 960/1200/1512 and owned screenshots', lambda: [styles(row) for row in layouts])
                case(run, 'TV-DETAILS', 'resume-label', 'primary label/video and 3px progress use the exact saved episode, position and remaining duration', lambda: [resume_label(row) for row in layouts])
                states = session.command('tv5', action='states', payload=ready, timeout=120); run.observed['states'] = states
                case(run, 'TV-DETAILS', 'visual-spec', 'real More glass dialog, season change and selected match confirmation candidate/cancel', lambda: modal_states(states))
                header = ROOT / screenshots[-1]['path']; header_target = header.with_name('details-header-1512.png'); header.rename(header_target)
                screenshots[-1]['path'] = str(header_target.relative_to(ROOT))
                lower = session.command('tv5', action='lower', payload=ready); run.observed['lower'] = lower
                lower_shot = tv1.capture(run, session.process.pid, 1512, layouts[-1]['native']['scale_factor'])
                lower_path = ROOT / lower_shot['path']; lower_target = lower_path.with_name('details-lower-1512.png'); lower_path.rename(lower_target)
                lower_shot['path'] = str(lower_target.relative_to(ROOT)); screenshots.append(lower_shot)
                added = session.command('tv5', action='add', payload=ready, timeout=120); run.observed['added'] = added
                case(run, 'TV-WATCHLIST', 'add-remove', 'real add persists one durable entry and fades to check', lambda: require(len(added['saved']['entries']) == 1 and added['saved']['entries'][0]['content_id'] == ready['record']['content_id'] and added['icon']['animation-duration'] == '0.2s', 'add/check or entry identity'))
                case(run, 'TV-WATCHLIST', 'home-row', 'watchlist follows continue watching and uses vertical posters', lambda: require(added['count'] == 1 and added['after_history'] and abs(added['poster']['rect']['height'] / added['poster']['rect']['width'] - 1.5) < .01, 'watchlist home placement/poster'))
                session.command('identity'); session.close()
                require(session.record.get('exit') == 0 and session.record.get('lingering') == [], 'first owned App/WebKit session did not clean up')
                restarted = bridge.invoke(dict(operation='restart')); run.observed['restarted'] = restarted
                require(restarted['old_epoch'] != restarted['new_epoch'] and len(restarted['watchlist']['entries']) == 1 and restarted['history']['entries'] == [ready['record']], 'new Host must reload exact saved watchlist/history')
                session = t01.NativeSession(run, fixture, executable, 2); session.wait(0, 30); session.command('identity')
                after = session.command('tv5', action='after-restart', payload=ready, timeout=120); run.observed['after_restart'] = after
                case(run, 'TV-WATCHLIST', 'persist-restart', 'close actual packaged App, reopen Host/SQLite with new epoch, relaunch App and reopen saved title', lambda: require(after['before']['count'] == 1 and after['before']['epoch'] == restarted['new_epoch'] and after['before']['entries'] == added['saved']['entries'] and after['label'] == 'Continue playing · Episode 3 · 1 min left', 'restarted watchlist/details identity or continuation'))
                for name in ['add-remove', 'home-row']:
                    case(run, 'TV-WATCHLIST', name, 'actual remove commits and empty home row is hidden', lambda: require(after['after'] == dict(entries=[]) and after['hidden'], 'removed item/empty row remains'))
                motion = session.command('tv5', action='transitions', payload=ready, timeout=120); run.observed['transitions'] = motion
                case(run, 'TV-DETAILS', 'transition-fallback', 'real shared-element transition and disabled-API 300ms crossfade both restore scroll', lambda: transitions(motion))
                played = session.command('tv5', action='resume', payload=ready, timeout=180); run.observed['playback'] = played
                case(run, 'TV-DETAILS', 'resume-label', 'primary resume rematches the saved episode and actual official player decodes at saved position', lambda: require(played['played']['video'] == ready['record']['video_id'] and played['played']['decoded'] and 6800 <= played['played']['position'] < 10000 and played['history']['video_id'] == ready['record']['video_id'] and not played['errors'], 'actual primary resume differs from history'))
                require(session.command('tv5', action='errors', payload=ready) == [], 'actual window raised uncaught errors')
                session.command('identity'); session.close()
                require(session.record.get('exit') == 0 and session.record.get('lingering') == [] and session.record.get('peak_combined_rss_bytes', 0) > 0 and fixture.active == 0, 'owned App/WebKit/HTTP cleanup/RSS missing')
            bridge.finish()
        return dict(window='actual packaged WKWebView', screenshots=screenshots, restart='App and production Host reopened with new epoch')
    finally:
        try:
            if session is not None and not session.closed: session.close(graceful=False)
        finally:
            if bridge is not None: bridge.close()
            t01.DATA, t01.BUILD, t12.LIVE_TEST = previous


def main():
    parser = argparse.ArgumentParser(); parser.add_argument('--suite', choices=['all', 'native', 'window'], required=True)
    parser.add_argument('--report', type=Path, required=True); parser.add_argument('--junit', type=Path, required=True); args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    if args.suite != 'all': run.REQUIREMENTS = ['REQ-TV5']
    if run.case('resource-ownership', ['REQ-TV5'], 'original runner, actual macOS arm64 and approved RSS budget', lambda: resources(run, card=run.CARD)):
        case(run, 'TV-WATCHLIST', 'permissions-exact', 'TV1 permission set plus exactly three watchlist IPCs; CSP matches baseline', security)
        if args.suite in ['all', 'native']:
            requirements = ['TV-WATCHLIST', 'TV-WATCHLIST/migration-keeps-history', 'TV-WATCHLIST/migration-failure', 'TV-WATCHLIST/persist-restart', 'TV-WATCHLIST/add-remove', 'REQ-TV5']
            run.case('native-watchlist', requirements, 'three real Host CRUD/restart, v1 upgrade and failed-migration rollback behaviors', lambda: native(run))
        if args.suite in ['all', 'window']:
            run.case('packaged-details', ['TV-DETAILS', 'TV-WATCHLIST', 'REQ-TV5'], 'actual details, durable watchlist, App restart, transitions and official playback with cleanup', lambda: window(run))
    return run.save()


if __name__ == '__main__':
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-di', '-w', str(os.getpid())]) if sys.platform == 'darwin' else None
    try:
        raise SystemExit(main())
    finally:
        if awake is not None: awake.terminate(); awake.wait(timeout=5)
