import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlsplit


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        url = urlsplit(self.path)
        query = parse_qs(url.query)
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
        payload = json.dumps(data, ensure_ascii=False).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_args):
        pass


HTTPServer(("0.0.0.0", 80), Handler).serve_forever()
