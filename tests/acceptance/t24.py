"""T24: fixed-build native endurance, independent review, then raw-evidence replay.

Both modes require the original run.py. `endurance` creates a new immutable
batch; `all` rechecks that batch and its original receipt, never saved PASS flags.
"""
import argparse
import array
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

import t01
import t02
import t12
import t14
import t18
import t20
import t22
import t23
import tv7
from t00 import file_hash, read_json, require, workflow, write_json
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
CURRENT = ROOT / 'evidence/acceptance/T24-endurance-current.json'
IDS = ['U01','U04','R30','R32','R48','R49','REQ-T01','U06','U07','R01','R07','R50','R51','R57','REQ-T24']
OBLIGATIONS = ['U%02d' % i for i in range(1,10)] + ['G0-Core','G1','G2','G3','G4','G5']


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.35', 'T24', IDS
    command = t23.Acceptance.command
    DEPLOYMENT = 'actual packaged App/official Video.js/Rust Host/gateway; owned loopback fixtures; no cluster'
    DEFERRED = [dict(target=target,status='NOT RUN',reason='ADR-005/006; no target machine') for target in t01.DEFERRED] + [
        dict(target='clean installation/initialization/first play/upgrade/uninstall and human sound confirmation',status='NOT RUN',reason='ADR-058; T0.36/A2'),
        dict(target='mobile runtime',status='NOT RUN',reason='U07 initial shared structure only; M-series requires explicit authorization')]

    def save(self):
        result=super().save()
        report=read_json(self.report_path);report['suite']=self.suite
        write_json(self.report_path,report);write_json(self.folder/'report.json',report)
        return result


def artifact(path):
    return dict(path=path.relative_to(ROOT).as_posix(),sha256=file_hash(path),bytes=path.stat().st_size)


def checked_file(row, folder=ROOT / 'evidence'):
    path=t23.inside(row['path'],folder)
    require(path.is_file() and path.stat().st_size==row['bytes'] and file_hash(path)==row['sha256'], 'Raw evidence missing/changed: '+row['path'])
    return path


def owner(run):
    return read_json(ROOT / run.observed['resources']['receipt'])


def check_binding(value, current):
    require(value['source']==current['source'] and value['protected']==current['protected']
        and value['generation']==current['generation'], 'Current source/protected/generation binding differs')


def check_original(receipt, manifest_hash, current):
    check_binding(receipt,current)
    require(receipt['card']=='T0.35' and receipt['id']=='t24-endurance' and receipt['status']=='finished'
        and receipt['exit']==receipt['raw_exit']==0 and receipt['after_source']==current['source'], 'Successful original t24-endurance receipt required')
    require(receipt['argv']==['{python}','-B','tests/acceptance/t24.py','--suite','endurance',
        '--report','evidence/acceptance/T24-endurance.json','--junit','evidence/acceptance/T24-endurance.xml'], 'Unexpected endurance payload')
    log=ROOT / receipt['log']
    require(file_hash(log)==receipt['log_hash'] and log.read_text().count('T24_MANIFEST_SHA256:'+manifest_hash)==1,
        'Original receipt log/manifest binding missing or changed')
    env=receipt['environment']
    require((env['backend'],env['memory_enforcement'],env['memory_mib'],env['jobs'],env['test_threads'])
        ==('process-watchdog','rss-sampled',4096,2,1), 'Actual sampled resource backend differs')


def audio_windows(path, minimum_seconds):
    """Recompute every full PCM second with bounded memory; no saved RMS is trusted."""
    require(path.stat().st_size % 4==0, 'Truncated float32 PCM')
    seconds=path.stat().st_size / 4 / 48000
    require(seconds>=minimum_seconds, 'Actual captured PCM duration is too short')
    rows=[]
    with path.open('rb') as stream:
        while data:=stream.read(48000*4):
            if len(data)<48000*4: break
            pcm=array.array('f');pcm.frombytes(data)
            if sys.byteorder!='little': pcm.byteswap()
            require(all(math.isfinite(value) for value in pcm),'Nonfinite native PCM')
            rms=math.sqrt(sum(value*value for value in pcm)/len(pcm))
            # The owned stimulus is one sine; positive-going crossings count its
            # fundamental without adding a numerical/FFT dependency.
            hz=sum(a<=0<b for a,b in zip(pcm,pcm[1:]))
            rows.append(dict(second=len(rows),rms=rms,hz=hz))
    # Capture startup can precede the first render quantum. All subsequent full
    # seconds must contain the owned 440Hz tone, including the final long interval.
    relevant=rows[1:]
    require(len(relevant)>=int(minimum_seconds)-1 and all(row['rms']>.001 and abs(row['hz']-440)<40 for row in relevant),
        'Captured native application audio is silent or has the wrong frequency')
    return dict(seconds=seconds,windows=rows)


