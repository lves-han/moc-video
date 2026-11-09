"""TV6: production player composition in packaged WKWebView, then original 50-switch ownership gates."""
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
from t00 import require, file_hash, write_json
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
BEHAVIORS = ['loading-backdrop','immersive-autohide','custom-skin-controls','tray-episodes-streams','next-episode-card','escape-levels','error-overlay','switch-recycle-50','visual-spec']
IDS = ['TV-PLAYER', *[f'TV-PLAYER/{name}' for name in BEHAVIORS], 'REQ-TV6']


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.33', 'TV6', IDS
    DEPLOYMENT = 'actual packaged Tauri WKWebView and real Host/MP4/HLS; original sampled process-watchdog budget and cleanup'
    DEFERRED = [dict(target=target, status='NOT RUN', reason='ADR-005/006; local macOS arm64 required') for target in t01.DEFERRED]


def case(run, name, expected, function):
    return run.case(name, ['TV-PLAYER', f'TV-PLAYER/{name}', 'REQ-TV6'], expected, function)


def near(value, expected, message, tolerance=.15):
    tv1.near(value, expected, message, tolerance)


def validate_loading(row):
    require(row['phase'] == 'resolving' and row['title'] == 'TV6 自有测试影片' and row['episode'] == '第1集', 'Resolver loading identity missing')
    require(row['backdrop']['filter'] == 'blur(40px) brightness(0.35)', 'Loading backdrop blur/brightness differs')
    require(row['titleStyle']['fontSize'] == '28px' and row['titleStyle']['fontWeight'] == '700' and row['episodeStyle']['fontSize'] == '15px', 'Loading typography differs')
    near(row['spinner']['width'], 28, 'Loading spinner width'); near(row['spinner']['height'], 28, 'Loading spinner height')
    return dict(phase=row['phase'], title=row['title'], decoded=True)


def validate_styles(row):
    width, height, s = row['width'], row['height'], row['styles']
    require(width in [960,1200,1512] and row['videoCount'] == row['controlsCount'] == 1 and not row['legacy'] and row['errors'] == [], 'Immersive window/control ownership or console differs')
    require(s['.playback-view']['backgroundColor'] == 'rgb(0, 0, 0)' and s['video']['objectFit'] == 'contain', 'Black contained video differs')
    for key in ['.playback-view','video']:
        near(s[key]['rect']['x'],0,'Full-window left'); near(s[key]['rect']['y'],0,'Full-window top')
        near(s[key]['rect']['width'],width,'Full-window width'); near(s[key]['rect']['height'],height,'Full-window height')
    near(s['.playback-shade']['rect']['height'],height*.3,'Gradient height')
    near(s['.playback-header']['rect']['x'],24,'Header left'); near(s['.playback-header']['rect']['y'],52,'Header traffic-light clearance')
    near(s['.playback-back']['rect']['width'],44,'Back hit area'); near(s['.playback-back']['rect']['height'],44,'Back hit area')
    require(s['.playback-back']['borderRadius'] == '50%', 'Back circle differs')
    for selector,size,weight in [('.playback-header h2','17px','600'),('.playback-header p','13px','400')]:
        require(s[selector]['fontSize'] == size and s[selector]['fontWeight'] == weight, f'{selector} typography differs')
    require(s['.playback-header p']['color'] == 'rgba(235, 235, 245, 0.62)', 'Header secondary color differs')
    bar = s['.playback-bar']; r = bar['rect']
    near(r['x'],24,'Bar left'); near(width-r['x']-r['width'],24,'Bar right'); near(height-r['y']-r['height'],24,'Bar bottom')
    require(bar['borderRadius'] == '20px' and [bar[key] for key in ['paddingTop','paddingRight','paddingBottom','paddingLeft']] == ['12px','20px','12px','20px'], 'Control bar radius/padding differs')
    require(bar['backgroundColor'] == 'rgba(30, 30, 34, 0.62)' and 'blur(30px)' in bar['webkitBackdropFilter'] and 'saturate(1.6)' in bar['webkitBackdropFilter'], 'Glass surface/filter differs')
    near(s['.playback-track']['rect']['height'],4,'Track thickness')
    require(s['.playback-track']['backgroundColor'] == 'rgba(255, 255, 255, 0.3)' and s['media-slider-buffer']['backgroundColor'] == 'rgba(255, 255, 255, 0.5)' and s['media-slider-fill']['backgroundColor'] == 'rgb(245, 245, 247)', 'Progress colors differ')
    near(s['media-slider-thumb']['rect']['width'],14,'Seek thumb'); near(s['media-slider-thumb']['rect']['height'],14,'Seek thumb')
    require(s['.playback-preview']['fontSize'] == '12px' and s['.playback-preview']['borderRadius'] == '999px' and s['media-time']['fontSize'] == '13px' and s['media-time']['fontVariantNumeric'] == 'tabular-nums', 'Preview/time typography differs')
    near(s['.playback-button']['rect']['width'],44,'Control hit width'); near(s['.playback-button']['rect']['height'],44,'Control hit height'); near(s['media-play-button media-icon']['rect']['width'],28,'Play icon size')
    require(s['.playback-tray']['borderTopLeftRadius'] == s['.playback-tray']['borderTopRightRadius'] == '20px' and '0.3s' in s['.playback-tray']['transitionDuration'], 'Tray radius/transition differs')
    near(s['.playback-tray']['rect']['height'],min(height*.46,420),'Tray height')
    require(s['.playback-now']['rect']['width'] == s['.playback-now']['rect']['height'] == 1, 'Status identity must be visually hidden')
    near(s['.playback-buffering .playback-spinner']['width'],40,'Buffer spinner width'); near(s['.playback-buffering .playback-spinner']['height'],40,'Buffer spinner height')
    require(row['bufferDelay']==300,'Official buffer indicator delay must be 300ms')
    require(row['cue']['fontSize'] == ('34px' if width >= 1440 else '28px') and row['cue']['color'] == 'rgb(255, 255, 255)' and '2px' in row['cue']['textShadow'], 'Native subtitle typography/outline differs')
    return dict(width=width,height=height,official_controls=1)


