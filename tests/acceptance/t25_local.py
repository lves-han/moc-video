"""Verify the user-authorized local T0.36 lifecycle observation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import sys
from urllib.parse import quote
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'packaging'))
import release
OBSERVATION = ROOT / 'evidence/acceptance/T25-local-lifecycle-20261110/observations.json'
USER_OBSERVATION = ROOT / 'evidence/acceptance/T25-local-observation-20261110.json'
SEED = ROOT / 'evidence/releases/T23-clean-macos'
IDS = ('U04', 'R30', 'R50', 'REQ-T25-LIFECYCLE')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def tree(root):
    result = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            path = Path(directory) / name
            relative = path.relative_to(root).as_posix()
            info = path.lstat()
            if path.is_symlink():
                result[relative] = ['link', os.readlink(path)]
            elif path.is_file():
                digest = hashlib.sha256()
                with path.open('rb') as stream:
                    for block in iter(lambda: stream.read(1024 * 1024), b''):
                        digest.update(block)
                result[relative] = ['file', stat.S_IMODE(info.st_mode), info.st_size, digest.hexdigest()]
    return result


def history(path):
    connection = sqlite3.connect('file:' + quote(str(path.resolve())) + '?mode=ro&immutable=1', uri=True)
    try:
        require(connection.execute('PRAGMA quick_check').fetchone()[0] == 'ok', 'Test history database is invalid')
        return (connection.execute('PRAGMA user_version').fetchone()[0],
                connection.execute('SELECT * FROM watch_history ORDER BY content_id').fetchall())
    finally:
        connection.close()


def verify():
    observed = json.loads(OBSERVATION.read_text())
    user = json.loads(USER_OBSERVATION.read_text())
    support = Path.home() / 'Library/Application Support'
    backup = Path(observed['original_backup']).resolve()
    require(backup.parent == support.resolve() and backup.name.startswith('.t036-backup-current-'),
            'Backup is outside the local test location')
    require(observed['environment'] == 'local-isolated-profile' and observed['clean_system'] is False,
            'The host test environment was not reported accurately')
    require(observed['status'] == 'PASS' and observed['security_exception_performed'] is False,
            'The local lifecycle did not complete without a security exception')
    events = {row['stage']: row for row in observed['events']}
    require(events['original-data-isolated']['clean_profile'] is True, 'Original data was not isolated')
    require(events['fresh-install']['installed_tree_matches'] and events['fresh-install']['process_alive']
            and events['fresh-install']['settings_created'], 'Fresh test profile did not initialize')
    require(user['user_playback_confirmation'] == '画面和声音正常' and user['installed_tree_matches_manifest'] is True,
            'User playback confirmation is missing')
    require(events['upgrade-before']['predecessor_process_started'] and events['upgrade-before']['version'] == 1,
            'Predecessor version did not run with a v1 profile')
    require(events['upgrade-after']['version'] == 2 and events['upgrade-after']['history_preserved']
            and events['upgrade-after']['settings_preserved'], 'Upgrade did not preserve test data')
    require(events['uninstall']['application_absent'] and events['uninstall']['test_data_preserved'],
            'Uninstall did not preserve the test profile')
    require(events['restore']['success'] and not events['restore']['errors'], 'Original files were not restored')
    manifest_path = (ROOT / user['package_manifest']).resolve()
    require(manifest_path.is_relative_to((ROOT / 'evidence/releases').resolve()), 'Release manifest path is invalid')
    delivery = json.loads((ROOT / 'evidence/acceptance/T25-all.json').read_text())['observed']['release']
    require(delivery['manifest'] == user['package_manifest']
            and hashlib.sha256(manifest_path.read_bytes()).hexdigest() == delivery['sha256'],
            'Local test package differs from the verified delivery')
    manifest = json.loads(manifest_path.read_text())
    require(release.tree(backup / 'test-current-fresh') == manifest['files']
            and release.tree(backup / 'test-current-upgraded') == manifest['files']
            and release.tree(backup / 'test-predecessor') == manifest['upgrade_from']['files'],
            'Test applications differ from the actual release and predecessor')
    for name, path in [('application', Path('/Applications/墨客视频.app')),
                       ('data', support / 'app.yingxiang.video')]:
        expected = json.loads((backup / (name + '-manifest.json')).read_text())
        require(path.is_dir() and tree(path) == expected, 'Restored ' + name + ' differs from backup')
    require(tree(backup / 'test-current-fresh') == tree(backup / 'test-current-upgraded'),
            'Fresh and upgraded applications differ')
    current_version, current_rows = history(backup / 'test-data-final/moc-video.db')
    old_version, old_rows = history(SEED / 'upgrade-before.db')
    require((old_version, current_version) == (1, 2) and old_rows and old_rows == current_rows,
            'Actual v1 to v2 history migration differs')
    old_settings = json.loads((SEED / 'upgrade-before-settings.json').read_text())
    new_settings = json.loads((backup / 'test-data-final/settings.json').read_text())
    ignored = {'window_bounds', 'metadata_preset_applied'}
    require({k: v for k, v in old_settings.items() if k not in ignored}
            == {k: v for k, v in new_settings.items() if k not in ignored}, 'Test settings changed during upgrade')
    return {'environment': 'local-isolated-profile', 'clean_system': False,
            'installation': 'PASS', 'upgrade': 'PASS', 'uninstall': 'PASS',
            'original_data_restored': True, 'user_playback_confirmation': user['user_playback_confirmation'],
            'audio_sampling': 'NOT RUN', 'history_rows': len(current_rows)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--junit', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = verify()
        status, error = 'PASS', None
    except Exception as exc:
        status, result, error = 'FAIL', {}, type(exc).__name__ + ': ' + str(exc)
    report = {'card': 'T0.36', 'case': 'local-lifecycle', 'status': status, 'result': result, 'error': error}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    suite = ET.Element('testsuite', tests=str(len(IDS)), failures=str(0 if status == 'PASS' else len(IDS)))
    for name in IDS:
        case = ET.SubElement(suite, 'testcase', name=name)
        if error:
            ET.SubElement(case, 'failure', message=error).text = error
    args.junit.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(suite).write(args.junit, encoding='utf-8', xml_declaration=True)
    print(json.dumps(report, ensure_ascii=False))
    return 0 if status == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