class AudioCapture:
    def __init__(self,run,session):
        self.path=run.folder/'application-audio.f32'
        self.log=run.folder/'application-audio.log'
        self.stream=self.log.open('w')
        timing=run.folder/'application-audio-timing.jsonl'
        self.process=subprocess.Popen([str(run.folder.parent/'t24-audio'),str(session.process.pid),str(self.path),str(timing)],
            stdin=subprocess.PIPE,stdout=self.stream,stderr=subprocess.STDOUT,text=True)
        self.token=workflow.runtime.identity(self.process.pid)
        session.owned[self.process.pid]=self.token
        self.record=dict(pid=self.process.pid,identity=self.token,application_pid=session.process.pid,
            pcm=self.path.relative_to(ROOT).as_posix(),timing=timing.relative_to(ROOT).as_posix(),log=self.log.relative_to(ROOT).as_posix(),exit=None)
        deadline=time.monotonic()+20
        while True:
            require(self.process.poll() is None,'Owned application audio capture failed: '+str(self.log.relative_to(ROOT)))
            text=self.log.read_text()
            if '"started":true' in text.replace(' ',''): break
            require(time.monotonic()<deadline,'Owned application audio capture did not start')
            time.sleep(.05)
        self.record['host_started_monotonic']=time.monotonic()

    def close(self):
        self.record['host_stop_requested_monotonic']=time.monotonic()
        if self.process.poll() is None:
            self.process.stdin.write('\n');self.process.stdin.flush()
        self.process.wait(timeout=20)
        self.process.stdin.close();self.stream.close()
        self.record['exit']=self.process.returncode
        self.record['host_stopped_monotonic']=time.monotonic()
        require(self.process.returncode==0,'Owned audio capture did not stop cleanly')
        return self.record


def check_audio(record,minimum):
    rows=[json.loads(line) for line in (ROOT/record['log']).read_text().splitlines() if line.startswith('{')]
    require(len(rows)==2 and rows[0]['started'] and rows[0]['pid']==record['application_pid']
        and rows[0]['scope']=='owned application only' and rows[0]['format']=='f32le'
        and rows[0]['sample_rate']==48000 and rows[0]['channels']==1
        and rows[1]['stopped'] and not rows[1]['failed'] and record['exit']==0,'Raw application audio lifecycle failed')
    path=ROOT/record['pcm']
    require(rows[1]['frames']*4==path.stat().st_size and rows[1]['last_pts']-rows[1]['first_pts']>=minimum-1,
        'Audio sample count/timestamp coverage differs')
    timing=[json.loads(line) for line in (ROOT/record['timing']).read_text().splitlines()]
    check_audio_timing(timing,rows[1]['frames'])
    require(abs(rows[1]['first_pts']-timing[0]['pts'])<.000001
        and abs(rows[1]['last_pts']-(timing[-1]['pts']+timing[-1]['frames']/48000))<.000001
        and abs(rows[1]['last_pts']-rows[1]['first_pts']-rows[1]['frames']/48000)<.001,
        'PCM first/last timestamps and complete byte duration disagree')
    return audio_windows(path,minimum)


def check_audio_timing(timing,frames):
    require(timing,'Raw PCM buffer timestamps required')
    offset=0;previous=None
    for index,row in enumerate(timing):
        require(row['index']==index and row['offset_frames']==offset and type(row['frames']) is int
            and 0<row['frames']<=48000 and t22.numeric(row['pts']),'PCM buffer offset/count/PTS is incomplete')
        require(abs(row['pts']-(timing[0]['pts']+offset/48000))<.001,'Native PCM timestamp drift/gap differs from byte duration')
        if previous is not None:
            require(abs(row['pts']-(previous['pts']+previous['frames']/48000))<.001,'Native PCM has a timestamp gap/overlap')
        offset+=row['frames'];previous=row
    require(offset==frames,'PCM timeline does not cover every captured byte')
    return dict(buffers=len(timing),frames=frames)


