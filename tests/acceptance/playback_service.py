"""T1.1 container acceptance; execute through scripts/run.py."""
import argparse
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
        cls.host_peak = 0
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
        self.assertLessEqual(self.host_peak + BUILD_LIMIT, 4096 * MIB)


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
               "-v", str(ROOT / "tests/fixtures/service/tmdb_fixture.py") + ":/fixture.py:ro",
               "-v", cls.volume + ":/service-data",
               "python:3.12-slim", "python", "-u", "/fixture.py")
        docker("exec", cls.fixture, "python", "-c", "import socket,time\nfor _ in range(100):\n try:\n  socket.create_connection(('127.0.0.1',80),timeout=1).close(); break\n except OSError: time.sleep(.1)\nelse: raise SystemExit('fixture did not start')")
        address = docker("inspect", "--format", "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}", cls.fixture)
        if not address:
            raise RuntimeError("fixture container has no address")
        cls.fixture_env = {"MOC_TMDB_API_URL": "http://" + address + "/3", "MOC_TMDB_KEY": "fixture-key"}

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
        self.assertLessEqual(self.host_peak + BUILD_LIMIT + SERVICE_LIMIT + 256 * MIB, 4096 * MIB)


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
        self.assertIn("2 passed", unit.stdout)
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", required=True, choices=["service-core", "tmdb-metadata", "metadata-cache-home"])
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    test_class = {"service-core":ServiceCore, "tmdb-metadata":TmdbMetadata, "metadata-cache-home":CacheHome}[args.suite]
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(test_class))
    case_ids = {"service-core":["REQ-SERVICE-META/core-config-auth", "REQ-SERVICE-META/core-token-restart", "REQ-SERVICE-META/core-redaction-resources"],
                "tmdb-metadata":["REQ-SERVICE-META/tmdb-routes-fields", "REQ-SERVICE-META/tmdb-identity-language", "REQ-SERVICE-META/tmdb-auth-errors"],
                "metadata-cache-home":["REQ-SERVICE-META/home-cold-start-daily", "REQ-SERVICE-META/cache-isolation-expiry", "REQ-SERVICE-META/home-failure-retains-snapshot"]}[args.suite]
    failures = {getattr(test, "_testMethodName", "setup"): message for test, message in result.failures + result.errors}
    suite = ET.Element("testsuite", tests=str(result.testsRun), failures=str(len(result.failures)), errors=str(len(result.errors)))
    methods = {"service-core":("test_1_config_auth", "test_2_token_restart_and_env_priority", "test_3_redaction_and_resources"),
               "tmdb-metadata":("test_1_routes_fields", "test_2_identity_language", "test_3_auth_errors"),
               "metadata-cache-home":("test_1_home_cold_start_daily", "test_2_cache_isolation_expiry", "test_3_home_failure_retains_snapshot")}[args.suite]
    for method, case_id in zip(methods, case_ids):
        case = ET.SubElement(suite, "testcase", name=case_id)
        if method in failures:
            ET.SubElement(case, "failure").text = failures[method]
        elif "setup" in failures:
            ET.SubElement(case, "error").text = failures["setup"]
    args.junit.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(suite).write(args.junit, encoding="utf-8", xml_declaration=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps({"suite": args.suite, "tests": result.testsRun, "successful": result.wasSuccessful(),
                                       "host_peak_bytes": test_class.host_peak, "build_memory_cap": BUILD_LIMIT,
                                       "service_memory_cap": SERVICE_LIMIT}, indent=2) + "\n")
    return 0 if result.wasSuccessful() and result.testsRun == 3 else 1


if __name__ == "__main__":
    raise SystemExit(main())
