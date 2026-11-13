import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, unquote, urlsplit
import time
from pathlib import Path

STATE = {"fail": False, "fail_addon_b": False, "add_episode": False, "changed_title": False, "line_mode": "normal", "delay": 0.0, "detail_delay": 0.0, "media_url": "", "counts": {}, "t4_mode": "normal", "t4_requests": [], "rest_requests": [], "rest_contexts": {}, "stream_mode": "normal", "stream_requests": [], "parser_mode":"normal", "parser_requests": [], "probe_fail": False, "tmdb_requests": []}


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        url = urlsplit(self.path)
        query = parse_qs(url.query)
        if url.path == "/control":
            if "fail" in query: STATE["fail"] = query["fail"][0] == "1"
            if "fail_addon_b" in query: STATE["fail_addon_b"] = query["fail_addon_b"][0] == "1"
            if "add_episode" in query: STATE["add_episode"] = query["add_episode"][0] == "1"
            if "changed_title" in query: STATE["changed_title"] = query["changed_title"][0] == "1"
            if "line_mode" in query: STATE["line_mode"] = query["line_mode"][0]
            if "delay" in query: STATE["delay"] = float(query["delay"][0])
            if "detail_delay" in query: STATE["detail_delay"] = float(query["detail_delay"][0])
            if "media_url" in query: STATE["media_url"] = query["media_url"][0]
            if "probe_fail" in query: STATE["probe_fail"] = query["probe_fail"][0] == "1"
            if "parser_mode" in query: STATE["parser_mode"] = query["parser_mode"][0]
            if "reset_parser" in query: STATE["parser_requests"].clear()
            if "t4_mode" in query: STATE["t4_mode"] = query["t4_mode"][0]
            if "reset" in query: STATE["counts"].clear()
            if "reset_tmdb" in query: STATE["tmdb_requests"].clear()
            if "reset_t4" in query: STATE["t4_requests"].clear()
            if "reset_rest" in query: STATE["rest_requests"].clear(); STATE["rest_contexts"].clear()
            if "stream_mode" in query: STATE["stream_mode"] = query["stream_mode"][0]
            if "reset_stream" in query: STATE["stream_requests"].clear()
            return self.send_json(STATE)
        if url.path == "/stats":
            return self.send_json(STATE)
        STATE["counts"][url.path] = STATE["counts"].get(url.path, 0) + 1
        time.sleep(STATE["delay"])
        if STATE["fail"]:
            self.send_response(503)
            self.end_headers()
            return
        if STATE["fail_addon_b"] and url.path.startswith("/addon-b/"):
            self.send_response(503)
            self.end_headers()
            return
        if url.path.startswith(('/stream-tt/', '/stream-raw/', '/stream-series/', '/stream-none/', '/stream-open/')):
            return self.stream_addon(url)
        if url.path.startswith(("/addon-a/", "/addon-b/", "/addon-same/")):
            return self.addon(url)
        if url.path in ("/json-parser", "/json-other"):
            STATE["parser_requests"].append({"path":url.path,"query":query,"raw_query":url.query,"header":self.headers.get("X-Parser")})
            if self.headers.get("X-Parser") != "allowed": self.send_response(403); self.end_headers(); return
            if STATE["parser_mode"] == "html": self.send_response(200); self.end_headers(); self.wfile.write(b"<html>error</html>"); return
            if STATE["parser_mode"] == "missing": return self.send_json({"data":{}})
            if STATE["parser_mode"] == "window": return self.send_json({"url":STATE["media_url"],"headers":{"Referer":"https://parsed.test/"}})
            return self.send_json({"data":{"stream":"https://media.test/parsed.m3u8?generation="+query.get("gen",["1"])[0]},"headers":{"Referer":"https://parsed.test/","X-Media":"media"}})
        if url.path in ("/t4-query", "/t4-js0", "/web/302redirect"):
            STATE["t4_requests"].append({"path":url.path,"query":query,"header":self.headers.get("X-Fixture")})
            if self.headers.get("X-Fixture") != "allowed":
                self.send_response(403); self.end_headers(); return
            if STATE["t4_mode"] == "html":
                self.send_response(200); self.end_headers(); self.wfile.write(b'<html>error</html>'); return
            if url.path == "/web/302redirect":
                if STATE["t4_mode"] == "js0hint": return self.send_json({"url":"https://media.test/watch?id=js0","type":"hls"})
                return self.send_json({"url":"https://media.test/js0.mp4?sig=a%2Fb", "headers":{"Referer":"https://fixture.test/"}})
            if "play" in query:
                mode = STATE["t4_mode"]
                values = ["HD","https://media.test/hd.m3u8?generation=2","SD","https://media.test/sd.mp4?generation=2"]
                if mode == "normal": values = ["HD","https://media.test/hd.m3u8?generation=1","SD","https://media.test/sd.mp4?generation=1"]
                if mode == "window": return self.send_json({"url":"https://watch.test/episode?id=a%2Fb&part=1","parse":1})
                if mode in ("parse", "jx"): values = ["HD","https://watch.test/episode?id=a%2Fb&part=1","SD","https://watch.test/episode?id=c%2Fd&part=2"]
                if mode == "reverse": values = values[2:] + values[:2]
                if mode == "duplicate": values[2] = "HD"
                if mode == "missing": return self.send_json({"parse":0})
                if mode == "hint": values = ["HD","https://media.test/watch?id=hd","SD","https://media.test/watch?id=sd"]
                if mode == "dash": values = ["HD","https://media.test/video.mpd","SD","https://media.test/video.mkv"]
                if mode == "emptyheader": return self.send_json({"url":values,"parse":0,"header":"","headers":{"Referer":"https://fixture.test/"}})
                return self.send_json({"url":values,"type":"hls" if mode == "hint" else "", "parse":int(mode == "parse"),"jx":int(mode == "jx"),"header":{"Referer":"https://fixture.test/"}})
            video={"vod_id":"201","vod_name":"Show","type_name":"电视剧","vod_year":"2020","vod_play_from":"fixture-flag","vod_play_url":"S01E02$play-2#S01E03$play-3"}
            if "ids" in query and query["ids"] != ["201"]: return self.send_json({"list":[]})
            if "wd" in query and query["wd"] != ["Show"]: return self.send_json({"list":[]})
            return self.send_json({"list":[video]})
        if url.path == "/maccms":
            movie = {"vod_id":1,"vod_name":"Film","type_name":"电影","vod_year":"2020","vod_play_from":"fixture","vod_play_url":"Film$"+STATE["media_url"]}
            return self.send_json({"code":1,"page":1,"pagecount":1,"limit":20,"total":1,"list":[movie]})
        if url.path in ("/maccms-json", "/maccms-xml", "/maccms-invalid", "/maccms-match", "/maccms-slow"):
            if self.headers.get("X-Fixture") != "allowed":
                self.send_response(403); self.end_headers(); return
            if "sig=" in url.query and "sig=a%2Fb+c" not in url.query:
                self.send_response(400); self.end_headers(); return
            if url.path == "/maccms-invalid": return self.send_json({"code":0,"message":"closed"})
            if url.path == "/maccms-slow": time.sleep(3)
            if url.path in ("/maccms-match", "/maccms-slow"): return self.matching(query)
            if url.path == "/maccms-json":
                return self.send_json({"code":1,"page":1,"pagecount":1,"list":[{"vod_id":101,"vod_name":"Film","type_name":"电影","vod_year":"2020","vod_play_from":"hls$$$direct","vod_play_url":"Episode 1$https://media.test/video?sig=a$b#Episode 2$https://media.test/2$$$https://media.test/direct"}]})
            data=b'<rss><list><video><id>101</id><name>Film</name><type>movie</type><year>2020</year><dl><dd flag="hls"><![CDATA[Episode 1$https://media.test/video?sig=a$b#Episode 2$https://media.test/2]]></dd><dd flag="direct"><![CDATA[https://media.test/direct]]></dd></dl></video></list></rss>'
            self.send_response(200); self.send_header("Content-Type","application/xml"); self.send_header("Content-Length",str(len(data))); self.end_headers(); self.wfile.write(data); return
        if url.path == "/broken.mp4":
            if STATE["counts"].get(url.path, 0) > 1:
                self.send_response(503); self.end_headers(); return
            return self.media()
        if url.path == "/video.mp4":
            return self.media()
        if url.path == "/live.m3u":
            payload = ("#EXTM3U\n#EXTINF:-1,Fixture live\n" + STATE["media_url"].rstrip('/') + "/video.mp4\n").encode()
            self.send_response(200); self.send_header("Content-Length", str(len(payload))); self.end_headers(); self.wfile.write(payload); return
        if url.path.startswith('/play-hls/'):
            filename = url.path.removeprefix('/play-hls/')
            if '/' in filename or filename not in ('index.m3u8', 'segment0.ts', 'segment1.ts', 'segment2.ts', 'segment3.ts', 'segment4.ts', 'segment5.ts', 'segment6.ts'):
                self.send_response(404); self.end_headers(); return
            path = Path('/hls') / filename
            if not path.is_file():
                self.send_response(404); self.end_headers(); return
            data = path.read_bytes()
            raw = self.headers.get('Range', '')
            if raw.startswith('bytes='):
                start, _, end = raw[6:].partition('-')
                data = data[int(start or 0):int(end) + 1 if end else None]
            self.send_response(206 if raw else 200)
            self.send_header('Content-Type', 'application/vnd.apple.mpegurl' if filename.endswith('.m3u8') else 'video/mp2t')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers(); self.wfile.write(data); return
        if url.path in ("/probe-fast.mp4", "/probe-slow.mp4", "/hls/segment.ts"):
            if STATE["probe_fail"] and url.path == "/probe-fast.mp4": self.send_response(503); self.end_headers(); return
            time.sleep(0.05 if url.path == "/probe-fast.mp4" else 0.4 if url.path == "/probe-slow.mp4" else 0)
            payload = b"controlled-media-bytes"
            self.send_response(206 if self.headers.get("Range") else 200)
            self.send_header("Content-Length", str(len(payload)))
            if self.headers.get("Range"): self.send_header("Content-Range", f"bytes 0-{len(payload)-1}/{len(payload)}")
            self.end_headers()
            self.wfile.write(payload)
            return
        if url.path in ("/hls/master.m3u8", "/hls/variant.m3u8"):
            payload = ("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvariant.m3u8\n" if url.path.endswith("master.m3u8") else "#EXTM3U\n#EXTINF:1,\nsegment.ts\n").encode()
            self.send_response(200)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        if query.get("api_key") != ["fixture-key"]:
            self.send_response(401)
            self.end_headers()
            return
        lang = query.get("language", ["en-US"])[0]
        if url.path.startswith('/3/discover/'):
            allowed = ('with_origin_country','with_original_language','with_genres','primary_release_year','first_air_date_year','page','language','sort_by')
            STATE['tmdb_requests'].append({'path':url.path,'query':{key:query[key] for key in allowed if key in query}})
        if url.path == "/3/configuration/countries":
            data = [{"iso_3166_1":code,"english_name":name} for code,name in [('CN','China'),('HK','Hong Kong'),('TW','Taiwan'),('JP','Japan'),('US','United States')]]
        elif url.path == "/3/configuration/languages":
            data = [{"iso_639_1":"en", "english_name":"English"}]
        elif url.path.startswith("/3/genre/"):
            data = {"genres":[{"id":18, "name":"剧情" if lang == "zh-CN" else "Drama"}]}
        elif url.path.startswith(("/3/discover/", "/3/search/", "/3/trending/")) or url.path in ("/3/movie/now_playing", "/3/movie/upcoming", "/3/movie/top_rated", "/3/tv/on_the_air", "/3/tv/top_rated"):
            movie = "/movie" in url.path
            suffix = " P2" if query.get("page") == ["2"] and query.get("query") == ["film"] else ""
            data = {"results":[{"id":42, "title":("电影" if lang == "zh-CN" else "Film") + suffix, "name":"剧集" if lang == "zh-CN" else "Series", "release_date":"2020-01-01", "first_air_date":"2021-01-01", "genre_ids":[18], "overview":"fixture", "vote_average":8.0}], "page": int(query.get("page",[1])[0]), "fixture_movie":movie}
            if "/search/tv" in url.path and query.get("query") == ["Show"]:
                data["results"] = [{"id":44,"name":"Show","first_air_date":"2020-01-01","genre_ids":[18],"overview":"fixture","vote_average":8.0}]
            if "/search/movie" in url.path and query.get("query") == ["Same"]:
                data["results"] = [{"id":45,"title":"Same","release_date":"2020-01-01","genre_ids":[18],"overview":"fixture","vote_average":8.0}]
        elif url.path in ("/3/movie/42", "/3/movie/45", "/3/movie/99", "/3/tv/42", "/3/tv/43", "/3/tv/44"):
            movie = url.path.startswith("/3/movie")
            data = {"id":43 if url.path == "/3/tv/43" else 42,"title":"电影" if lang == "zh-CN" else "Film","name":"剧集" if lang == "zh-CN" else "Series", "original_title":"Original Film", "original_name":"Original Series", "release_date":"2020-01-01", "first_air_date":"2021-01-01", "status":"Returning Series", "external_ids":{"imdb_id":None if url.path == "/3/tv/43" else "tt1234567"}, "seasons":[] if movie else [{"season_number":1}], "alternative_titles":{"titles":[{"iso_3166_1":"CN", "title":"别名"},{"iso_3166_1":"US", "title":"Alias"}]}, "credits":{"cast":[],"crew":[]}, "genres":[], "production_countries":[], "runtime":90}
            if url.path == "/3/movie/45": data.update(id=45,title="Same",original_title="Same",external_ids={"imdb_id":None})
            if url.path == "/3/tv/44": data.update(id=44,name="Show",original_name="Show",first_air_date="2020-01-01",external_ids={"imdb_id":"tt7654321"})
        elif url.path in ("/3/tv/42/season/1", "/3/tv/43/season/1", "/3/tv/44/season/1"):
            data = {"episodes":[{"episode_number":1,"name":"第一集" if lang == "zh-CN" else "Episode 1","air_date":"2021-01-01"}]}
            if url.path == "/3/tv/44/season/1": data={"episodes":[{"episode_number":2,"name":"Episode 2"},{"episode_number":3,"name":"Episode 3"},{"episode_number":9,"name":"Episode 9"}]}
        else:
            self.send_response(404)
            self.end_headers()
            return
        self.send_json(data)

    def matching(self, query):
        def video(identifier, title, kind="电影", year="2020", episodes="Full$https://media.test/film"):
            return {"vod_id":identifier,"vod_name":title,"type_name":kind,"vod_year":year,"vod_play_from":"hls","vod_play_url":episodes}
        records={
            "101":video("101","Film (2020)"), "102":video("102","Film (2019)",year="2019"),
            "103":video("103","Film 解说"), "104":video("104","Film Special",kind="其他"),
            "105":video("105","Film 2"), "106":video("106","Film",kind="电视剧"),
            "107":video("107","Same"), "108":video("108","Same"), "109":video("109","Mystery",kind="其他",year=""),
            "201":video("201","Show",kind="电视剧",episodes="S01E02$https://media.test/2#S01E03$https://media.test/3"),
            "202":video("202","Show",kind="电视剧",episodes="S01E02$https://media.test/a#S01E02$https://media.test/b"),
            "203":video("203","Show",kind="电视剧",episodes="S01E03$https://media.test/3")}
        if STATE["media_url"]:
            flavor = self.headers.get("X-Probe", "fast")
            path = {"fast": "/probe-fast.mp4", "slow": "/probe-slow.mp4", "hls": "/hls/master.m3u8", "playable": "/video.mp4", "hls-playable": "/play-hls/index.m3u8", "broken": "/broken.mp4"}.get(flavor, "/probe-fast.mp4")
            records["201"]["vod_play_url"] = "S01E02$" + STATE["media_url"].rstrip("/") + path
            for item in ("107", "108"):
                records[item]["vod_play_url"] = "Full$" + STATE["media_url"].rstrip("/") + path
        if STATE["add_episode"]: records["201"]["vod_play_url"] += "#S01E04$https://media.test/4"
        if STATE["changed_title"]: records["201"]["vod_name"] = "Different Show"
        if STATE["line_mode"] == "reordered": records["201"]["vod_play_url"] = "S01E03$https://media.test/3#S01E02$https://media.test/new"
        if STATE["line_mode"] == "missing": records["201"]["vod_play_url"] = "S01E03$https://media.test/3"
        if STATE["line_mode"] == "duplicate": records["201"]["vod_play_url"] = "S01E02$https://media.test/a#S01E02$https://media.test/b"
        if "ids" in query:
            STATE["counts"]["detail_started"] = STATE["counts"].get("detail_started", 0) + 1
            time.sleep(STATE["detail_delay"])
            rows=[records[query["ids"][0]]] if query["ids"][0] in records else []
        else:
            text=query.get("wd",[""])[0]
            if text=="Film": rows=[video(str(300+i),f"Unrelated {i}") for i in range(12)]+[records[str(i)] for i in range(101,107)]
            elif text=="Original": rows=[records["101"]]
            elif text=="Same": rows=[records["107"],records["108"]]
            elif text=="Mystery": rows=[records["109"]]
            elif text=="Show": rows=[records["201"],records["202"],records["203"]]
            else: rows=[]
        return self.send_json({"code":1,"page":1,"pagecount":1,"list":rows})

    def addon(self, url):
        parts = [unquote(part) for part in url.path.strip('/').split('/')]
        addon = parts[0]
        label = "Addon A" if addon == "addon-a" else "Addon B" if addon == "addon-b" else "Same"
        if parts[1:] == ["manifest.json"]:
            return self.send_json({"id":addon,"version":"1.0.0","name":label,"description":"fixture","resources":["catalog",{"name":"meta","types":["movie"],"idPrefixes":["same:"]}],"types":["movie"],"idPrefixes":["same:"],"catalogs":[{"type":"movie","id":"all","name":label,"extra":[{"name":"skip"}]},{"type":"movie","id":"catalog","name":label,"extra":[{"name":"search","isRequired":True},{"name":"skip"}]}]})
        if len(parts) == 4 and parts[1:3] == ["catalog", "movie"] and parts[3] == "all.json":
            return self.send_json({"metas":[{"id":"same:42","type":"movie","name":label}]})
        if len(parts) == 5 and parts[1:3] == ["catalog", "movie"] and parts[3] in ("catalog", "all"):
            extras = parse_qs(parts[4].removesuffix('.json'))
            return self.send_json({"metas":[{"id":"same:42","type":"movie","name":label,"query":extras.get("search"),"skip":extras.get("skip")} ]})
        if len(parts) == 4 and parts[1:3] == ["meta", "movie"]:
            return self.send_json({"meta":{"id":parts[3].removesuffix('.json'),"type":"movie","name":label}})
        self.send_response(404)
        self.end_headers()

    def stream_addon(self, url):
        parts = [unquote(part) for part in url.path.strip('/').split('/')]
        owner = parts[0]
        STATE['stream_requests'].append({'owner':owner,'path':url.path,'query':parse_qs(url.query),'header':self.headers.get('X-Fixture')})
        if self.headers.get('X-Fixture') != 'allowed': self.send_response(403); self.end_headers(); return
        if parts[1:] == ['manifest.json']:
            resource = {'name':'stream','types':['series'] if owner == 'stream-series' else ['movie','series'], 'idPrefixes':['same:'] if owner == 'stream-raw' else ['tt']}
            if owner == 'stream-open': resource.pop('idPrefixes')
            return self.send_json({'id':'fixture.'+owner,'name':owner,'version':'1.0.0','resources':['catalog'] if owner == 'stream-none' else [resource], 'types':['movie','series'], 'idPrefixes':['tt'] if owner == 'stream-open' else [], 'catalogs':[]})
        if len(parts) != 4 or parts[1] != 'stream' or not parts[3].endswith('.json'):
            self.send_response(404); self.end_headers(); return
        mode=STATE['stream_mode'];generation='1' if mode == 'normal' else '2'
        first={'name':'Primary','description':'1080p · H.264','url':'https://media.test/a.m3u8?token='+generation,'behaviorHints':{'videoSize':2147483648,'videoHash':'0123456789abcdef','proxyHeaders':{'request':{'Referer':'https://fixture.test/','User-Agent':'FixturePlayer'}}}}
        second={'name':'Backup','url':'https://media.test/b.mp4?token='+generation,'behaviorHints':{'videoHash':'fedcba9876543210'}}
        if mode == 'window': first['url'] = STATE['media_url']; second['url'] = STATE['media_url'] + '?backup=1'
        streams=[first,second,{'infoHash':'a'*40,'name':'Torrent'},{'name':'Website','externalUrl':'https://media.test/page'},{'name':'DASH','url':'https://media.test/video.mpd'},{'name':'Unknown','url':'https://media.test/opaque?id=unknown'}]
        if mode == 'reordered': streams[0],streams[1] = streams[1],streams[0]
        if mode == 'missing': streams = [item for item in streams if item.get('name') != 'Primary']
        if mode == 'duplicate': streams.insert(1,dict(first))
        return self.send_json({'streams':streams})

    def do_POST(self):
        url = urlsplit(self.path)
        parts = url.path.strip('/').split('/')
        if len(parts) != 3 or parts[0] != 'cat' or parts[2] not in ('init', 'search', 'detail', 'play'):
            self.send_response(404); self.end_headers(); return
        try: body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        except (ValueError, TypeError): self.send_response(400); self.end_headers(); return
        owner, action = parts[1], parts[2]
        recorded = {'owner':owner,'action':action,'body':body,'header':self.headers.get('X-Fixture'),'query':parse_qs(url.query)}
        STATE['rest_requests'].append(recorded)
        if self.headers.get('X-Fixture') != 'allowed': self.send_response(403); self.end_headers(); return
        if action == 'init':
            ext = body.get('ext') or {}
            if ext.get('tenant') == 'FAIL': self.send_response(503); self.end_headers(); return
            context = {'owner':owner,'tenant':ext.get('tenant')}
            STATE['rest_contexts'][owner] = context
            if owner == 'shared': time.sleep(.05); return self.send_json({})
            return self.send_json({'context':context})
        if owner == 'shared':
            time.sleep(.05)
            if 'context' in body or owner not in STATE['rest_contexts']: self.send_response(403); self.end_headers(); return
            recorded['effective_tenant'] = STATE['rest_contexts'][owner]['tenant']
        elif body.get('context') != STATE['rest_contexts'].get(owner): self.send_response(403); self.end_headers(); return
        video={'vod_id':'201','vod_name':'Show','type_name':'电视剧','vod_year':'2020','vod_play_from':'rest-flag','vod_play_url':'S01E02$rest-2#S01E03$rest-3'}
        if action == 'search': return self.send_json({'list':[video] if body.get('wd') == 'Show' else []})
        if action == 'detail': return self.send_json({'list':[video] if body.get('id') == '201' else []})
        if body.get('id') != 'rest-2' or body.get('flag') != 'rest-flag': self.send_response(400); self.end_headers(); return
        suffix = '?tenant=' + STATE['rest_contexts'][owner]['tenant'] if owner == 'shared' else ''
        return self.send_json({'url':'https://media.test/'+owner+'.m3u8'+suffix,'header':{'Referer':'https://fixture.test/'+owner}})

    def send_json(self, data):
        payload = json.dumps(data, ensure_ascii=False).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_HEAD(self):
        if urlsplit(self.path).path == "/video.mp4": return self.media(head=True)
        self.send_response(404); self.end_headers()

    def media(self, head=False):
        with open("/video.mp4", "rb") as stream:
            data=stream.read()
        start,end=0,len(data)-1
        raw=self.headers.get("Range", "")
        if raw.startswith("bytes="):
            values=raw[6:].split("-",1)
            if values[0]: start=int(values[0])
            if len(values)>1 and values[1]: end=min(int(values[1]),end)
            if start>end or start>=len(data):
                self.send_response(416); self.end_headers(); return
        payload=data[start:end+1]
        self.send_response(206 if raw else 200)
        self.send_header("Content-Type","video/mp4")
        self.send_header("Accept-Ranges","bytes")
        self.send_header("Content-Length",str(len(payload)))
        if raw: self.send_header("Content-Range",f"bytes {start}-{end}/{len(data)}")
        self.end_headers()
        if not head: self.wfile.write(payload)

    def log_message(self, *_args):
        pass


ThreadingHTTPServer(("0.0.0.0", 80), Handler).serve_forever()