def check_playback(rows,kind,minimum=600):
    require(kind in ['mp4','hls'] and len(rows)>=121,'Both complete 600s first/last plus 5s sample sequences required')
    first,last=rows[0],rows[-1]
    seconds=last['host_monotonic']-first['host_monotonic']
    require(seconds>=minimum and last['value']['sample']['position']-first['value']['sample']['position']>=minimum-10,
        'Full wall/media playback duration missing')
    sid=first['value']['session_id']
    for row in rows:
        value=row['value'];s=value['sample'];audit=value['input_audit']
        require(audit['active'] and audit['count']==0 and audit['events']==[], 'Continuous playback received control/seek/rate/pause/EOS events')
        require(sid and value['session_id']==sid and value['actual_gateway'] and value['playback_rate']==1
            and s['session_bound'] and s['video_count']==1 and s['width']==1920 and s['height']==1080
            and not s['paused'] and s['media_error'] is None and s['frames']>0,'Current gateway/1080p playback failed')
        require(t22.numeric(value['ui_position']) and abs(value['ui_position']-s['position'])<=1
            and value['visibility']=='visible' and value['focused'] and value['native']['focused']
            and value['native']['visible'] and not value['native']['minimized'] and value['errors']==[], 'Visible foreground UI/media progress failed')
        require(s['pixel'][0]>180 and s['pixel'][1]<50 and s['pixel'][2]<50 and s['pixel'][3]==255,'Owned decoded red frame differs')
        if kind=='mp4':
            require(s['audio_state']=='running' and s['audio_rms']>.001 and abs(s['audio_peak_hz']-440)<40,'Decoded MP4 PCM failed')
        else:
            require(value['native']['audio_playing'] and s['diagnostics']['engine']=='native','Native HLS audio/engine inactive')
        t22.check_process(row['process'],value['native'])
        require(row['gpu']['counters'] and row['process']['owned_rss_bytes']>0,'Raw CPU/RSS/GPU sample missing')
    for a,b in zip(rows,rows[1:]):
        require(0<b['host_monotonic']-a['host_monotonic']<15
            and b['value']['sample']['position']>a['value']['sample']['position']
            and b['value']['sample']['frames']>a['value']['sample']['frames'],'Frame/media sequence stalled or sample gap exceeds 15s')
    return dict(format=kind,seconds=seconds,samples=len(rows),media_seconds=last['value']['sample']['position']-first['value']['sample']['position'])


def check_lifecycles(rows):
    require(len(rows)==5 and len({(row['session']['pid'],row['session']['identity']) for row in rows})==5,'Five distinct complete native windows required')
    for index,row in enumerate(rows,1):
        require(row['index']==index and row['kind']==('hls' if index%2==0 else 'mp4'),'Window format/order differs')
        session=row['session'];t22.check_cleanup(session)
        require(row['identity']['native']['pid']==session['pid'] and row['identity']['origin']=='tauri://localhost'
            and row['identity']['native']['webkit_pids']['renderer']>0,'Real native window/renderer identity missing')
        t12.validate_stop(row['stop'])
        require(row['disposed']['audio_closed'] and row['disposed']['videos']==0 and row['disposed']['errors']==[], 'Formal player/audio unmount failed')
        require(row['host']['closed'] and row['host']['active_requests']==0 and row['host']['opens']>=1
            and row['transport']['active_requests']==0,'Host/fixture requests or sessions leaked')
        sample=row['played']['sample']
        require(sample['session_bound'] and sample['video_count']==1 and sample['frames']>0
            and sample['width']==1920 and sample['height']==1080 and not sample['paused'] and sample['media_error'] is None,'Window did not actually decode current media')
        require(sample['pixel'][0]>180 and sample['pixel'][1]<50 and sample['pixel'][2]<50 and sample['pixel'][3]==255,'Window decoded the wrong frame')
        if row['kind']=='mp4':
            require(sample['audio_state']=='running' and sample['audio_rms']>.001 and abs(sample['audio_peak_hz']-440)<40,'Window did not decode MP4 audio')
        else:
            t12.validate_cycle(row['switch'],51)
            require(row['played']['native']['audio_playing'] and sample['diagnostics']['engine']=='native','Window native HLS audio inactive')
    return dict(windows=5,formats=['mp4','hls'],normal_cleanup=True)


