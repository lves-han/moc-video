"""TV7: actual packaged window, production search Host, library and grouped forms."""
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
import tv5
from t00 import Acceptance as CommandRunner, require, write_json
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
LIVE_TEST = 'host::maccms::tests::t08::t09::tv4::tv5::tv7::tv7_packaged_host_bridge'
CHILDREN = {'TV-SEARCH':['debounce-cancel','grouped-results','empty-genres','visual-spec','permissions-exact'],
            'TV-LIBRARY':['three-tabs','history-actions-kept'], 'TV-MANAGE':['grouped-lists','actions-kept']}
IDS = [*CHILDREN,*[f'{p}/{n}' for p,names in CHILDREN.items() for n in names],'REQ-TV7']


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = 'T0.34','TV7',IDS
    DEPLOYMENT = 'actual packaged Tauri WKWebView, production Rust Host/SQLite/gateway and controlled owned search fixtures'
    DEFERRED = tv5.Acceptance.DEFERRED
    command = CommandRunner.command


class Bridge(t14.Bridge):
    def invoke(self, value):
        # The production control path remains responsive while ordinary work waits.
        with self.lock:
            self.sequence += 1
            path = self.folder / f'request-{self.sequence}.json'
            temporary = path.with_suffix('.tmp')
            write_json(temporary,value)
            temporary.replace(path)
            target = self.folder / f'response-{self.sequence}.json'
        return self.wait(target)


def case(run,parent,name,expected,function):
    return run.case(name,[parent,f'{parent}/{name}','REQ-TV7'],expected,function)


def security():
    main=json.loads((ROOT/'src-tauri/capabilities/main.json').read_text())
    expected=tv1.PERMISSIONS+['core:window:allow-start-dragging','allow-list-watchlist','allow-add-watchlist','allow-remove-watchlist','allow-search-catalog']
    require(len(main['permissions'])==len(expected) and set(main['permissions'])==set(expected),'permission set must equal T0.32 plus search only')
    require(set(main)=={'identifier','description','windows','permissions'} and main['identifier']=='main' and main['windows']==['main'],'main capability scope differs')
    require(json.loads((ROOT/'src-tauri/tauri.conf.json').read_text())['app']['security']==dict(capabilities=['main'],csp=tv1.CSP),'CSP differs from baseline')
    return dict(permissions=len(expected),csp='exact baseline')


def native(run):
    output=run.command(['cargo','test','--locked','--offline','--manifest-path','src-tauri/Cargo.toml','--lib','tv7_','--','--test-threads=1','--nocapture','--skip','tv7_packaged_host_bridge'],timeout=600)
    names=re.findall(r'^test (?:\w+::)*(tv7_\w+) \.\.\.',output,re.M)
    require(set(names)=={'tv7_declared_search_catalogs_maccms_wd_and_closed_inputs','tv7_cancellation_and_per_source_serial_queue'} and 'test result: ok. 2 passed; 0 failed; 0 ignored;' in output,'two named real Host search tests must pass')
    rows={marker:[json.loads(line.split(marker+':',1)[1]) for line in output.splitlines() if line.startswith(marker+':')] for marker in ['TV7_SEARCH','TV7_CANCEL']}
    require(all(len(values)==1 for values in rows.values()),'one observation from each native search test')
    run.observed['native']=rows
    return dict(tests=2,observations=rows)


def frontend(run):
    output=run.command(['node','--experimental-strip-types','--test','--test-reporter=tap','--test-concurrency=1','tests/tv7.test.ts'])
    require(all(re.search(fr'^# {key} {value}$',output,re.M) for key,value in [('tests',5),('pass',5),('fail',0),('skipped',0)]),'five frontend search identity/cancel/late-response/poster behaviors must pass')
    return dict(tests=5,passed=5)


def healthy(row):
    require(row['width'] in [960,1200,1512] and row['origin']=='tauri://localhost' and row['title'] and not row['overflow'] and not row['errors'],'real packaged identity/viewport/console health differs')