def validate_controls(row, keys):
    require(abs(row['forward']-22)<.2 and abs(row['backward']-12)<.2 and row['shown'] and row['off'] and row['speeds'] == [.5,1,1.25,1.5,2] and row['volume'] == .5 and row['registered'] and row['owner'] == 1 and not row['errors'], 'Official controls did not reach real media')
    require(abs(keys['right']['position']-keys['before']['position']-10)<.25 and abs(keys['left']['position']-keys['before']['position'])<.25, 'Native arrows must seek 10s')
    require(abs(keys['up']['volume']-keys['before']['volume']-.05)<.001 and abs(keys['down']['volume']-keys['before']['volume'])<.001, 'Native volume keys differ')
    require(keys['mute']['muted'] and not keys['unmute']['muted'] and not keys['play']['paused'] and keys['pause']['paused'] and keys['fullscreen']['fullscreen'] and not keys['windowed']['fullscreen'], 'Native Space/M/F bindings differ')
    return dict(keys=list(keys),speeds=row['speeds'])


def validate_next(row):
    require(not row['before']['next'] and row['visible']['next'], 'Next card must appear only within final 20s')
    require(row['offEos']['trusted'] and row['off']['phase']=='ended' and row['off']['countdown'] is None and row['off']['matches']==row['offMatches'], 'Auto-next off must stay after trusted EOS')
    require(row['initialCountdown']['countdown']=='5' and row['cancelledEos']['trusted'] and row['cancelled']['phase']=='ended' and row['cancelled']['countdown'] is None and row['cancelled']['video']==row['initialCountdown']['video'], 'Cancel must prevent pending auto-next')
    require(row['autoEos']['trusted'] and row['advanced']['video']!=row['initialCountdown']['video'] and row['advanced']['elapsed']>=4900 and row['advanced']['phase']=='playing', 'Only trusted EOS followed by 5s may advance')
    require(row['stale']['matches']==row['staleCount'] and row['stale']['phase']=='playing' and row['stale']['countdown'] is None and row['completedSaves']>0, 'Stale countdown or completion history differs')
    near(row['card']['rect']['width'],300,'Next card width'); require(row['card']['borderRadius']=='16px','Next card radius')
    near(row['image']['rect']['height']/row['image']['rect']['width'],9/16,'Next still ratio',.002)
    require(row['kicker']['fontSize']=='12px' and row['kicker']['fontWeight']=='600' and row['title']['fontSize']=='14px' and row['title']['fontWeight']=='500','Next card typography')
    return dict(autoplay_ms=row['advanced']['elapsed'],cancelled=True,stale_epoch_cancelled=True)


def long_media(run, folder):
    t12.owned_media(run,folder,seconds=40)
    run.observed['tv6_media']=run.observed['media']