def collect_window(run,build,media,index,kind,long):
    base=run.folder;run.folder=base/('window-%d'%index);run.folder.mkdir()
    session=bridge=capture=transport=None
    row=dict(index=index,kind=kind)
    try:
        # Host DTO/capabilities are transient transport, as in the original T12.
        transport=tempfile.TemporaryDirectory(prefix='bridge-',dir=run.folder)
        live=Path(transport.name)
        with t01.environment({'T12_MIN_OPENS':'1'}):
            bridge=t12.Bridge(run,live,media);bridge.minimum_opens=1
        ready=bridge.wait(live/'ready.json',timeout=180)
        with t01.Fixture(t01.MEDIA,invoke=bridge.invoke) as fixture:
            session=t01.NativeSession(run,fixture,ROOT/build['automation']['path']/'Contents/MacOS/yingxiang-video',index)
            session.wait(0,30);row['identity']=session.command('identity')
            run.command([str(base/'tv1-native'),'click',str(session.process.pid),'600','200'])
            session.command('t12',action='start',payload=ready,timeout=60)
            session.record['commands'][-1]['args']=dict(action='start',payload='Host DTO; capabilities redacted')
            if kind=='hls':
                row['switch']=session.command('t12',action='switch',payload=51,timeout=40);t12.validate_cycle(row['switch'],51)
            row['played']=session.command('t12',action='t24-ready',payload=kind)
            if long:
                # Keep native HLS PCM outside WebKit's unsupported WebAudio tap.
                capture=AudioCapture(run,session)
                time.sleep(2.5)
                audio_windows(capture.path,2)
                samples=run.folder/'playback.jsonl';start=time.monotonic();rows=[]
                while True:
                    value=session.command('t12',action='t22-observe')
                    raw=dict(utc=t22.utc(),host_monotonic=time.monotonic(),value=value,process=t12.process_sample(session,bridge),gpu=t22.gpu_probe())
                    rows.append(raw);t22.append_sample(samples,raw)
                    if raw['host_monotonic']-rows[0]['host_monotonic']>=600: break
                    time.sleep(max(0,start+len(rows)*5-time.monotonic()))
                row['audio']=capture.close();capture=None
                require(row['audio']['host_started_monotonic']<rows[0]['host_monotonic']
                    and row['audio']['host_stop_requested_monotonic']>rows[-1]['host_monotonic'],'PCM capture did not cover the complete playback interval')
                row['playback']=samples.relative_to(ROOT).as_posix()
                row['long_result']=check_playback(rows,kind)
                row['audio_result']=check_audio(row['audio'],600)
            row['stop']=session.command('t12',action='stop');t12.validate_stop(row['stop'])
            row['disposed']=session.command('t12',action='dispose')
            session.close();row['session']=session.record;row['transport']=fixture.summary()
        bridge.finish();row['host']=dict(run.observed['host'])
        write_json(run.folder/'lifecycle.json',row)
        return row
    finally:
        try:
            if capture is not None: capture.close()
        finally:
            try:
                if session is not None and not session.closed: session.close(graceful=False)
            finally:
                if bridge is not None: bridge.close()
                if transport is not None: transport.cleanup()
                run.folder=base


def fresh_public(run,build):
    base=run.folder;run.folder=base/'public';run.folder.mkdir()
    bridge=session=transport=None
    try:
        transport=tempfile.TemporaryDirectory(prefix='bridge-',dir=run.folder);live=Path(transport.name)
        bridge=t02.Bridge(run,live);ready=bridge.wait(live/'ready.json',timeout=900)
        write_json(run.folder/'protocol.json',ready.get('protocol'))
        write_json(run.folder/'sources.json',dict(sources=ready['sources'],selected=ready.get('source'),status=ready['status']))
        if ready['status']!='READY':
            bridge.finish();require(False,'No real public source/media chain available; raw source states retained')
        with t01.Fixture(t01.MEDIA) as fixture:
            session=t01.NativeSession(run,fixture,ROOT/build['automation']['path']/'Contents/MacOS/yingxiang-video',6)
            session.wait(0,30);session.command('identity')
            value=session.command('t02-public',timeout=120,view=ready['view'],kind=ready['kind'])
            session.record['commands'][-1]['args']=dict(view='Rust gateway view; capability redacted',kind=ready['kind'])
            t02.validate_short_play(value);session.command('stop');session.close()
        bridge.finish()
        result=dict(sources=ready['sources'],selected=ready['source'],protocol=ready['protocol'],short_play=value,host=run.observed['host'],session=session.record)
        result['supported']=t02.validate_sources(result['sources'],result['selected'])
        write_json(run.folder/'public.json',result)
        return result
    finally:
        try:
            if session is not None and not session.closed: session.close(graceful=False)
        finally:
            if bridge is not None: bridge.close()
            if transport is not None: transport.cleanup()
            run.folder=base


