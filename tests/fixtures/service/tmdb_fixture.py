import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, unquote, urlsplit
import time

STATE = {"fail": False, "fail_addon_b": False, "delay": 0.0, "media_url": "", "counts": {}}


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        url = urlsplit(self.path)
        query = parse_qs(url.query)
        if url.path == "/control":
            if "fail" in query: STATE["fail"] = query["fail"][0] == "1"
            if "fail_addon_b" in query: STATE["fail_addon_b"] = query["fail_addon_b"][0] == "1"
            if "delay" in query: STATE["delay"] = float(query["delay"][0])
            if "media_url" in query: STATE["media_url"] = query["media_url"][0]
            if "reset" in query: STATE["counts"].clear()
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
        if url.path.startswith(("/addon-a/", "/addon-b/")):
            return self.addon(url)
        if url.path == "/maccms":
            movie = {"vod_id":1,"vod_name":"Film","type_name":"电影","vod_year":"2020","vod_play_from":"fixture","vod_play_url":"Film$"+STATE["media_url"]}
            return self.send_json({"code":1,"page":1,"pagecount":1,"limit":20,"total":1,"list":[movie]})
        if url.path in ("/maccms-json", "/maccms-xml", "/maccms-invalid"):
            if self.headers.get("X-Fixture") != "allowed":
                self.send_response(403); self.end_headers(); return
            if "sig=" in url.query and "sig=a%2Fb+c" not in url.query:
                self.send_response(400); self.end_headers(); return
            if url.path == "/maccms-invalid": return self.send_json({"code":0,"message":"closed"})
            if url.path == "/maccms-json":
                return self.send_json({"code":1,"page":1,"pagecount":1,"list":[{"vod_id":101,"vod_name":"Film","type_name":"电影","vod_year":"2020","vod_play_from":"hls$$$direct","vod_play_url":"Episode 1$https://media.test/video?sig=a$b#Episode 2$https://media.test/2$$$https://media.test/direct"}]})
            data=b'<rss><list><video><id>101</id><name>Film</name><type>movie</type><year>2020</year><dl><dd flag="hls"><![CDATA[Episode 1$https://media.test/video?sig=a$b#Episode 2$https://media.test/2]]></dd><dd flag="direct"><![CDATA[https://media.test/direct]]></dd></dl></video></list></rss>'
            self.send_response(200); self.send_header("Content-Type","application/xml"); self.send_header("Content-Length",str(len(data))); self.end_headers(); self.wfile.write(data); return
        if url.path == "/video.mp4":
            return self.media()
        if query.get("api_key") != ["fixture-key"]:
            self.send_response(401)
            self.end_headers()
            return
        lang = query.get("language", ["en-US"])[0]
        if url.path == "/3/configuration/languages":
            data = [{"iso_639_1":"en", "english_name":"English"}]
        elif url.path.startswith("/3/genre/"):
            data = {"genres":[{"id":18, "name":"剧情" if lang == "zh-CN" else "Drama"}]}
        elif url.path.startswith(("/3/discover/", "/3/search/", "/3/trending/")) or url.path in ("/3/movie/now_playing", "/3/movie/upcoming", "/3/movie/top_rated", "/3/tv/on_the_air", "/3/tv/top_rated"):
            movie = "/movie" in url.path
            suffix = " P2" if query.get("page") == ["2"] and query.get("query") == ["film"] else ""
            data = {"results":[{"id":42, "title":("电影" if lang == "zh-CN" else "Film") + suffix, "name":"剧集" if lang == "zh-CN" else "Series", "release_date":"2020-01-01", "first_air_date":"2021-01-01", "genre_ids":[18], "overview":"fixture", "vote_average":8.0}], "page": int(query.get("page",[1])[0]), "fixture_movie":movie}
        elif url.path in ("/3/movie/42", "/3/movie/99", "/3/tv/42", "/3/tv/43"):
            movie = url.path.startswith("/3/movie")
            data = {"id":43 if url.path == "/3/tv/43" else 42,"title":"电影" if lang == "zh-CN" else "Film","name":"剧集" if lang == "zh-CN" else "Series", "original_title":"Original Film", "original_name":"Original Series", "release_date":"2020-01-01", "first_air_date":"2021-01-01", "status":"Returning Series", "external_ids":{"imdb_id":None if url.path == "/3/tv/43" else "tt1234567"}, "seasons":[] if movie else [{"season_number":1}], "alternative_titles":{"titles":[{"iso_3166_1":"CN", "title":"别名"},{"iso_3166_1":"US", "title":"Alias"}]}, "credits":{"cast":[],"crew":[]}, "genres":[], "production_countries":[], "runtime":90}
        elif url.path in ("/3/tv/42/season/1", "/3/tv/43/season/1"):
            data = {"episodes":[{"episode_number":1,"name":"第一集" if lang == "zh-CN" else "Episode 1","air_date":"2021-01-01"}]}
        else:
            self.send_response(404)
            self.end_headers()
            return
        self.send_json(data)

    def addon(self, url):
        parts = [unquote(part) for part in url.path.strip('/').split('/')]
        addon = parts[0]
        label = "Addon A" if addon == "addon-a" else "Addon B"
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


HTTPServer(("0.0.0.0", 80), Handler).serve_forever()
