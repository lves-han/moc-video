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
    def start(cls, token=None):
        if cls.container:
            docker("rm", "-f", cls.container)
        cls.container = cls.name
        args = ["run", "-d", "--name", cls.container, "--memory", str(SERVICE_LIMIT),
                "-p", "127.0.0.1::8787", "-v", cls.volume + ":/data"]
        if token is not None:
            args += ["-e", "MOC_SERVICE_TOKEN=" + token]
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", required=True, choices=["service-core"])
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(ServiceCore))
    case_ids = ["REQ-SERVICE-META/core-config-auth", "REQ-SERVICE-META/core-token-restart", "REQ-SERVICE-META/core-redaction-resources"]
    failures = {getattr(test, "_testMethodName", "setup"): message for test, message in result.failures + result.errors}
    suite = ET.Element("testsuite", tests=str(result.testsRun), failures=str(len(result.failures)), errors=str(len(result.errors)))
    for method, case_id in zip(("test_1_config_auth", "test_2_token_restart_and_env_priority", "test_3_redaction_and_resources"), case_ids):
        case = ET.SubElement(suite, "testcase", name=case_id)
        if method in failures:
            ET.SubElement(case, "failure").text = failures[method]
        elif "setup" in failures:
            ET.SubElement(case, "error").text = failures["setup"]
    args.junit.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(suite).write(args.junit, encoding="utf-8", xml_declaration=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps({"suite": args.suite, "tests": result.testsRun, "successful": result.wasSuccessful(),
                                       "host_peak_bytes": ServiceCore.host_peak, "build_memory_cap": BUILD_LIMIT,
                                       "service_memory_cap": SERVICE_LIMIT}, indent=2) + "\n")
    return 0 if result.wasSuccessful() and result.testsRun == 3 else 1


if __name__ == "__main__":
    raise SystemExit(main())
