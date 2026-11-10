"""Independent corruption examples for the final raw-evidence acceptance."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import t24
from test_t22 import playback
from test_t12 import cycle, stopped


def samples():
    rows=playback()
    for row in rows:
        row['value']['focused']=True
        row['value']['native']['audio_playing']=True
        row['value']['sample'].update(pixel=[255,0,0,255],diagnostics=dict(engine='native'))
    return rows


def lifecycles():
    rows=[]
    for i in range(1,6):
        native=dict(pid=i,webkit_pids=dict(renderer=i+10),audio_playing=True)
        played=dict(sample=samples()[0]['value']['sample'],native=native)
        played['sample']['frames']=3
        switch=cycle();switch.update(index=51,native=native)
        switch['sample'].update(pixel=[255,0,0,255],audio_peak_hz=440)
        session=dict(pid=i,identity='owned-'+str(i),exit=0,lingering=[],forced_cleanup=[],wake_guard=dict(exit=0),peak_combined_rss_bytes=4096)
        rows.append(dict(index=i,kind='hls' if i%2==0 else 'mp4',audio_sampling=t24.AUDIO,session=session,identity=dict(origin='tauri://localhost',native=native),
            stop=stopped(),disposed=dict(audio_closed=True,videos=0,errors=[]),host=dict(closed=True,active_requests=0,opens=1),
            transport=dict(active_requests=0),played=played,switch=switch))
    return rows


def source():
    return dict(name='synthetic',records=[dict(stage='list',status='JSON_CAPTURED',response=dict(code=1),source_error=False,
        adapter_decode=dict(status='PASS')),dict(stage='adapter_details',status='PASS',groups=1,episodes=2),
        dict(stage='direct_media_prefix',status='PREFIX_ONLY',host='owned.invalid')],error=None,
        compatibility='COMPATIBLE',supported=True,media_samples=dict(attempted=1,passed=1,failed_hosts=[],verdict='MEDIA_AVAILABLE'))


class T24Tests(unittest.TestCase):
    def test_two_complete_formats_recompute_from_raw_values(self):
        for kind in ['mp4','hls']:
            self.assertEqual(t24.check_playback(samples(),kind)['seconds'],600)

    def test_short_missing_or_stalled_sample_fails(self):
        for change in ['short','missing','stalled','gap']:
            rows=samples()
            if change=='short': rows[-1]['host_monotonic']-=1
            elif change=='missing':rows.pop()
            elif change=='stalled':rows[20]['value']['sample']['frames']=rows[19]['value']['sample']['frames']
            else:rows[20]['host_monotonic']=rows[19]['host_monotonic']+16
            with self.subTest(change=change),self.assertRaises(ValueError):t24.check_playback(rows,'mp4')

    def test_foreign_session_and_gateway_fail(self):
        for key,value in [('session_id','foreign'),('actual_gateway',False),('playback_rate',2)]:
            rows=samples();rows[20]['value'][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):t24.check_playback(rows,'mp4')

    def test_foreground_and_ui_fail(self):
        for change in ['focus','native-focus','progress']:
            rows=samples();row=rows[20]['value']
            if change=='focus':row['focused']=False
            elif change=='native-focus':row['native']['focused']=False
            else:row['ui_position']+=2
            with self.subTest(change=change),self.assertRaises(ValueError):t24.check_playback(rows,'hls')

    def test_changed_pixels_and_media_fail(self):
        for key,value in [('pixel',[0,0,0,255]),('paused',True),('video_count',2),('media_error',3)]:
            rows=samples();rows[20]['value']['sample'][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):t24.check_playback(rows,'mp4')

    def test_control_audit_cannot_hide_old_input(self):
        rows=samples();rows[30]['value']['input_audit']['count']=65
        with self.assertRaises(ValueError):t24.check_playback(rows,'mp4')

    def test_missing_process_and_cpu_samples_fail(self):
        for key in ['processes','support_processes']:
            rows=samples();rows[30]['process'][key]=[]
            with self.subTest(key=key),self.assertRaises(ValueError):t24.check_playback(rows,'mp4')

    def test_stale_source_protected_or_generation_fail(self):
        current=dict(source='source',protected='protected',generation=1)
        t24.check_binding(current,current)
        for key in current:
            changed=dict(current);changed[key]='old'
            with self.subTest(key=key),self.assertRaises(ValueError):t24.check_binding(changed,current)

    def test_raw_file_missing_changed_size_hash_and_escape_fail(self):
        with tempfile.TemporaryDirectory(dir=t24.ROOT/'evidence') as temporary:
            folder=Path(temporary);path=folder/'raw';path.write_bytes(b'original')
            row=t24.artifact(path);self.assertEqual(t24.checked_file(row,folder),path)
            changed=dict(row,bytes=1)
            with self.assertRaises(ValueError):t24.checked_file(changed,folder)
            path.write_bytes(b'modified')
            with self.assertRaises(ValueError):t24.checked_file(row,folder)
            with self.assertRaises(ValueError):t24.checked_file(dict(row,path='../raw'),folder)
            path.unlink()
            with self.assertRaises(ValueError):t24.checked_file(row,folder)

    def test_empty_partial_duplicate_or_forced_lifecycles_fail(self):
        self.assertEqual(t24.check_lifecycles(lifecycles())['windows'],5)
        for change in ['missing','duplicate','forced','lingering','omitted-skip','frame','switch']:
            rows=lifecycles()
            if change=='missing':rows.pop()
            elif change=='duplicate':rows[4]['session']=rows[0]['session']
            elif change=='forced':rows[0]['session']['forced_cleanup']=[1]
            elif change=='lingering':rows[0]['session']['lingering']=[1]
            elif change=='omitted-skip':rows[3].pop('audio_sampling')
            elif change=='frame':rows[4]['played']['sample']['pixel']=[0,0,255,255]
            else:rows[1]['switch']['host']['old_http_status']=200
            with self.subTest(change=change),self.assertRaises((ValueError,KeyError)):t24.check_lifecycles(rows)

    def test_authorized_audio_skip_keeps_frame_ui_and_ownership_gates(self):
        for kind in ['mp4','hls']:
            rows=samples()
            for row in rows:
                row['value']['sample'].update(audio_rms=0,audio_peak_hz=0)
                row['value']['native']['audio_playing']=False
            self.assertEqual(t24.check_playback(rows,kind)['samples'],121)
        self.assertEqual(t24.AUDIO['status'],'SKIPPED_USER_AUTHORIZED')

    def test_latest_review_requires_final_clear_and_one_binding(self):
        for text in ['', 'ACCEPTANCE-VERDICT: BLOCKING','ACCEPTANCE-VERDICT: CLEAR\nextra','ACCEPTANCE-VERDICT: CLEAR']:
            with self.subTest(text=text),self.assertRaises((ValueError,IndexError)):
                t24.check_review(text,{},Path('missing'),{})

    def test_original_receipt_rejects_partial_failed_old_or_wrong_payload(self):
        current=dict(source='s',protected='p',generation=0)
        with tempfile.TemporaryDirectory(dir=t24.ROOT/'evidence') as temporary:
            log=Path(temporary)/'original.log';log.write_text('T24_MANIFEST_SHA256:hash\n')
            base=dict(current,card='T0.35',id='t24-endurance',status='finished',exit=0,raw_exit=0,after_source='s',
                argv=['{python}','-B','tests/acceptance/t24.py','--suite','endurance','--report','evidence/acceptance/T24-endurance.json','--junit','evidence/acceptance/T24-endurance.xml'],
                log=log.relative_to(t24.ROOT).as_posix(),log_hash=t24.file_hash(log),
                environment=dict(backend='process-watchdog',memory_enforcement='rss-sampled',memory_mib=4096,jobs=2,test_threads=1))
            t24.check_original(base,'hash',current)
            for key,value in [('status','running'),('exit',1),('raw_exit',1),('after_source','old'),('card','T0.2'),('id','A1'),('argv',['fake']),('log_hash','changed')]:
                row=dict(base);row[key]=value
                with self.subTest(key=key),self.assertRaises(ValueError):t24.check_original(row,'hash',current)
            with self.assertRaises(ValueError):t24.check_original(base,'other-manifest',current)
            log.write_text('changed')
            with self.assertRaises(ValueError):t24.check_original(base,'hash',current)

    def test_review_valid_binding_then_stale_commit_manifest_raw_or_omitted_obligation_fail(self):
        with tempfile.TemporaryDirectory(dir=t24.ROOT/'evidence') as temporary:
            folder=Path(temporary)
            for name in ['raw','manifest','release','receipt','log']:(folder/name).write_text(name)
            receipt=folder/'receipt';log=folder/'log';t24.write_json(receipt,dict(log=log.relative_to(t24.ROOT).as_posix()))
            path=folder/'manifest';current=dict(source='s',protected='p',generation=0)
            manifest=dict(current,commit='commit',files=[t24.artifact(folder/'raw')],receipt=receipt.relative_to(t24.ROOT).as_posix(),release=t24.artifact(folder/'release'))
            files={item.relative_to(t24.ROOT).as_posix():t24.file_hash(item) for item in folder.iterdir()}
            good=dict(current,commit='commit',manifest=t24.artifact(path),files=files,audio_sampling=t24.AUDIO,unresolved_blockers=[],personal_use_only=True,
                public_distribution_authorized=False,obligations={key:dict(result='PASS',evidence=['synthetic raw'],reason='synthetic test') for key in t24.OBLIGATIONS})
            def text(value):return 'independent synthetic test\n```json\n'+json.dumps(value)+'\n```\nACCEPTANCE-VERDICT: CLEAR\n'
            self.assertEqual(t24.check_review(text(good),manifest,path,current)['commit'],'commit')
            for change in ['commit','manifest','files','obligation','blocker','public','audio-overclaim']:
                bad=copy.deepcopy(good)
                if change=='commit':bad['commit']='old'
                elif change=='manifest':bad['manifest']['sha256']='changed'
                elif change=='files':bad['files'].pop(next(iter(bad['files'])))
                elif change=='obligation':bad['obligations'].pop('U09')
                elif change=='blocker':bad['unresolved_blockers']=['high impact issue']
                elif change=='public':bad['public_distribution_authorized']=True
                else:bad['audio_sampling']['status']='PASS'
                with self.subTest(change=change),self.assertRaises(ValueError):t24.check_review(text(bad),manifest,path,current)

    def test_latest_numeric_review_never_falls_back_to_old_clear(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder=Path(temporary)
            (folder/'round-9.md').write_text('ACCEPTANCE-VERDICT: CLEAR')
            (folder/'round-10.md').write_text('ACCEPTANCE-VERDICT: BLOCKING')
            self.assertEqual(t24.latest_review(folder).name,'round-10.md')
            with self.assertRaises(ValueError):t24.check_review(t24.latest_review(folder).read_text(),{},Path('none'),{})

    def test_source_summary_recomputed_from_records(self):
        self.assertEqual(t24.check_source(source())['compatibility'],'COMPATIBLE')
        for change in ['summary','adapter','source-error','outer-error','defect','exhausted']:
            row=source()
            if change=='summary':row['media_samples']['passed']=0
            elif change=='adapter':row['records'][0]['adapter_decode']['status']='FAIL'
            elif change=='source-error':row['records'][0]['response']['code']=0
            elif change=='outer-error':row['error']=dict(code='INTERNAL',message='unrecognized program error')
            elif change=='defect':row['records'][1].update(program_defect=False,error=dict(code='INTERNAL',message='bad adapter'))
            else:
                row['records']+= [dict(stage='direct_media_prefix',status='ACCESS_CONTROL_STOP',host='owned.invalid'),
                    dict(stage='direct_media_prefix',status='PREFIX_ONLY',host='owned.invalid')]
            with self.subTest(change=change),self.assertRaises(ValueError):t24.check_source(row)


if __name__=='__main__':unittest.main()