def color_probe():
    """Diagnose the existing untagged generated YUV input without changing any verdict."""
    with tempfile.TemporaryDirectory(prefix='tv6-color-',dir=ROOT/'evidence') as temporary:
        path=Path(temporary)/'blue.mp4'
        subprocess.run(['ffmpeg','-nostdin','-hide_banner','-loglevel','error','-y','-f','lavfi','-i','color=c=blue:size=1920x1080:rate=30','-t','0.1','-c:v','libx264','-preset','ultrafast','-pix_fmt','yuv420p',str(path)],check=True)
        metadata=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_entries','stream=color_space,color_transfer,color_primaries','-of','json',str(path)]))
        colors={}
        for matrix in ['bt601','bt709']:
            pixels=subprocess.check_output(['ffmpeg','-nostdin','-hide_banner','-loglevel','error','-i',str(path),'-vf',f'scale=1:1:in_color_matrix={matrix},format=rgb24','-frames:v','1','-f','rawvideo','pipe:1'])
            colors[matrix]=list(pixels)
        print(json.dumps(dict(metadata=metadata,decoded_rgb=colors)))


def diagnose():
    """One real window for raw readback diagnostics; never an A1 verdict."""
    run=Acceptance(ROOT/'evidence/acceptance/TV6-diagnostic.json',ROOT/'evidence/acceptance/TV6-diagnostic.xml')
    previous=t01.DATA,t01.BUILD
    session=bridge=None
    try:
        resources(run,card='T0.33')
        t01.DATA=run.folder/'packaged';t01.DATA.mkdir();t01.BUILD=t01.DATA/'build.json';t01.build(run)
        os.environ['T12_MIN_OPENS']='1'
        with tempfile.TemporaryDirectory(prefix='tv6-diagnostic-',dir=run.folder) as temporary:
            folder=Path(temporary);media=folder/'media';media.mkdir();t12.owned_media(run,media)
            bridge=t12.Bridge(run,folder,media);bridge.minimum_opens=1;ready=bridge.wait(folder/'ready.json',timeout=180)
            with t01.Fixture(t01.MEDIA,invoke=bridge.invoke) as fixture:
                executable=ROOT/t01.check_build(run.source)['automation']['path']/'Contents/MacOS/yingxiang-video'
                session=t01.NativeSession(run,fixture,executable,1)
                session.wait(0,30);session.command('identity')
                session.command('t12',action='start',payload=ready,timeout=60)
                session.record['commands'][-1]['args']=dict(action='start',payload='real Host DTO; capabilities transient')
                run.observed['colors']=session.command('t12',action='color-diagnostic',timeout=45)
                print('TV6_COLOR:'+json.dumps(run.observed['colors']))
                session.command('t12',action='stop');session.command('t12',action='dispose');session.close()
            bridge.finish()
    finally:
        os.environ.pop('T12_MIN_OPENS',None)
        try:
            if session is not None and not session.closed:session.close(graceful=False)
        finally:
            if bridge is not None:bridge.close()
            t01.DATA,t01.BUILD=previous
            write_json(run.folder/'diagnostic.json',dict(source=run.source,observed=run.observed,commands=run.commands))
    print('Diagnostic evidence: '+str(run.folder/'diagnostic.json'))