def check_public(value):
    for row in value['sources']:
        check_source(row)
    supported=t02.validate_sources(value['sources'],value['selected'])
    require(value['supported']==supported,'Public support list differs from recomputation')
    declared=read_json(ROOT/'docs/compatibility-report.json')
    require({row['name'] for row in value['sources']}=={row['name'] for row in declared['sources'] if row.get('probe')}, 'All approved source candidates must be observed')
    t02.validate_short_play(value['short_play']);t22.check_cleanup(value['session'])
    require(value['host']['ready'] and value['host']['closed'],'Public native Host failed cleanup')
    protocol=value['protocol']
    require(all(protocol[key] is True for key in ['list','details','resolve']) and type(protocol['complete_groups']) is int
        and protocol['complete_groups']>0 and type(protocol['episodes']) is int and protocol['episodes']>0,'Real complete public protocol chain missing')
    return dict(supported=supported,availability=[dict(name=row['name'],compatibility=row['compatibility'],media_samples=row['media_samples']) for row in value['sources']])


def program_defect(error):
    states=['Public HTTP','HTTP redirect','HTTPS redirect','unsafe_target','closed','not JSON','Public capture','detail is missing','changed or is missing']
    return not (error['code']=='UPSTREAM_TIMEOUT' or 'no links' in error['message'] or any(state in error['message'] for state in states))


def check_source(source):
    """Independently replay the Rust source-state rules from the raw projections."""
    records=source['records'];require(records,'Public source raw records missing')
    media=[row for row in records if row['stage']=='direct_media_prefix']
    require(len(media)<=8,'Source sampled beyond its approved bound')
    for index,row in enumerate(media):
        previous=[item for item in media[:index] if item.get('host')==row.get('host') and item['status']!='PREFIX_ONLY']
        require(not any(item['status']=='ACCESS_CONTROL_STOP' for item in previous) and len(previous)<2,'Requests continued to an exhausted/refusing media host')
    for row in records:
        if 'program_defect' in row:
            require(row['program_defect'] is program_defect(row['error']),'Source failure attribution differs from raw error')
        if 'response' in row and isinstance(row['response'],dict) and 'code' in row['response']:
            code=row['response']['code']
            require(row.get('source_error',False) is not (type(code) is int and code==1),'Source API error attribution differs')
    api=[row for row in records if row['stage'] not in ['adapter_details','direct_media_prefix']]
    if source.get('error') and program_defect(source['error']) or any(row.get('program_defect') is True for row in records):
        verdict='PROGRAM_INCOMPATIBLE'
    elif any(row['stage']=='adapter_details' and row['status']=='NO_CONTENT' for row in records):verdict='SOURCE_NO_CONTENT'
    elif any(row.get('adapter_decode',{}).get('status')=='FAIL' and row.get('source_error') is not True for row in api):verdict='PROGRAM_INCOMPATIBLE'
    elif any(row['status']!='JSON_CAPTURED' or row.get('source_error') is True for row in api):verdict='SOURCE_UNAVAILABLE'
    elif any(row['stage']=='adapter_details' and row['status']=='PASS' for row in records):verdict='COMPATIBLE'
    else:verdict='PROGRAM_INCOMPATIBLE'
    passed=sum(row['status']=='PREFIX_ONLY' for row in media)
    summary=dict(attempted=len(media),passed=passed,
        failed_hosts=sorted({row['host'] for row in media if row['status']!='PREFIX_ONLY' and isinstance(row.get('host'),str)}),
        verdict='MEDIA_AVAILABLE' if passed else 'NO_SAMPLE_AVAILABLE' if media else 'NOT RUN')
    require(source['compatibility']==verdict and source['supported'] is (verdict=='COMPATIBLE') and source['media_samples']==summary,
        'Source support/media summary differs from complete raw-record recomputation')
    require(verdict!='PROGRAM_INCOMPATIBLE','Accessible source exposes a program defect: '+source['name'])
    return dict(name=source['name'],compatibility=verdict,media=summary)