def search_styles(row):
    healthy(row);s=row['styles'];bar=s['.search-bar'];genre=s['.search-genre']
    tv1.near(bar['rect']['top'],72,'search top');require(bar['height']=='56px' and bar['max-width']=='720px' and bar['border-radius']=='16px' and bar['background-color']=='rgba(255, 255, 255, 0.1)','search geometry/surface')
    require(s['.search-input']['font-size']=='20px' and bar['border-top-width']=='1px' and bar['border-top-color']=='rgba(255, 255, 255, 0.1)' and row['focused']['border-top-color']=='rgb(245, 245, 247)','input typography and focus border')
    require(genre['height']=='96px' and genre['rect']['width']>=200 and genre['border-radius']=='14px' and genre['background-color']=='rgba(255, 255, 255, 0.06)','genre card geometry/surface')
    require(s['.search-genre span']['font-size']=='17px' and s['.search-genre span']['font-weight']=='600' and genre['transition-duration']=='0.2s' and row['genres']>=4,'genre type/cards/motion')
    return dict(width=row['width'],genres=row['genres'])


def library_styles(row):
    healthy(row);s=row['styles'];title=s['.library-heading h1'];tabs=s['.library-tabs']
    tv1.near(title['rect']['top'],72,'library title top');require(title['font-size']=='34px' and title['font-weight']=='700','library title type')
    require(tabs['height']=='36px' and tabs['border-radius']=='999px' and tabs['background-color']=='rgba(30, 30, 34, 0.62)' and 'blur(30px)' in tabs['-webkit-backdrop-filter'] and s['#library-tab-history']['background-color']=='rgba(255, 255, 255, 0.18)','three segment glass/current style')
    require(row['tabs']==['Continue watching','Watchlist','Watch history'] and row['groups']==['Today','Yesterday','Earlier'] and row['continuing']['count']==2 and row['watchlist']['count']==1,'library three tabs and date groups')
    tv1.near(row['continuing']['poster']['rect']['width']/row['continuing']['poster']['rect']['height'],16/9,'continue landscape',.01)
    tv1.near(row['watchlist']['poster']['rect']['height']/row['watchlist']['poster']['rect']['width'],1.5,'watchlist poster',.01)
    require(s['.history-group > h2']['font-size']=='17px' and s['.history-group > h2']['font-weight']=='600' and s['.history-group > h2']['position']=='sticky' and 'blur(30px)' in s['.history-group > h2']['-webkit-backdrop-filter'],'sticky date group glass/type')
    require(s['.history-delete']['width']==s['.history-delete']['height']=='28px' and s['.history-delete']['border-radius']=='50%' and s['.history-delete']['opacity']=='0' and s['.history-clear']['font-size']=='13px' and s['.history-clear']['color']=='rgb(255, 69, 58)','delete and clear states')
    return dict(width=row['width'],groups=row['groups'])