def interactive(run):
    previous_folder=run.folder
    run.folder=previous_folder/'immersive'; run.folder.mkdir()
    os.environ['T12_MIN_OPENS']='1'
    session=bridge=None
    try:
        receipt=t01.check_build(run.source)
        executable=ROOT/receipt['automation']['path']/'Contents/MacOS/yingxiang-video'
        run.command(['clang','-fobjc-arc','-framework','Cocoa','-framework','ApplicationServices','tests/fixtures/tv1_native.m','-o',str(run.folder/'tv1-native')])
        with tempfile.TemporaryDirectory(prefix='tv6-live-',dir=run.folder) as temporary:
            folder=Path(temporary); media=folder/'media'; media.mkdir(); long_media(run,media)
            bridge=t12.Bridge(run,folder,media); bridge.minimum_opens=1
            ready=bridge.wait(folder/'ready.json',timeout=180)
            with t01.Fixture(t01.MEDIA,invoke=bridge.invoke) as fixture:
                session=t01.NativeSession(run,fixture,executable,1)
                session.wait(0,30)
                session.observe();tv1.native(run,'click',session.process.pid,600,200)
                session.command('identity')
                def action(name,timeout=45,**kwargs):
                    value=session.command('tv6',action=name,timeout=timeout,**kwargs)
                    if name=='start': session.record['commands'][-1]['args']=dict(action='start',payload='real Host DTO/context; URLs transient and redacted')
                    run.observed.setdefault('immersive',{})[name]=value
                    return value
                loaded=action('start',payload=ready)
                long=action('long-loading')
                missing=action('loading-no-image')
                case(run,'loading-backdrop','real pending resolver, decoded blurred background, identified title including missing image, >10s message',lambda: (validate_loading(loaded),require(long['elapsed']>=10000 and long['visible'] and long['text']=='正在查找可用线路' and missing['title']==loaded['title'] and missing['images']==0,'Long resolver hint/missing-image title missing')))
                action('release')
                styles=[]; screenshots=[]; hovers=[]
                for width in [960,1200,1512]:
                    session.command('layout',value=width)
                    tv1.native(run,'hover',session.process.pid,width/2,160)
                    row=action('styles')
                    styles.append(row); validate_styles(row)
                    screenshots.append(tv1.capture(run,session.process.pid,width,row['native']['scale_factor']))
                    rect=action('rect',payload='.playback-seek')
                    tv1.native(run,'hover',session.process.pid,rect['x']+rect['width']*.4,rect['y']+rect['height']/2)
                    hover=action('hover-style'); hovers.append(hover)
                    near(hover['track']['rect']['height'],8,'Hovered track'); require(hover['thumb']['opacity']=='1' and hover['preview']['opacity']=='1','Hover thumb/preview missing')
                run.observed['styles']=styles; run.observed['screenshots']=screenshots; run.observed['hover_styles']=hovers
                case(run,'visual-spec','exact three-width computed styles, native subtitle cue, real hover and screenshots',lambda: dict(layouts=[validate_styles(row) for row in styles],screenshots=screenshots))
                session.command('layout',value=1200)
                controls=action('controls'); keys=dict(before=action('focus'))
                for name,code in [('right',124),('left',123),('up',126),('down',125),('mute',46),('unmute',46),('play',49),('pause',49),('fullscreen',3),('windowed',3)]:
                    tv1.native(run,'key',session.process.pid,code); keys[name]=action('snapshot')
                run.observed['native_keys']=keys
                case(run,'custom-skin-controls','official seek/preview/captions/volume/rates and native arrows/Space/M/F',lambda: validate_controls(controls,keys))
                tv1.native(run,'hover',session.process.pid,600,160)
                hidden=action('autohide')
                case(run,'immersive-autohide','visible at 2.3s, controls/cursor hidden after 3s, movement/focus restore',lambda: require(hidden['before']['visible'] and not hidden['hidden']['visible'] and hidden['cursor']=='none' and hidden['opacity']=='0' and hidden['restored']['visible'] and hidden['focused']['visible'],'Three-second activity visibility differs'))
                tray=action('tray')
                case(run,'tray-episodes-streams','three tabs; real same-Resolution line and selected episode; disabled reason/color/no open; all business actions in tray',lambda: require(tray['opened']['tray'] and tray['sameResolution'] and tray['sessionChanged'] and tray['selected']==1 and tray['tracks']>=2 and tray['matches']==1 and tray['businessInTray'] and not tray['error'] and len(tray['lines'])>=3 and all(row['height']>=44 for row in tray['lines']) and tray['disabledBlocked'] and tray['disabledStyle']['color']=='rgba(235, 235, 245, 0.38)' and any(row['disabled'] and '当前设备不支持DRM' in row['label'] for row in tray['lines']),'Tray selection/ownership differs'))
                next=action('next',timeout=100)
                case(run,'next-episode-card','20s card, auto-off, trusted EOS 5s countdown, cancellation, epoch guard and history',lambda: validate_next(next))
                error=action('error')
                def error_check():
                    require(error['recovered'] and error['buttons']==['重试','换线路'] and error['panel']['borderRadius']=='20px' and error['title']['fontSize']=='17px' and error['title']['fontWeight']=='600' and error['description']['fontSize']=='14px','Error overlay/retry differs')
                    near(error['panel']['rect']['width'],420,'Error panel width')
                    return error
                case(run,'error-overlay','retryable error glass panel and actual successful retry',error_check)
                escape=[action('escape-start')]
                for _ in range(3):
                    tv1.native(run,'key',session.process.pid,53); escape.append(action('escape-state'))
                run.observed['escape']=escape
                case(run,'escape-levels','native Escape closes tray, then fullscreen, then returns to details',lambda: require(escape[0]['tray'] and escape[0]['fullscreen'] and not escape[1]['tray'] and escape[1]['fullscreen'] and escape[1]['route']=='playback' and not escape[2]['fullscreen'] and escape[2]['route']=='playback' and escape[3]['route']=='details' and escape[3]['hash'].startswith('#/details/') and escape[3]['videos']==0 and not escape[3]['errors'],'Escape did not unwind one level at a time'))
                disposed=action('dispose'); require(disposed['videos']==0 and not disposed['errors'],'TV6 media/console cleanup failed')
                session.close(); require(session.record.get('exit')==0 and session.record.get('lingering')==[] and fixture.active==0,'Native TV6/fixture resources remain')
            bridge.finish(); run.observed['immersive_host']=run.observed['host']
        return dict(real_window=True,dimensions=[row['width'] for row in styles])
    finally:
        os.environ.pop('T12_MIN_OPENS',None)
        try:
            if session is not None and not session.closed: session.close(graceful=False)
        finally:
            try:
                if bridge is not None: bridge.close()
            finally:
                run.folder=previous_folder