def package(run):
    # Existing release builder preserves a separate signed production installer.
    run.command(['{python}','-B','packaging/release.py','--build'],timeout=6600)
    pointer=read_json(ROOT/'evidence/releases/T23-current.json')
    manifest_path=ROOT/pointer['manifest'];require(file_hash(manifest_path)==pointer['sha256'],'Release manifest changed')
    manifest=read_json(manifest_path)
    require(manifest['source']==run.source and manifest['protected']==workflow.protected_hash()
        and manifest['scope']=='personal use only; not public distribution','Current personal release binding differs')
    t23.payload(manifest,manifest_path);t23.archives(run,manifest,manifest_path);t23.signing(run,manifest,manifest_path)
    return artifact(manifest_path)


def product_scenario(run,build_dir,name,function,key):
    base=run.folder;run.folder=base/name;run.folder.mkdir()
    try:
        function(run,build_folder=build_dir)
        value=dict(value=run.observed[key],host=run.observed['host'],sessions=run.observed['native_sessions'][-1:])
        write_json(run.folder/'behavior.json',value)
        return (run.folder/'behavior.json').relative_to(ROOT).as_posix()
    finally:run.folder=base


def check_product(value,key):
    require(value['host']['active_requests']==0 and value['host']['restarts']==1,'Product Host did not restart/clean up')
    require(len(value['sessions'])==1,'One real product scenario window required')
    t22.check_cleanup(value['sessions'][0])
    if key=='public_product':t20.validate_public(value['value'])
    else:
        e=value['value'];t20.validate_history(e)
        require(e['restart']['after_records']==[e['restart']['before_record']],'Actual complete history row changed across Host/SQLite restart')
        closing=e['closing'];records=closing['history_records'];settings=closing['settings_snapshot']
        require(len(records)==1 and records[0]['video_id']==closing['video'] and records[0]['position_ms']>0
            and abs(settings['volume']-.37)<.001 and settings['auto_next'] is True
            and settings['window_bounds']==e['bounds']['saved'],'Real window close did not retain queued settings/current history')
    return dict(scenario=key,real_window=True)


def check_package(row,source):
    path=checked_file(row,ROOT/'evidence/releases');value=read_json(path)
    require(value['source']==source and value['protected']==workflow.protected_hash()
        and value['scope']=='personal use only; not public distribution','Package source/scope differs')
    t23.payload(value,path)
    for item in value['archives']:
        checked_file(item,ROOT/'evidence/releases')
    require(not value['signing']['developer_id_signed'] and not value['signing']['notarized'], 'Personal release signing state overclaimed')
    for command in value['commands']:
        require(file_hash(ROOT/command['log'])==command['sha256'],'Release build log changed')
    return dict(manifest=row,signing=value['signing'],lifecycle='NOT RUN: ADR-058; T0.36/A2')


def produce(run):
    original=owner(run)
    commit=workflow.git('rev-parse','HEAD')
    require(workflow.snapshot(commit)==run.source,'Fixed WIP commit must contain all tested source before endurance')
    run.command(['clang','-fobjc-arc','-fblocks','tests/fixtures/t24_audio.m','-framework','Cocoa','-framework','ScreenCaptureKit',
        '-framework','CoreMedia','-framework','AudioToolbox','-o',run.folder/'t24-audio'])
    run.command(['clang','-fobjc-arc','tests/fixtures/tv1_native.m','-framework','Cocoa','-framework','ApplicationServices','-o',run.folder/'tv1-native'])
    release=package(run)
    build_dir=run.folder/'packaged';build_dir.mkdir()
    with t22.packaged_paths(build_dir):
        build=t01.build(run);t01.check_build(run.source)
    product={key:product_scenario(run,build_dir,name,function,key) for name,function,key in [
        ('public-product',t20.public,'public_product'),('history-product',t20.controlled_history,'history')]}
    for key,name in product.items():check_product(read_json(ROOT/name),key)
    media=run.folder/'media';media.mkdir()
    t22.media(run,media)
    # t22 keeps the old short HLS collection; this batch needs both full formats.
    run.command(['ffmpeg','-nostdin','-hide_banner','-loglevel','error','-y','-i',media/'a.mp4','-map','0','-c','copy','-f','hls',
        '-hls_time','5','-hls_list_size','0','-hls_segment_filename',media/'long-%d.ts',media/'index.m3u8'],timeout=120)
    check_media(run,media)
    public=fresh_public(run,build);check_public(public)
    windows=[]
    for index in range(1,6):
        windows.append(collect_window(run,build,media,index,'hls' if index%2==0 else 'mp4',index<=2))
    check_lifecycles(windows)
    require(workflow.snapshot()==run.source,'Source changed during native batch')
    raw=[artifact(path) for path in sorted(run.folder.rglob('*')) if path.is_file()
        and not any(part.endswith('.app') for part in path.parts)]
    manifest=dict(schema_version=1,card='T0.35',commit=commit,source=run.source,protected=original['protected'],generation=original['generation'],
        receipt=run.observed['resources']['receipt'],build=artifact(build_dir/'build.json'),release=release,
        windows=[artifact(ROOT/row['session']['log']).get('path').rsplit('/',1)[0]+'/lifecycle.json' for row in windows],
        public=(run.folder/'public/public.json').relative_to(ROOT).as_posix(),product=product,files=raw,commands=run.commands,
        environment=run.observed.get('reference_machine'),utc=t22.utc())
    path=run.folder/'manifest.json';write_json(path,manifest)
    pointer=artifact(path);write_json(CURRENT,pointer)
    print('T24_MANIFEST_SHA256:'+pointer['sha256'],flush=True)
    run.observed['batch']=pointer
    return dict(manifest=pointer,windows=5,long_formats=2)