def manage_styles(row,source):
    healthy(row);s=row['styles'];root='.native-sources' if source else '.tv-settings';title=s[root+' h1']
    require(s[root]['max-width']=='720px' and s[root]['rect']['width']<=720,'manage content width')
    tv1.near(title['rect']['top'],72,'manage title top');require(title['font-size']=='34px' and title['font-weight']=='700','manage title type')
    heading=s['.settings-group > h2'];panel=s['.settings-panel'];require(heading['font-size']=='13px' and heading['font-weight']=='600' and heading['color']=='rgba(235, 235, 245, 0.62)' and heading['margin-bottom']=='8px','group heading')
    require(panel['border-radius']=='14px' and panel['background-color']=='rgba(255, 255, 255, 0.06)','group surface/radius')
    prefix='.sources-row' if source else '.setting-row';line=s[prefix]
    require(line['min-height']=='52px' and line['padding-left']==line['padding-right']=='16px' and line['padding-top']==line['padding-bottom']=='12px' and s[prefix+' h3']['font-size']=='15px' and s[prefix+' p']['font-size']=='13px','group row dimensions and typography')
    control=s['.sources-pickers select' if source else '.setting-select:not(.narrow-language)'];require(control['height']=='36px' and control['border-radius']=='10px' and control['background-color']=='rgba(255, 255, 255, 0.1)','form control geometry/surface')
    if source:
        dot=s['.source-status'];require(dot['width']==dot['height']=='8px' and dot['background-color']=='rgb(48, 209, 88)' and row['actions']>=9,'available source state and original actions')
        require(row['dialog']['border-radius']=='20px' and row['dialog']['background-color']=='rgba(30, 30, 34, 0.62)' and 'blur(30px)' in row['dialog']['-webkit-backdrop-filter'] and row['input']['height']=='36px' and row['input']['border-radius']=='10px','add source glass form')
    else:
        require(row['swatches']==5 and s['.palette-swatch']['width']==s['.palette-swatch']['height']=='32px' and s['.palette-swatch']['border-radius']=='50%' and s['.palette-swatch[aria-checked=true]']['outline-width']=='2px' and s['.palette-swatch[aria-checked=true]']['outline-color']=='rgb(245, 245, 247)','five circular palettes and selected ring')
        require(s['.settings-language']['height']=='36px' and s['.settings-language [aria-checked=true]']['background-color']=='rgba(255, 255, 255, 0.18)','language segment state')
    return dict(width=row['width'],page=root)


def behavior(row):
    require(row['early']==0 and 290<=row['debounce_ms']<1500 and row['kept'] and row['pending_opacity']=='0.6' and row['cancelled'] and row['latest'] and not row['errors'],'debounce, cancellation, loading retention or late old success isolation failed')
    require([r['kind'] for r in row['rows']]==['movie','series','other'] and all(r['count']==3 for r in row['rows']),'movies/series/variety-short result rows differ')
    require(row['genres']>=4 and 'No results found' in row['empty']['text'] and row['local']['count']==3 and all(t.startswith('wd ') for t in row['local']['titles']),'no-result/type cards or MacCMS wd results differ')
    return dict(debounce_ms=row['debounce_ms'],rows=row['rows'])


