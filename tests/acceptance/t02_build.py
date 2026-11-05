"""Fetches the pinned inputs for the Android engine and builds it; shared by the T02 acceptance."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request

from t00 import require

ROOT = Path(__file__).resolve().parents[2]
ENGINE = ROOT / "android-engine"
UPSTREAM, TOOLS, PACKAGES = ENGINE / "upstream", ENGINE / ".tools", ENGINE / "packages"
COMMIT = "a7ac454af8fbd64574c96847a936905855911720"
SDK = Path.home() / "Library/Android/sdk"
JDK_API = "https://api.adoptium.net/v3/assets/latest/17/hotspot?architecture=aarch64&image_type=jdk&os=mac&vendor=eclipse"
APP_ID = "com.lagradost.cloudstream3.engine"


def opened(url, timeout):
    # Adoptium rejects urllib's default agent.
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "yingxiang-video-engine-build"}), timeout=timeout)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory():
    return json.loads((ROOT / "replica-spec/references/plugin-artifact-inventory.json").read_text())


def fetch_packages():
    """Downloads each original package and rejects any bytes outside the locked inventory."""
    data = inventory()
    PACKAGES.mkdir(parents=True, exist_ok=True)
    rows = []
    for plugin in data["plugins"]:
        name = plugin.get("internalName") or plugin.get("internal_name") or plugin["name"]
        target = PACKAGES / f"{name}.cs3"
        if not target.exists() or sha256(target) != plugin["sha256"]:
            url = plugin.get("pinned_url") or plugin["url"]
            with urllib.request.urlopen(url, timeout=60) as response:
                target.write_bytes(response.read())
        require(sha256(target) == plugin["sha256"], f"{name}: package bytes differ from the locked inventory")
        rows.append(dict(internalName=name, sha256=plugin["sha256"], bytes=target.stat().st_size))
    require(len(rows) == 7, "All seven original packages are required")
    return dict(registry_sha256=data["registry_sha256"], registry_commit=data.get("registry_commit"), plugins=rows)


def fetch_jdk():
    home = TOOLS / "jdk"
    java = next(home.glob("*/Contents/Home/bin/java"), None) if home.exists() else None
    if java is None:
        with opened(JDK_API, 60) as response:
            package = json.load(response)[0]["binary"]["package"]
        TOOLS.mkdir(parents=True, exist_ok=True)
        archive = TOOLS / package["name"]
        with opened(package["link"], 600) as response, archive.open("wb") as output:
            shutil.copyfileobj(response, output)
        require(sha256(archive) == package["checksum"], "JDK archive checksum differs")
        home.mkdir(exist_ok=True)
        with tarfile.open(archive) as bundle:
            bundle.extractall(home, filter="tar")
        archive.unlink()
        java = next(home.glob("*/Contents/Home/bin/java"))
    return java.parents[1]


def prepare_upstream():
    """Pinned source plus the committed patch and overlay; nothing else differs from upstream."""
    if not (UPSTREAM / ".git").exists():
        UPSTREAM.mkdir(parents=True, exist_ok=True)
        for args in (["init", "-q"], ["remote", "add", "origin", "https://github.com/recloudstream/cloudstream"], ["fetch", "-q", "--depth", "1", "origin", COMMIT], ["checkout", "-q", "FETCH_HEAD"]):
            subprocess.run(["git", *args], cwd=UPSTREAM, check=True, timeout=300)
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=UPSTREAM, check=True, capture_output=True, text=True).stdout.strip()
    require(head == COMMIT, "Upstream commit differs")
    subprocess.run(["git", "checkout", "-q", "--", "."], cwd=UPSTREAM, check=True, timeout=60)
    subprocess.run(["git", "apply", str(ENGINE / "engine.patch")], cwd=UPSTREAM, check=True, timeout=60)
    shutil.copytree(ENGINE / "overlay/app/src", UPSTREAM / "app/src", dirs_exist_ok=True)


def bundle_packages(locked):
    assets = UPSTREAM / "app/src/engine/assets/engine"
    assets.mkdir(parents=True, exist_ok=True)
    for row in locked["plugins"]:
        shutil.copyfile(PACKAGES / f"{row['internalName']}.cs3", assets / f"{row['internalName']}.cs3")
    (assets / "inventory.json").write_text(json.dumps(locked))


def gradle_command(java_home, *tasks):
    (UPSTREAM / "local.properties").write_text(f"sdk.dir={SDK}\n")
    return ["./gradlew", "--no-daemon", "--console=plain", "--max-workers=2", "-Dorg.gradle.jvmargs=-Xmx2400m -Dfile.encoding=UTF-8", "-Pkotlin.compiler.execution.strategy=in-process",
            "-Porg.gradle.java.installations.auto-download=false", f"-Porg.gradle.java.installations.paths={java_home}", *tasks]


def gradle_environment(java_home):
    return dict(os.environ, JAVA_HOME=str(java_home), GRADLE_USER_HOME=str(TOOLS / "gradle"), ANDROID_HOME=str(SDK), PATH=f"{java_home}/bin:{os.environ['PATH']}")