def check_media(run,folder):
    probes={}
    for kind,name in [('mp4','a.mp4'),('hls','index.m3u8')]:
        probe=json.loads(run.command(['ffprobe','-v','error','-show_streams','-show_format','-of','json',folder/name],timeout=60))
        video=next(row for row in probe['streams'] if row['codec_type']=='video')
        audio=next(row for row in probe['streams'] if row['codec_type']=='audio')
        require((video['codec_name'],video['width'],video['height'],video['r_frame_rate'])==('h264',1920,1080,'30/1')
            and audio['codec_name']=='aac' and float(probe['format']['duration'])>=620,'Both actual 1080p30 H264/AAC long formats required')
        probes[kind]=probe
    write_json(folder/'probes.json',probes)


def load_batch(run):
    pointer=read_json(CURRENT);path=checked_file(pointer,ROOT/'evidence/acceptance');manifest=read_json(path)
    check_binding(manifest,owner(run))
    require(manifest['card']=='T0.35' and workflow.snapshot(manifest['commit'])==run.source,'Fixed tested commit source differs')
    receipt=read_json(ROOT/manifest['receipt']);check_original(receipt,pointer['sha256'],owner(run))
    names=[row['path'] for row in manifest['files']]
    require(len(names)==len(set(names)) and names,'Nonempty unique raw evidence inventory required')
    for row in manifest['files']: checked_file(row,path.parent)
    require(len(manifest['windows'])==5 and set(manifest['windows'])<=set(names) and manifest['public'] in names
        and manifest['build']['path'] in names,'Raw lifecycle/public/build files missing from manifest')
    for command in manifest['commands']:
        optional=command['argv'][:2] in (['/usr/sbin/spctl','--assess'],['/usr/bin/xcrun','stapler'])
        require((command['exit']==0 or optional) and command['log'] in names,'Failed or missing batch command log')
    with t22.packaged_paths((ROOT/manifest['build']['path']).parent):
        checked_file(manifest['build']);t01.check_build(run.source)
    windows=[read_json(ROOT/name) for name in manifest['windows']]
    for row in windows[:2]:
        require(row['playback'] in names and row['audio']['pcm'] in names and row['audio']['log'] in names and row['audio']['timing'] in names,'Raw long-play/PCM files missing')
        samples=[json.loads(line) for line in (ROOT/row['playback']).read_text().splitlines()]
        require(row['audio']['host_started_monotonic']<samples[0]['host_monotonic']
            and row['audio']['host_stop_requested_monotonic']>samples[-1]['host_monotonic'],'Audio capture does not span the raw playback interval')
        check_playback(samples,row['kind']);check_audio(row['audio'],600)
    check_lifecycles(windows);check_public(read_json(ROOT/manifest['public']));check_package(manifest['release'],run.source)
    require(set(manifest['product'])=={'public_product','history'},'Both current product/history UI scenarios required')
    for key,name in manifest['product'].items():
        require(name in names,'Raw product UI behavior missing')
        check_product(read_json(ROOT/name),key)
    run.observed['batch']=pointer
    return manifest,path