def window(run,widths=(960,1200,1512)):
    previous=t01.DATA,t01.BUILD,t12.LIVE_TEST
    t01.DATA=run.folder/'packaged';t01.DATA.mkdir();t01.BUILD=t01.DATA/'build.json';t12.LIVE_TEST=LIVE_TEST
    session=bridge=None
    try:
        require(t01.native_gui_session()['screen_locked'] is not True,'Unlock macOS for real-window acceptance')
        t01.check_media();t01.build(run);receipt=t01.check_build(run.source)
        executable=ROOT/receipt['automation']['path']/'Contents/MacOS/moc-video'
        run.command(['clang','-fobjc-arc','-framework','Cocoa','-framework','ApplicationServices','tests/fixtures/tv1_native.m','-o',str(run.folder/'tv1-native')])
        with tempfile.TemporaryDirectory(prefix='tv7-live-',dir=run.folder) as temporary:
            folder=Path(temporary);media=folder/'media';media.mkdir();bridge=Bridge(run,folder,media);ready=bridge.wait(folder/'ready.json',timeout=180)
            with t01.Fixture(t01.MEDIA,invoke=bridge.invoke) as fixture:
                session=t01.NativeSession(run,fixture,executable,1);session.wait(0,30);session.command('identity')
                command=lambda action:session.command('tv7',action=action,payload=ready,timeout=120)
                command('prepare-shortcut');tv1.native(run,'key',session.process.pid,3,'cmd');shortcut=command('shortcut');run.observed['shortcut']=shortcut
                case(run,'TV-SEARCH','debounce-cancel','real native Cmd+F opens Search and focuses its input',lambda:require(shortcut['hash']=='#/search' and shortcut['focused'],'native keyboard shortcut failed'))
                result=command('search-behavior');run.observed['search']=result
                for name in ['debounce-cancel','grouped-results','empty-genres']:case(run,'TV-SEARCH',name,'real Host search, 300ms debounce, cancel old UUID, release late old success after fresh, grouped rows and empty states',lambda:behavior(result))
                layouts=[];screenshots=[]
                for width in widths:
                    session.command('layout',value=width);rows={}
                    for action in ['search-styles','results-styles','library-styles','sources-styles','settings-styles']:
                        row=command(action);rows[action]=row
                        shot=tv1.capture(run,session.process.pid,width,row['native']['scale_factor']);path=ROOT/shot['path'];target=path.with_name(f'{action}-{width}.png');path.rename(target);shot['path']=str(target.relative_to(ROOT));screenshots.append(shot)
                        if action=='search-styles':
                            tv1.native(run,'hover',session.process.pid,5,50);command('search-hover-arm');tv1.native(run,'hover',session.process.pid,row['hover']['x'],row['hover']['y']);row['hovered']=command('search-hover');tv1.native(run,'hover',session.process.pid,5,50)
                        if action=='library-styles':
                            tv1.native(run,'hover',session.process.pid,row['hover']['x'],row['hover']['y']);row['hovered']=command('history-hover');tv1.native(run,'hover',session.process.pid,5,50)
                    layouts.append(rows)
                run.observed.update(layouts=layouts,screenshots=screenshots)
                for rows in layouts:
                    case(run,'TV-SEARCH','visual-spec','ADR-057 search computed styles and native hover at required widths',lambda rows=rows: [search_styles(rows['search-styles']),require(rows['search-styles']['hovered']['native_event'] and rows['search-styles']['hovered']['hovered'] and rows['search-styles']['hovered']['transform'].startswith('matrix(1.04,'),'genre native hover scale'),require(rows['results-styles']['rows']==3,'three MediaRows')])
                    case(run,'TV-LIBRARY','three-tabs','continue landscape, watchlist wall, dated history and glass segments at required widths',lambda rows=rows:[library_styles(rows['library-styles']),require(rows['library-styles']['hovered']['opacity']=='1','history hover delete')])
                    case(run,'TV-MANAGE','grouped-lists','ADR-057 grouped sources/settings geometry/colors/type/controls and glass source dialog',lambda rows=rows:[manage_styles(rows['sources-styles'],True),manage_styles(rows['settings-styles'],False)])
                settings=command('settings-actions');run.observed['settings_actions']=settings
                case(run,'TV-MANAGE','actions-kept','persist auto-next, speed, five palettes, both locales, cache validation/save/clear',lambda:require(settings['auto']!=settings['auto_before'] and settings['speed']==1.5 and settings['palettes']==['lime','ocean','violet','rose','amber'] and '设置' in settings['chinese'] and settings['invalid'] and settings['max']==67108864 and settings['cache']==dict(used=0,pending=0) and not settings['errors'],'settings actions lost'))
                sources=command('sources-actions');run.observed['sources_actions']=sources
                case(run,'TV-MANAGE','actions-kept','source validation/add/edit/failed row/recovery/disable/enable/delete cancel/confirm and diagnostics copy feedback',lambda:require(sources['before']==sources['after'] and sources['added']==sources['cancelled']==sources['before']+1 and sources['invalid'] and sources['diagnostics'] and sources['copy_feedback'] and sources['presets']==8 and sources['disabled']['background-color']=='rgba(235, 235, 245, 0.38)' and sources['failed']['dot']['background-color']=='rgb(255, 69, 58)' and sources['failed']['error']['font-size']=='13px' and not sources['errors'],'source actions or state styles lost'))
                history=command('history-actions');run.observed['history_actions']=history
                case(run,'TV-LIBRARY','history-actions-kept','real SQLite delete cancel, delete one, clear all, preserve watchlist and glass confirmation',lambda:require((history['cancelled'],history['removed'],history['cleared'],history['watchlist'])==(3,2,0,1) and history['modal']['border-radius']=='20px' and history['modal']['background-color']=='rgba(30, 30, 34, 0.62)' and 'blur(30px)' in history['modal']['-webkit-backdrop-filter'] and not history['errors'],'history actions/confirmation lost'))
                require(command('errors')==[],'actual window uncaught errors');session.command('identity');session.close()
                require(session.record.get('exit')==0 and session.record.get('lingering')==[] and fixture.active==0,'owned App/WebKit/HTTP cleanup missing')
            bridge.finish()
        return dict(window='actual packaged WKWebView',screenshots=screenshots)
    finally:
        try:
            if session is not None and not session.closed:session.close(graceful=False)
        finally:
            if bridge is not None:bridge.close()
            t01.DATA,t01.BUILD,t12.LIVE_TEST=previous