def diagnose_interactive():
    """Run only the current UI loop while debugging; qualification still uses A1."""
    run=Acceptance(ROOT/'evidence/acceptance/TV6-diagnostic.json',ROOT/'evidence/acceptance/TV6-diagnostic.xml')
    previous=t01.DATA,t01.BUILD
    awake=subprocess.Popen(['/usr/bin/caffeinate','-di','-w',str(os.getpid())]) if sys.platform=='darwin' else None
    try:
        resources(run,card='T0.33')
        t01.DATA=run.folder/'packaged';t01.DATA.mkdir();t01.BUILD=t01.DATA/'build.json'
        t01.build(run);interactive(run)
        require(all(case['result']=='PASS' for case in run.cases),'UI diagnostic has failed behavioral checks')
    finally:
        t01.DATA,t01.BUILD=previous
        write_json(run.folder/'diagnostic.json',dict(source=run.source,cases=run.cases,observed=run.observed,commands=run.commands))
        if awake is not None:awake.terminate();awake.wait(timeout=10)
    print('UI diagnostic evidence: '+str(run.folder/'diagnostic.json'))


def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--suite',choices=['all'],required=True); parser.add_argument('--report',type=Path,required=True); parser.add_argument('--junit',type=Path,required=True); args=parser.parse_args()
    require(args.report.resolve()==ROOT/'evidence/acceptance/TV6-all.json' and args.junit.resolve()==ROOT/'evidence/acceptance/TV6-all.xml','Exact TV6 paths required')
    run=Acceptance(args.report,args.junit)
    previous=t01.DATA,t01.BUILD
    try:
        t01.DATA=run.folder/'packaged'; t01.DATA.mkdir(); t01.BUILD=t01.DATA/'build.json'
        if run.case('resource-ownership',['REQ-TV6'],'original T0.33 scope and sampled process watchdog',lambda: resources(run,card='T0.33')):
            def oracles():
                output=run.command(['{python}','-B','-m','unittest','discover','-s','tests/acceptance','-p','test_tv6.py'])
                count=re.search(r'Ran (\d+) tests?',output); require(count and int(count[1])>=4,'Nonzero independent negative oracle tests required'); return dict(tests=int(count[1]))
            run.case('negative-oracles',['REQ-TV6'],'corrupted independent style/control/next evidence is rejected',oracles)
            built=run.case('packaged-build',['REQ-TV6'],'current-source production and automation bundles',lambda:t01.build(run))
            if built:
                run.case('packaged-immersive',['TV-PLAYER','REQ-TV6'],'real window current-source interaction loop and screenshots',lambda:interactive(run))
                def recycle():
                    run.command(['clang','-fobjc-arc','-framework','Cocoa','-framework','ApplicationServices','tests/fixtures/tv1_native.m','-o',str(run.folder/'tv1-native')])
                    def activate(session):
                        session.observe();tv1.native(run,'click',session.process.pid,600,200)
                    return t12.webview(run,build=False,before_identity=activate)
                case(run,'switch-recycle-50','original 50 native frame/audio/Session/RSS recycle and MP4/HLS/error/EOS/exit gates',recycle)
    finally:
        t01.DATA,t01.BUILD=previous
    return run.save()


if __name__=='__main__':
    awake=subprocess.Popen(['/usr/bin/caffeinate','-di','-w',str(os.getpid())]) if sys.platform=='darwin' else None
    try:code=main()
    finally:
        if awake is not None:awake.terminate();awake.wait(timeout=10)
    raise SystemExit(code)