def check_review(text,manifest,path,current,root=ROOT):
    require(text.rstrip().splitlines()[-1]=='ACCEPTANCE-VERDICT: CLEAR','Latest independent review is missing CLEAR or remains BLOCKING')
    matches=re.findall(r'```json\s*\n(.*?)\n```',text,re.S)
    require(len(matches)==1,'One structured independent review binding required')
    review=json.loads(matches[0]);check_binding(review,current)
    require(review['commit']==manifest['commit'] and review['manifest']==artifact(path)
        and review['unresolved_blockers']==[] and review['personal_use_only'] is True
        and review['public_distribution_authorized'] is False,'Independent review build/evidence/scope binding differs')
    require(set(review['obligations'])==set(OBLIGATIONS),'Every revised U01..U09 and G0-Core/G1..G5 must be independently reviewed')
    for key,row in review['obligations'].items():
        require(row['result']=='PASS' and row['evidence'] and row['reason'],'Independent obligation lacks concrete reviewed evidence: '+key)
    required={item['path']:item['sha256'] for item in manifest['files']}
    required[path.relative_to(ROOT).as_posix()]=file_hash(path)
    required[manifest['receipt']]=file_hash(ROOT/manifest['receipt'])
    receipt=read_json(ROOT/manifest['receipt']);required[receipt['log']]=file_hash(ROOT/receipt['log'])
    required[manifest['release']['path']]=manifest['release']['sha256']
    require(review['files']==required,'Independent report must record every raw file, original receipt/log, manifest and release SHA')
    return review


def independent(run,manifest,path):
    latest=latest_review(ROOT/'docs/reviews/T24')
    review=check_review(latest.read_text(),manifest,path,owner(run))
    # Reports are durable reviewed documentation, not uncommitted author flags.
    name=latest.relative_to(ROOT).as_posix()
    require(workflow.git('show','HEAD:'+name)==latest.read_text().rstrip('\n'),'Independent original report must be committed verbatim')
    workflow.git('merge-base','--is-ancestor',manifest['commit'],'HEAD')
    run.observed['independent_review']=dict(path=name,sha256=file_hash(latest),commit=review['commit'])
    return run.observed['independent_review']


def latest_review(folder):
    rounds=[(int(match[1]),item) for item in folder.glob('round-*.md') if (match:=re.fullmatch(r'round-(\d+)\.md',item.name))]
    require(rounds,'Independent T24 fixed-build review missing')
    return max(rounds)[1]


def regressions(run):
    output=run.command(['{python}','-B','-m','unittest','discover','-s','tests/acceptance','-p','test_t24.py','-v'])
    count=re.search(r'Ran (\d+) tests?',output);require(count and int(count[1])>=10,'Nonzero independent T24 corruption tests required')
    return dict(tests=int(count[1]))


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--suite',choices=['endurance','all'],required=True)
    parser.add_argument('--report',type=Path,required=True);parser.add_argument('--junit',type=Path,required=True);args=parser.parse_args()
    stem='T24-'+args.suite
    require(args.report.as_posix()=='evidence/acceptance/'+stem+'.json' and args.junit.as_posix()=='evidence/acceptance/'+stem+'.xml','Exact T24 output paths required')
    run=Acceptance(args.report,args.junit);run.suite=args.suite
    if run.case('resource-ownership',IDS,'current source/card original process-watchdog sampled 4096MiB jobs2/thread1',lambda:resources(run,card='T0.35')):
        if args.suite=='endurance':
            run.case('reference-machine',IDS,'actual macOS14+ arm64/WebKit/toolchain/locks',lambda:t22.environment(run))
            run.case('current-native-endurance',IDS,'two 600s formats, five full windows, PCM/frame/UI/process/raw evidence and current personal package',lambda:produce(run))
        else:
            regressions_ok=run.case('independent-negative-oracles',IDS,'missing/stale/corrupt/partial evidence fails',lambda:regressions(run))
            batch=[]
            if regressions_ok and run.case('raw-endurance-recomputed',IDS,'full recomputation of current batch samples/audio/lifecycles/package/public evidence',lambda:batch.append(load_batch(run)) or dict(files=len(batch[0][0]['files']))):
                run.case('independent-fixed-build-review',IDS,'latest numeric committed read-only review; every U/G and original raw hash checked',lambda:independent(run,*batch[0]))
                run.case('current-source-protocol-and-source-regressions',IDS,'real public response projections, DNS/peer/cap isolation and all sources recomputed',lambda:t02.native(run))
                run.case('current-source-data-and-history',IDS,'real SQLite restart/migration/write barriers/settings and exit regression',lambda:t18.native(run))
                run.case('current-permissions',IDS,'exact current C17 permissions and CSP',tv7.security)
    return run.save()


if __name__=='__main__':
    raise SystemExit(main())