def hover_diagnostic(run):
    run.command(['clang','-fobjc-arc','-framework','Cocoa','-framework','ApplicationServices','tests/fixtures/tv1_native.m','-o',str(run.folder/'tv1-native')])
    original=t01.NativeSession.command
    def observe(session,op,**kwargs):
        if op!='tv7':return original(session,op,**kwargs)
        ready=kwargs['payload'];rows=[]
        for _ in range(3):
            row=original(session,'tv7',action='search-styles',payload=ready)
            tv1.native(run,'hover',session.process.pid,5,50)
            original(session,'tv7',action='search-hover-arm',payload=ready)
            tv1.native(run,'hover',session.process.pid,row['hover']['x'],row['hover']['y'])
            row['hovered']=original(session,'tv7',action='search-hover',payload=ready)
            rows.append(row)
        return rows
    def validate(rows):
        require(len(rows)==3,'three independent native entries required')
        for row in rows:
            search_styles(row);value=row['hovered']
            require(value['native_event'] and value['hovered'] and value['focused'] and value['transform'].startswith('matrix(1.04,'),'actual native hover did not settle at the original scale')
    t01.NativeSession.command=observe
    try:return t14.packaged(run,LIVE_TEST,'tv7',validate,'hover_observations',lambda rows:dict(samples=len(rows)),bridge_class=Bridge)
    finally:t01.NativeSession.command=original


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--suite',choices=['all','native','window','diagnostic','hover'],required=True);parser.add_argument('--report',type=Path,required=True);parser.add_argument('--junit',type=Path,required=True);args=parser.parse_args()
    run=Acceptance(args.report,args.junit)
    if args.suite!='all':run.REQUIREMENTS=['REQ-TV7']
    if run.case('resource-ownership',['REQ-TV7'],'original runner and actual macOS arm64 sampled resource budget',lambda:resources(run,card=run.CARD)):
        case(run,'TV-SEARCH','permissions-exact','T0.32 permissions plus search only, exact baseline CSP',security)
        if args.suite in ['all','native']:
            case(run,'TV-SEARCH','debounce-cancel','real Host declared Stremio catalogs/MacCMS wd, native cancellation and serial queue',lambda:native(run))
            case(run,'TV-SEARCH','debounce-cancel','frontend late response, cancellation and identity regressions',lambda:frontend(run))
        if args.suite in ['all','window']:run.case('packaged-window',['TV-SEARCH','TV-LIBRARY','TV-MANAGE','REQ-TV7'],'real packaged search/library/grouped forms at 960/1200/1512, native keyboard/hover/screenshots',lambda:window(run))
        if args.suite=='diagnostic':run.case('diagnostic-window',['REQ-TV7'],'focused actual 1200px computed styles, settled native hover, source error recovery and original actions',lambda:window(run,(1200,)))
        if args.suite=='hover':run.case('native-hover-diagnostic',['REQ-TV7'],'observe trusted pointer entry and completed CSS transition three times in the actual window',lambda:hover_diagnostic(run))
    return run.save()


if __name__=='__main__':
    awake=subprocess.Popen(['/usr/bin/caffeinate','-di','-w',str(os.getpid())]) if sys.platform=='darwin' else None
    try:raise SystemExit(main())
    finally:
        if awake is not None:awake.terminate();awake.wait(timeout=5)
