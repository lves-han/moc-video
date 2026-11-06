"""Diagnostic isolation, not a replacement for the unmodified T20/A1 oracle."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

import t01
import t02
import t12
import t20


def profile(run, mode):
    previous = t01.DATA, t01.BUILD
    folder = run.folder / mode
    folder.mkdir()
    t01.DATA = folder / 'packaged'
    t01.DATA.mkdir()
    t01.BUILD = t01.DATA / 'build.json'
    session = bridge = None
    try:
        t01.check_media()
        t01.build(run)
        receipt = t01.check_build(run.source)
        executable = t12.ROOT / receipt['automation']['path'] / 'Contents/MacOS/yingxiang-video'
        with tempfile.TemporaryDirectory(prefix='profile-', dir=folder) as temporary:
            live = Path(temporary)
            media = live / 'media'
            media.mkdir()
            t12.owned_media(run, media)
            bridge = t12.Bridge(run, live, media)
            ready = bridge.wait(live / 'ready.json', timeout=180)
            with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                session = t01.NativeSession(run, fixture, executable, 1 if mode == 'official' else 2)
                session.wait(0, 30)
                session.command('t12', action='start', payload=ready, timeout=60)
                session.record['commands'][-1]['args'] = dict(action='start', payload='Host DTO; capabilities redacted')
                if mode == 'native-only':
                    session.command('t12', action='profile-native')
                samples = [t12.process_sample(session, bridge)]
                row = dict(mode=mode, diagnostic_only=True, samples=samples, cycles=[])
                run.observed.setdefault('profiles', []).append(row)
                for index in range(1, 51):
                    cycle = session.command('t12', action='switch', payload=index, timeout=40)
                    row['cycles'].append(cycle)
                    t12.validate_cycle(cycle, index)
                    samples.append(t12.process_sample(session, bridge))
                row['rss_windows'] = t12.rss_windows(samples)
                print(mode, 'MiB:', [round(value / 1048576, 2) for value in row['rss_windows']], flush=True)
                t12.validate_stop(session.command('t12', action='stop'))
                disposed = session.command('t12', action='dispose')
                t12.require(disposed['audio_closed'] and disposed['videos'] == 0 and not disposed['errors'], 'Diagnostic media/audio must be released')
                session.close()
        bridge.finish()
        return row
    finally:
        try:
            if session is not None and not session.closed:
                session.close(graceful=False)
        finally:
            if bridge is not None:
                bridge.close()
            t01.DATA, t01.BUILD = previous


def main():
    t12.require(t01.native_gui_session()['screen_locked'] is not True, 'Unlock before native profiling')
    t02.NATIVE |= {t12.LIVE_TEST.split('::')[-1]}
    run = t20.Acceptance(Path('evidence/acceptance/T24-memory-profile.json'), Path('evidence/acceptance/T24-memory-profile.xml'))
    for mode in ('official', 'native-only'):
        run.case('profile-' + mode, ['REQ-T20'], 'diagnostic isolation with fifty real decoded switches', lambda mode=mode: profile(run, mode))
    # Both runs retain the same numerical oracle. No profile can satisfy A1.
    for row in run.observed.get('profiles', []):
        run.case('rss-' + row['mode'], ['REQ-T20'], 'unchanged RSS oracle', lambda row=row: t12.validate_rss_windows(row['rss_windows']))
    return run.save()


if __name__ == '__main__':
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-di', '-w', str(os.getpid())])
    try:
        code = main()
    finally:
        awake.terminate()
        awake.wait(timeout=10)
    raise SystemExit(code)
