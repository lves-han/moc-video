"""T1.1 container acceptance; execute through scripts/run.py."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
import uuid
import xml.etree.ElementTree as ET

import psutil

ROOT = Path(__file__).resolve().parents[2]
MIB = 1024 * 1024
BUILD_LIMIT = 2048 * MIB
SERVICE_LIMIT = 512 * MIB


def docker(*args, timeout=300):
    result = subprocess.run(["docker", *map(str, args)], cwd=ROOT, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"docker {args[0]} failed: {result.stderr[-2000:]}")
    return (result.stdout + result.stderr if args[0] == "logs" else result.stdout).strip()


class ServiceCore(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.name = "moc-t11-" + uuid.uuid4().hex[:12]
        cls.image = cls.name + ":test"
        cls.volume = cls.name
        cls.container = None
        cls.host_peak = psutil.Process(os.getppid()).memory_info().rss + psutil.Process().memory_info().rss
        cls.host_peak_build = cls.host_peak
        cls.host_peak_runtime = 0
        cls.in_build = True
        cls.other_owned_caps = 0
        ui_state = ROOT / "evidence/tmp/t15-window/resources.json"
        if ui_state.exists():
            resources = json.loads(ui_state.read_text())
            for name in (resources.get("service"), resources.get("fixture")):
                if not isinstance(name, str) or not name.startswith("moc-t15-"):
                    continue
                inspected = subprocess.run(["docker", "inspect", name], text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                if inspected.returncode == 0:
                    data = json.loads(inspected.stdout)[0]
                    if data["State"]["Running"]:
                        limit = data["HostConfig"]["Memory"]
                        if not limit: raise RuntimeError("owned UI fixture has no hard memory cap")
                        cls.other_owned_caps += limit
        cls.stop_sampling = threading.Event()
        cls.addClassCleanup(cls.cleanup)
        cls.sampler = threading.Thread(target=cls.sample, daemon=True)
        cls.sampler.start()
        # The build container itself has a hard limit. The host runner samples
        # its process tree separately; the Docker VM is accounted for by the cap.
        env = dict(os.environ, DOCKER_BUILDKIT="0")
        with tempfile.TemporaryDirectory(prefix="moc-t11-build-") as temporary:
            context = Path(temporary)
            for name in ("Cargo.toml", "Cargo.lock"):
                shutil.copy2(ROOT / name, context / name)
            shutil.copytree(ROOT / "crates/moc-core", context / "crates/moc-core")
            shutil.copytree(ROOT / "server", context / "server")
            result = subprocess.run(["docker", "build", "--memory", str(BUILD_LIMIT), "--tag", cls.image,
                                     "--file", "server/Dockerfile", str(context)], cwd=ROOT, env=env,
                                    text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1500)
        if result.returncode:
            raise RuntimeError("bounded container build failed: " + result.stdout[-3000:])
        cls.in_build = False
        docker("volume", "create", cls.volume)

    @classmethod
    def sample(cls):
        root = psutil.Process(os.getppid())
        while not cls.stop_sampling.wait(0.2):
            total = 0
            for proc in [root, *root.children(recursive=True)]:
                try:
                    total += proc.memory_info().rss
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass
            cls.host_peak = max(cls.host_peak, total)
            if cls.in_build:
                cls.host_peak_build = max(cls.host_peak_build, total)
            else:
                cls.host_peak_runtime = max(cls.host_peak_runtime, total)

    def assert_budget(self, fixture=False):
        self.assertGreater(self.host_peak_build, 0)
        self.assertGreater(self.host_peak_runtime, 0)
        self.assertLessEqual(self.host_peak_build + BUILD_LIMIT + self.other_owned_caps, 4096 * MIB)
        self.assertLessEqual(self.host_peak_runtime + SERVICE_LIMIT + (256 * MIB if fixture else 0) + self.other_owned_caps, 4096 * MIB)

    @classmethod
    def cleanup(cls):
        if cls.container:
            subprocess.run(["docker", "rm", "-f", cls.container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["docker", "volume", "rm", cls.volume], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["docker", "image", "rm", cls.image], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        cls.stop_sampling.set()
        cls.sampler.join(timeout=2)

    @classmethod
    def start(cls, token=None, extra_env=None):
        if cls.container:
            docker("rm", "-f", cls.container)
        cls.container = cls.name
        args = ["run", "-d", "--name", cls.container, "--memory", str(SERVICE_LIMIT),
                "-p", "127.0.0.1::8787", "-v", cls.volume + ":/data"]
        if token is not None:
            args += ["-e", "MOC_SERVICE_TOKEN=" + token]
        for key, value in (extra_env or {}).items():
            args += ["-e", key + "=" + value]
        docker(*args, cls.image)
        port = docker("port", cls.container, "8787/tcp").rsplit(":", 1)[1]
        cls.url = f"http://127.0.0.1:{port}/v1/health"
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                cls.request()
                return
            except (OSError, urllib.error.URLError):
                time.sleep(0.2)
        raise RuntimeError("service did not start")

    @classmethod
    def request(cls, token=None):
        headers = {} if token is None else {"Authorization": "Bearer " + token}
        try:
            response = urllib.request.urlopen(urllib.request.Request(cls.url, headers=headers), timeout=3)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, json.load(response)

    def test_1_config_auth(self):
        self.start("configured-test-token")
        status, error = self.request()
        self.assertEqual(status, 401)
        self.assertEqual(error["error"]["code"], "UNAUTHORIZED")
        self.assertEqual(self.request("incorrect")[0], 401)
        status, body = self.request("configured-test-token")
        self.assertEqual(status, 200)
        self.assertEqual(body["status"], "ok")
        docker("exec", self.container, "test", "-s", "/data/service.sqlite3")

    def test_2_token_restart_and_env_priority(self):
        self.start()
        token = docker("exec", self.container, "cat", "/data/service-token").strip()
        self.assertEqual(docker("exec", self.container, "stat", "-c", "%a", "/data/service-token"), "600")
        self.assertEqual(self.request(token)[0], 200)
        docker("restart", self.container)
        port = docker("port", self.container, "8787/tcp").rsplit(":", 1)[1]
        type(self).url = f"http://127.0.0.1:{port}/v1/health"
        for attempt in range(30):
            try:
                if self.request(token)[0] == 200:
                    break
            except (OSError, urllib.error.URLError):
                time.sleep(0.2)
        self.assertEqual(docker("exec", self.container, "cat", "/data/service-token").strip(), token)
        self.assertEqual(self.request(token)[0], 200)
        self.start("override-test-token")
        self.assertEqual(self.request(token)[0], 401)
        self.assertEqual(self.request("override-test-token")[0], 200)

    def test_3_redaction_and_resources(self):
        self.start("private-test-token")
        self.request("private-test-token")
        logs = docker("logs", self.container)
        self.assertNotIn("private-test-token", logs)
        self.assertNotIn("Authorization", logs)
        limit = int(docker("inspect", "--format", "{{.HostConfig.Memory}}", self.container))
        self.assertEqual(limit, SERVICE_LIMIT)
        self.assert_budget()


class TmdbMetadata(ServiceCore):
    test_1_config_auth = None
    test_2_token_restart_and_env_priority = None
    test_3_redaction_and_resources = None

    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.fixture = cls.name + "-fixture"
        cls.addClassCleanup(subprocess.run, ["docker", "rm", "-f", cls.fixture], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        docker("run", "-d", "--name", cls.fixture, "--memory", str(256 * MIB),
               "-p", "127.0.0.1::80",
               "-v", str(ROOT / "tests/fixtures/service/tmdb_fixture.py") + ":/fixture.py:ro",
               "-v", cls.volume + ":/service-data",
               "python:3.12-slim", "python", "-u", "/fixture.py")
        docker("exec", cls.fixture, "python", "-c", "import socket,time\nfor _ in range(100):\n try:\n  socket.create_connection(('127.0.0.1',80),timeout=1).close(); break\n except OSError: time.sleep(.1)\nelse: raise SystemExit('fixture did not start')")
        address = docker("inspect", "--format", "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}", cls.fixture)
        if not address:
            raise RuntimeError("fixture container has no address")
        cls.fixture_env = {"MOC_TMDB_API_URL": "http://" + address + "/3", "MOC_TMDB_KEY": "fixture-key"}
        port = docker("port", cls.fixture, "80/tcp").rsplit(":", 1)[1]
        cls.fixture_control = "http://127.0.0.1:" + port

    @classmethod
    def resource(cls, path, token="app-token"):
        request = urllib.request.Request(cls.url.rsplit("/v1/health", 1)[0] + "/stremio/" + token + "/tmdb/" + path)
        try:
            response = urllib.request.urlopen(request, timeout=20)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, json.load(response)

    def test_1_routes_fields(self):
        self.start("app-token", self.fixture_env)
        status, manifest = self.resource("zh-CN/manifest.json")
        self.assertEqual(status, 200, manifest)
        self.assertEqual((status, manifest["id"], len(manifest["catalogs"])), (200, "tmdb-addon", 22))
        catalog_ids = {(item["type"], item["id"]) for item in manifest["catalogs"]}
        self.assertTrue({("movie", "tmdb.now_playing"), ("movie", "tmdb.upcoming"), ("series", "tmdb.on_air"), ("series", "tmdb.variety")} <= catalog_ids)
        for attempt in range(200):
            if self.request("app-token")[1].get("snapshot_at") is not None:
                break
            time.sleep(0.1)
        for kind, catalog in (("movie", "tmdb.upcoming"), ("series", "tmdb.on_air"), ("series", "tmdb.variety")):
            self.assertEqual(self.resource(f"zh-CN/catalog/{kind}/{catalog}.json")[0], 200)
        status, catalog = self.resource("zh-CN/catalog/movie/tmdb.search/search=film&skip=20.json")
        self.assertEqual(status, 200)
        self.assertEqual((catalog["metas"][0]["id"], catalog["metas"][0]["name"]), ("tmdb:42", "电影 P2"))
        status, meta = self.resource("en-US/meta/movie/tmdb:42.json")
        self.assertEqual((status, meta["meta"]["original_title"], meta["meta"]["aliases"]), (200, "Original Film", ["Alias"]))

    def test_2_identity_language(self):
        self.start("app-token", self.fixture_env)
        status, meta = self.resource("zh-CN/meta/series/tmdb:42.json")
        self.assertEqual((status, meta["meta"]["type"], meta["meta"]["videos"][0]["id"]), (200, "series", "tt1234567:1:1"))
        self.assertEqual(meta["meta"]["aliases"], ["别名"])
        self.assertEqual(self.resource("en-US/meta/series/tmdb:42.json")[1]["meta"]["name"], "Series")
        self.assertEqual(self.resource("en-US/meta/series/tmdb:43.json")[1]["meta"]["videos"][0]["id"], "tmdb:43:1:1")
        self.assertEqual(self.resource("en-US/meta/movie/tmdb:99.json")[0], 502)
        self.assertEqual(self.resource("en-US/meta/unknown/tmdb:42.json")[0], 400)
        self.assertEqual(self.resource("en-US/meta/movie/tmdb:wrong.json")[0], 400)

    def test_3_auth_errors(self):
        self.start("app-token", {"MOC_TMDB_API_URL": self.fixture_env["MOC_TMDB_API_URL"]})
        status, body = self.resource("en-US/manifest.json")
        self.assertEqual((status, body["error"]["code"]), (401, "UNAUTHORIZED"))
        self.start("app-token", {**self.fixture_env, "MOC_TMDB_KEY": "invalid-test-key"})
        status, body = self.resource("en-US/manifest.json")
        self.assertEqual((status, body["error"]["code"]), (401, "UNAUTHORIZED"))
        self.assertNotIn("invalid-test-key", json.dumps(body))
        self.assertNotIn("invalid-test-key", docker("logs", self.container))
        self.assertEqual(self.resource("en-US/manifest.json", token="wrong")[0], 401)
        self.assertEqual(int(docker("inspect", "--format", "{{.HostConfig.Memory}}", self.fixture)), 256 * MIB)
        self.assert_budget(fixture=True)


class CacheHome(TmdbMetadata):
    test_1_routes_fields = None
    test_2_identity_language = None
    test_3_auth_errors = None

    @classmethod
    def fixture_request(cls, path):
        program = "import urllib.request; print(urllib.request.urlopen('http://127.0.0.1" + path + "').read().decode())"
        return json.loads(docker("exec", cls.fixture, "python", "-c", program))

    @classmethod
    def sql(cls, query):
        program = "import sqlite3; c=sqlite3.connect('/service-data/service.sqlite3'); c.execute(" + repr(query) + "); c.commit()"
        docker("exec", cls.fixture, "python", "-c", program)

    def test_1_home_cold_start_daily(self):
        unit = subprocess.run(["cargo", "test", "--locked", "--offline", "-p", "moc-service", "metadata::", "--", "--test-threads=1"], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=600)
        self.assertEqual(unit.returncode, 0, unit.stdout[-2000:])
        self.assertIn("daily_cutoff_uses_configured_timezone ... ok", unit.stdout)
        self.assertIn("cache_keys_separate_sources_types_and_languages ... ok", unit.stdout)
        self.fixture_request("/control?delay=0.02")
        self.start("app-token", {**self.fixture_env, "MOC_HOME_POLL_SECONDS": "1"})
        status, body = self.resource("zh-CN/catalog/movie/tmdb.top.json")
        self.assertEqual((status, body["error"]["code"], body["error"]["retryable"]), (503, "ENGINE_NOT_READY", True))
        self.assertEqual(self.resource("zh-CN/catalog/movie/tmdb.search/search=film.json")[0], 200)
        self.assertEqual(self.resource("zh-CN/meta/movie/tmdb:42.json")[0], 200)
        for attempt in range(300):
            if self.request("app-token")[1].get("snapshot_at") is not None: break
            time.sleep(0.1)
        else: self.fail("first home snapshot did not finish")
        self.fixture_request("/control?delay=0")
        self.assertEqual(self.resource("zh-CN/catalog/movie/tmdb.top.json")[0], 200)
        self.start("app-token", {**self.fixture_env, "MOC_HOME_POLL_SECONDS": "1"})
        self.assertIsNotNone(self.request("app-token")[1]["snapshot_at"])
        self.assertEqual(self.resource("zh-CN/catalog/movie/tmdb.top.json")[0], 200)

    def test_2_cache_isolation_expiry(self):
        self.sql("UPDATE metadata_cache SET expires_at=0")
        self.fixture_request("/control?reset=1")
        for path in ("zh-CN/catalog/movie/tmdb.search/search=film.json",)*2:
            self.assertEqual(self.resource(path)[0], 200)
        self.assertEqual(self.fixture_request("/stats")["counts"].get("/3/search/movie",0), 1)
        self.assertEqual(self.resource("en-US/catalog/movie/tmdb.search/search=film.json")[0], 200)
        self.assertEqual(self.resource("en-US/catalog/series/tmdb.search/search=film.json")[0], 200)
        self.assertEqual(self.fixture_request("/stats")["counts"]["/3/search/movie"], 2)
        self.assertEqual(self.fixture_request("/stats")["counts"]["/3/search/tv"], 1)
        self.sql("UPDATE metadata_cache SET expires_at=0")
        self.assertEqual(self.resource("zh-CN/catalog/movie/tmdb.search/search=film.json")[0], 200)
        self.assertEqual(self.fixture_request("/stats")["counts"]["/3/search/movie"], 3)

    def test_3_home_failure_retains_snapshot(self):
        before = self.resource("zh-CN/catalog/movie/tmdb.top.json")[1]
        self.fixture_request("/control?fail=1")
        self.sql("UPDATE home_snapshots SET built_at=0")
        self.start("app-token", {**self.fixture_env, "MOC_HOME_POLL_SECONDS": "1"})
        for attempt in range(100):
            if self.request("app-token")[1].get("snapshot_error"): break
            time.sleep(0.1)
        else: self.fail("home refresh failure was not reported")
        self.assertEqual(self.resource("zh-CN/catalog/movie/tmdb.top.json"), (200, before))
        self.assertEqual(self.request("app-token")[1]["snapshot_at"], 0)


class StremioMetadata(TmdbMetadata):
    test_1_routes_fields = None
    test_2_identity_language = None
    test_3_auth_errors = None

    @classmethod
    def native(cls, method, path, body=None):
        url = cls.url.rsplit("/v1/health", 1)[0] + path
        headers = {"Authorization":"Bearer app-token"}
        if body is not None: headers["Content-Type"] = "application/json"
        request = urllib.request.Request(url, data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
        try: response = urllib.request.urlopen(request, timeout=20)
        except urllib.error.HTTPError as error: response = error
        with response:
            content = response.read()
            return response.status, json.loads(content) if content else None

    @classmethod
    def proxy(cls, source, path):
        url = cls.url.rsplit("/v1/health", 1)[0] + f"/stremio/app-token/{source}/{path}"
        try: response = urllib.request.urlopen(url, timeout=20)
        except urllib.error.HTTPError as error: response = error
        with response: return response.status, json.load(response)

    @classmethod
    def group(cls):
        base = cls.fixture_env["MOC_TMDB_API_URL"].removesuffix("/3")
        return {"config_id":"original-config", "addons":[
            {"addon_key":"first-key","manifest_url":base+"/addon-a/manifest.json","order":0,"namespace_rule":{"kind":"addon"}},
            {"addon_key":"second-key","manifest_url":base+"/addon-b/manifest.json","order":1,"namespace_rule":{"kind":"verified-tmdb"}}]}

    def test_1_manifest_proxy(self):
        unit = subprocess.run(["cargo", "test", "--locked", "--offline", "-p", "moc-service", "metadata::", "--", "--test-threads=1"], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=600)
        self.assertEqual(unit.returncode, 0, unit.stdout[-2000:])
        self.assertIn("redirects_protect_credentials_and_tls ... ok", unit.stdout)
        self.assertIn("recognized_tmdb_language_paths_are_localized ... ok", unit.stdout)
        self.start("app-token", self.fixture_env)
        status, result = self.native("POST", "/v1/metadata-sources/import", self.group())
        self.assertEqual(status, 200)
        self.assertEqual([r["status"] for r in result["results"]], ["ok", "ok"])
        sources = [r["source"] for r in result["results"]]
        type(self).source_ids = [source["metadata_source_id"] for source in sources]
        self.assertNotEqual(*self.source_ids)
        for source, label in zip(self.source_ids, ["Addon A", "Addon B"]):
            self.assertEqual(self.proxy(source, "en-US/manifest.json")[1]["name"], label)
            status, catalog = self.proxy(source, "en-US/catalog/movie/catalog/search=film&skip=20.json")
            self.assertEqual((status, catalog["metas"][0]["name"], catalog["metas"][0]["skip"]), (200, label, ["20"]))
            self.assertEqual(self.proxy(source, "en-US/meta/movie/same:42.json")[1]["meta"]["name"], label)

    def test_2_source_identity_import(self):
        status, result = self.native("POST", "/v1/metadata-sources", self.group())
        sources = [r["source"] for r in result["results"]]
        self.assertEqual([s["metadata_source_id"] for s in sources], self.source_ids)
        self.assertEqual([s["revision"] for s in sources], [1, 1])
        self.start("app-token", self.fixture_env)
        rows = self.native("GET", "/v1/metadata-sources")[1]["sources"]
        self.assertEqual([s["metadata_source_id"] for s in rows], self.source_ids)
        self.assertEqual([s["origin_order"] for s in rows], [0, 1])
        self.assertEqual([s["origin_namespace_rule"]["kind"] for s in rows], ["addon", "verified-tmdb"])

    def test_3_capabilities_fallback(self):
        source = self.source_ids[0]
        self.assertEqual(self.proxy(source, "zh-CN/catalog/movie/catalog.json")[0], 400)
        self.assertEqual(self.proxy(source, "zh-CN/meta/series/same:42.json")[0], 501)
        self.assertEqual(self.proxy(source, "zh-CN/meta/movie/other:42.json")[0], 501)
        status, body = self.proxy(source, "zh-CN/meta/movie/same:part%2F42.json")
        self.assertEqual((status, body["meta"]["id"]), (200, "same:part/42"))
        self.assertEqual(self.native("GET", "/v1/metadata-sources")[1]["sources"][0]["language_status"], "original")

    def test_4_multi_manifest_import(self):
        group = self.group(); group["config_id"] = "partial-config"
        group["addons"][1]["manifest_url"] = group["addons"][1]["manifest_url"].replace("/addon-b/", "/missing/")
        status, result = self.native("POST", "/v1/metadata-sources/import", group)
        self.assertEqual((status, [r["status"] for r in result["results"]]), (200, ["ok", "failed"]))
        first = result["results"][0]["source"]["metadata_source_id"]
        self.assertEqual(self.native("POST", "/v1/metadata-sources/import", group)[1]["results"][0]["source"]["metadata_source_id"], first)
        self.assertEqual(self.native("PUT", "/v1/metadata-sources/"+first, group["addons"][0])[0], 200)
        self.assertEqual(self.native("DELETE", "/v1/metadata-sources/"+first)[0], 204)
        self.assertEqual(self.proxy(first, "en-US/manifest.json")[0], 404)
        self.assertEqual(self.resource("en-US/manifest.json")[0], 200)


class AppMetadata(TmdbMetadata):
    test_1_routes_fields = None
    test_2_identity_language = None
    test_3_auth_errors = None

    def test_1_service_transport(self):
        result = subprocess.run(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "t15_redirect_refuses_cross_origin_token", "--", "--test-threads=1"], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1200)
        self.assertEqual(result.returncode, 0, result.stdout[-3000:])
        self.assertIn("1 passed", result.stdout)

    def test_2_identity_import(self):
        self.start("app-token", self.fixture_env)
        endpoint = self.url.rsplit("/v1/health",1)[0]
        environment = dict(os.environ, T15_SERVICE_URL=endpoint, T15_ADDON_BASE=self.fixture_env["MOC_TMDB_API_URL"].removesuffix("/3"), T15_CONTROL_URL=self.fixture_control, T15_CONTAINER=self.container)
        result = subprocess.run(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "t15_service_host_group_migration_and_records", "--", "--ignored", "--nocapture", "--test-threads=1"], cwd=ROOT, env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1200)
        type(self).host_output = result.stdout
        self.assertEqual(result.returncode, 0, result.stdout[-4000:])
        self.assertIn("T15_SERVICE_HOST:partial,retry,search,identity,history,watchlist,restart", result.stdout)

    def test_3_service_errors(self):
        self.assertNotIn("app-token", docker("logs", self.container))
        self.assert_budget(fixture=True)

    def test_4_partial_import(self):
        self.assertIn("T15_SERVICE_HOST:partial,retry,search,identity,history,watchlist,restart", getattr(type(self), "host_output", ""))
        stats = json.loads(docker("exec", self.fixture, "python", "-c", "import urllib.request; print(urllib.request.urlopen('http://127.0.0.1/stats').read().decode())"))["counts"]
        self.assertTrue(any(path.startswith("/addon-a/catalog/movie/catalog/") for path in stats))
        self.assertTrue(any(path.startswith("/addon-b/catalog/movie/catalog/") for path in stats))


class AppMetadataApp(unittest.TestCase):
    host_peak = host_peak_build = host_peak_runtime = other_owned_caps = 0

    @classmethod
    def setUpClass(cls):
        cls.observation = json.loads((ROOT / "evidence/manual/T1.5-service-window.json").read_text())
        paths = ["src", "src-tauri/src", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "src-tauri/build.rs", "src-tauri/capabilities", "protocol/app.schema.json", "crates/moc-core/src", "server/src", "server/Cargo.toml", "server/Dockerfile", "Cargo.lock", "tests/fixtures/service/tmdb_fixture.py"]
        rows=[]
        for name in paths:
            path = ROOT / name
            for file in ([path] if path.is_file() else path.rglob("*")):
                if file.is_file(): rows.append((str(file.relative_to(ROOT)), hashlib.sha256(file.read_bytes()).hexdigest()))
        digest = hashlib.sha256(json.dumps(sorted(rows), separators=(",", ":")).encode()).hexdigest()
        if digest != cls.observation["product_hash"]: raise AssertionError("Window observation was made on different product inputs")
        cls.host_peak = cls.observation["resource_sample"]["host_including_all_visible_webkit_bytes"]

    def test_1_metadata_browse(self):
        result = self.observation["observed"]
        self.assertEqual(result["addon_details"], ["Addon A", "Addon B"])
        self.assertTrue(result["search_addon_a_and_b"] and result["fixture_addon_search_requests"] >= 2)
        self.assertEqual((result["tmdb_english_title"], result["tmdb_chinese_title"]), ("Film", "电影"))
        self.assertTrue(result["tmdb_search_chinese"] and result["font_startup_fixed"])
        sample = self.observation["resource_sample"]
        self.assertLessEqual(sample["combined_bytes"], sample["limit_bytes"])
        self.assertEqual(sample["sample_count"], 1)

    def test_2_existing_records_resume(self):
        result = self.observation["observed"]
        self.assertIn("Film", result["history_titles"])
        self.assertEqual(result["initial_position_ms"], 7000)
        self.assertEqual(result["playback_observed_seconds"], [7, 10])
        self.assertTrue(result["playback_video_visible"] and result["legacy_tmdb_without_service"] and result["service_error_and_retry"])

    def test_3_multi_manifest_records(self):
        result = self.observation["observed"]
        self.assertEqual(set(result["watchlist_titles"]), {"Addon A", "Addon B"})
        self.assertTrue({"Addon A", "Addon B"} <= set(result["history_titles"]))
        self.assertEqual(result["same_raw_id"], "same:42")
        self.assertTrue(result["endpoint_and_token_changed"] and result["reconnect_ready_and_both_favorites_open"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", required=True, choices=["service-core", "tmdb-metadata", "metadata-cache-home", "stremio-metadata", "app-metadata", "app-metadata-app"])
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    test_class = {"service-core":ServiceCore, "tmdb-metadata":TmdbMetadata, "metadata-cache-home":CacheHome, "stremio-metadata":StremioMetadata, "app-metadata":AppMetadata, "app-metadata-app":AppMetadataApp}[args.suite]
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(test_class))
    case_ids = {"service-core":["REQ-SERVICE-META/core-config-auth", "REQ-SERVICE-META/core-token-restart", "REQ-SERVICE-META/core-redaction-resources"],
                "tmdb-metadata":["REQ-SERVICE-META/tmdb-routes-fields", "REQ-SERVICE-META/tmdb-identity-language", "REQ-SERVICE-META/tmdb-auth-errors"],
                "metadata-cache-home":["REQ-SERVICE-META/home-cold-start-daily", "REQ-SERVICE-META/cache-isolation-expiry", "REQ-SERVICE-META/home-failure-retains-snapshot"],
                "stremio-metadata":["REQ-SERVICE-META/metadata-manifest-proxy", "REQ-SERVICE-META/metadata-source-identity-import", "REQ-SERVICE-META/metadata-capabilities-fallback", "REQ-SERVICE-META/metadata-multi-manifest-import"],
                "app-metadata":["REQ-SERVICE-META/app-service-transport", "REQ-SERVICE-META/app-metadata-identity-import", "REQ-SERVICE-META/app-service-errors", "REQ-SERVICE-META/app-multi-manifest-partial-import"],
                "app-metadata-app":["REQ-SERVICE-META/app-metadata-browse", "REQ-SERVICE-META/app-existing-records-resume", "REQ-SERVICE-META/app-multi-manifest-records"]}[args.suite]
    failures = {getattr(test, "_testMethodName", "setup"): message for test, message in result.failures + result.errors}
    suite = ET.Element("testsuite", tests=str(result.testsRun), failures=str(len(result.failures)), errors=str(len(result.errors)))
    methods = {"service-core":("test_1_config_auth", "test_2_token_restart_and_env_priority", "test_3_redaction_and_resources"),
               "tmdb-metadata":("test_1_routes_fields", "test_2_identity_language", "test_3_auth_errors"),
               "metadata-cache-home":("test_1_home_cold_start_daily", "test_2_cache_isolation_expiry", "test_3_home_failure_retains_snapshot"),
               "stremio-metadata":("test_1_manifest_proxy", "test_2_source_identity_import", "test_3_capabilities_fallback", "test_4_multi_manifest_import"),
               "app-metadata":("test_1_service_transport", "test_2_identity_import", "test_3_service_errors", "test_4_partial_import"),
               "app-metadata-app":("test_1_metadata_browse", "test_2_existing_records_resume", "test_3_multi_manifest_records")}[args.suite]
    for method, case_id in zip(methods, case_ids):
        case = ET.SubElement(suite, "testcase", name=case_id)
        if method in failures:
            ET.SubElement(case, "failure").text = failures[method]
        elif "setup" in failures:
            ET.SubElement(case, "error").text = failures["setup"]
    args.junit.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(suite).write(args.junit, encoding="utf-8", xml_declaration=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    resources = (test_class.observation["resource_sample"] if args.suite == "app-metadata-app" else
                 {"host_peak_bytes": test_class.host_peak, "host_peak_build_bytes": test_class.host_peak_build,
                  "host_peak_runtime_bytes": test_class.host_peak_runtime, "build_memory_cap": BUILD_LIMIT,
                  "service_memory_cap": SERVICE_LIMIT, "other_owned_container_caps": test_class.other_owned_caps})
    args.report.write_text(json.dumps({"suite": args.suite, "tests": result.testsRun, "successful": result.wasSuccessful(),
                                       "resources": resources}, indent=2) + "\n")
    return 0 if result.wasSuccessful() and result.testsRun == len(case_ids) else 1


if __name__ == "__main__":
    raise SystemExit(main())
