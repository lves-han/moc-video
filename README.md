# 墨客视频

Vue 3 / TypeScript / Tauri 2 的个人使用视频应用。服务端统一管理元数据和播放来源，App 提供浏览、手动选线与播放；Video.js 提供播放内核，SQLite 保存观看历史和待播清单。桌面界面支持本地视频、中英文、外观和播放设置。

当前发行配置为 **0.1.0（macOS build 2）**，目标 **macOS 14+ arm64**。最终包与干净目录复现以 T25-all 报告为准，本机隔离安装生命周期以 T25-lifecycle 报告为准；材料尚缺时对应状态为 NOT RUN。完整状态、路径与限制见 [T25 交付索引](docs/release-evidence/T25.md)、[进度](docs/PROGRESS.md) 和 [安装操作](packaging/T23.md)。当前仅个人使用；公开发行等待单独授权、Developer ID 签名、公证与第三方权利确认。

## 从干净源码目录开始

解压交付索引中“最终源码 ZIP”指向的源码包，或使用当前仓库工作树的完整源码。源码归档保留实际文件权限和逐文件摘要；依赖缓存、构建产物及历史 evidence 分别保存。安装包、上代升级测试包、报告和历史原始材料从交付索引取得。

准备 Python 3.12、Node 26.11.1 / npm 11.20.0、Rust 1.98.1、Tauri CLI 2.9.6、Xcode/macOS SDK。这些版本来自实际开发机观察；应用运行只需目标 macOS 提供的 WebKit 和系统媒体解码。Rust crate、npm 包的精确版本与许可来自 [版本锁](docs/runtime.lock.json)、`package-lock.json`、`src-tauri/Cargo.lock` 和 [第三方许可](licenses/THIRD_PARTY_NOTICES.txt)。

所有以下命令均为 **argv 数组**，在源码根目录执行，保持 `shell=false`。`{python}` 替换为该目录 `.venv/bin/python` 的绝对路径；原开发工作区是 `/Users/eason/jp/application`。首次用已安装的 Python 3.12 解释器执行 `[python绝对路径, "-m", "venv", ".venv"]`，随后执行：

```json
["{python}", "-m", "pip", "install", "-r", "scripts/requirements.txt"]
["cargo", "install", "tauri-cli", "--version", "2.9.6", "--locked"]
["npm", "ci", "--no-audit", "--no-fund"]
["cargo", "fetch", "--locked", "--target", "aarch64-apple-darwin", "--manifest-path", "src-tauri/Cargo.toml"]
["{python}", "-I", "src-tauri/ffmpeg/build_ffmpeg.py"]
```

首次准备依赖需要可访问相应包仓库和 FFmpeg 源码仓库。FFmpeg 脚本检出固定版本并生成构建所需的动态库；已有源码检出须处于脚本指定的提交。具备本地 npm/Cargo 缓存及已构建的 FFmpeg 时，以下 T25 复现命令直接离线安装并构建。先核对 `xcode-select -p` 与 `xcrun --show-sdk-version`；SDK 查询失败时保留错误并修复开发环境后继续。

T25 会原样读取并顺序执行下面的 `t25-reproduce` JSON。复现进程继承原资源执行器，工作目录为新解压源码；环境为 `CARGO_BUILD_JOBS=2`、`RUST_TEST_THREADS=1`、`ROLLDOWN_WORKER_THREADS=2`、`RAYON_NUM_THREADS=2`、`CARGO_TARGET_DIR={source}/target`，`RUSTFLAGS=--remap-path-prefix={home}=/build-user`。`{source}`、`{home}` 都以实际绝对路径替换。全新机器先完成上面的联网依赖准备。

<!-- t25-reproduce -->
```json
[
  {"id":"npm-init","argv":["npm","ci","--offline","--no-audit","--no-fund"],"timeout":900},
  {"id":"cargo-init","argv":["cargo","fetch","--locked","--offline","--target","aarch64-apple-darwin","--manifest-path","src-tauri/Cargo.toml"],"timeout":300},
  {"id":"frontend-build","argv":["npm","run","build"],"timeout":600},
  {"id":"native-build","argv":["cargo","tauri","build","--ci","--config","{\"build\":{\"devUrl\":null}}","--bundles","app","--no-sign","--","--locked","--offline"],"timeout":3600},
  {"id":"native-smoke","argv":["cargo","test","--locked","--offline","--manifest-path","src-tauri/Cargo.toml","--test","t25_delivery","--","--test-threads=1"],"timeout":900}
]
```

`native-smoke` 在专用临时数据目录执行真实 Rust Host 初始化、网关 ready、设置保存/重启和失效资源诊断。实际运行结果写入本卡报告。GUI 安装、初始化、画面和用户声音观察由 T25-lifecycle 单独记录。

Windows x64 使用 MSVC、LLVM/libclang 和 WebView2。通过 `scripts/run.py` 执行同一 FFmpeg 准备脚本会下载固定 SHA256 的 LGPLv3 共享 SDK；构建前将 `src-tauri/ffmpeg/dist/bin` 加入 PATH，并将 `LIBCLANG_PATH` 指向 LLVM 的 bin 目录。Windows 的项目解释器为 `.venv/Scripts/python.exe`，打包附加 `--config src-tauri/tauri.windows.conf.json`。Linux 安装 Tauri/WebKitGTK 构建依赖与 nasm，执行同一脚本编译固定的精简 LGPL 源码；打包附加 `--config src-tauri/tauri.linux.conf.json`。macOS 沿用现有源码构建及 Frameworks 布局。Windows 共享库来源和许可证见 `licenses/ffmpeg-windows/`。

## 运行和初始化

构建后用 Finder 打开 `target/release/bundle/macos/墨客视频.app`，或执行 `["/usr/bin/open", "target/release/bundle/macos/墨客视频.app"]`。开发时先执行 `["npm", "run", "dev"]`，再在另一终端执行 `["cargo", "tauri", "dev"]`；前端地址为 `http://127.0.0.1:5173/`。浏览器预览用于前端开发，桌面业务通过 Tauri IPC 调用 Rust。

首次启动后在设置中填写服务地址和访问令牌。元数据、扩展和播放源通过服务端 API 管理；点击影片后显示各来源状态、清晰度和大小，由用户选择线路。服务端将来源信息缓存一天，选中后才下发播放地址。本地视频可通过侧栏打开，历史与待播清单保存在应用数据目录。

应用数据位置：`~/Library/Application Support/app.moc.video/`，包含 `settings.json`、`moc-video.db` 和 `media-cache/`。WebKit 的网站存储另保留线路偏好；系统按应用标识管理。卸载 app 保留用户数据；显式清理按 [生命周期操作](packaging/T23.md) 仅删除本次可识别测试数据，原用户数据保留。

## 诊断和测试

来源页的 **Diagnostics / 诊断 → Copy redacted diagnostics / 复制脱敏诊断** 提供平台、宿主阶段、契约、来源登记状态、错误码与请求关联 ID。播放错误显示阶段与重试/换线入口。提交诊断时保留错误码和时间，敏感 URL、令牌及账户信息先脱敏。公共 DNS 使用 `1.1.1.1:53` 和 `8.8.8.8:53`；两者失败时显示 DNS 错误。来源及媒体允许内网、回环和代理 Fake-IP，已按用户要求取消 IP 范围及实际连接 IP 校验；支持系统 TUN 透明代理。站点访问控制、证书错误和媒体下架保留实际失败原因。

开发工作区的构建、测试及服务通过原 `scripts/run.py` 执行，使用同一项目解释器和实际全局重锁，保持 4096MiB RSS 采样、并行 2、测试线程 1。macOS 实际后端为 `process-watchdog`，允许瞬时超限。可按修改范围选择直接相关检查：

```json
["{python}","-B","scripts/run.py","diag-t25-oracles","--","{python}","-B","-m","unittest","discover","-s","tests/acceptance","-p","test_t23.py","-k","test_upgrade_accepts_only_the_documented_migration_differences","-k","test_removal_losing_user_data_or_leaving_processes_fails","-v"]
["{python}","-B","scripts/run.py","diag-t25-delivery-oracles","--","{python}","-B","-m","unittest","discover","-s","tests/acceptance","-p","test_t25.py","-v"]
["{python}","-B","scripts/run.py","diag-t23-release-build","--","{python}","-B","packaging/release.py","--build"]
["{python}","-B","scripts/run.py","A1","--","{python}","-B","tests/acceptance/t25.py","--suite","all","--report","evidence/acceptance/T25-all.json","--junit","evidence/acceptance/T25-all.xml"]
["{python}","-B","scripts/run.py","A2","--","{python}","-B","tests/acceptance/t25_local.py","--report","evidence/acceptance/T25-lifecycle.json","--junit","evidence/acceptance/T25-lifecycle.xml"]
```

A2 使用用户授权的本机隔离测试：核对当前安装包、独立测试数据的升级与卸载、用户声音确认以及原有应用和数据的恢复结果。复现、测试和构建期间保持源码稳定。失败保留原报告及收据，先记录原因再恢复。当前流程按卡选择必要检查（ADR-062）。

## 发布状态与限制

- ZIP/DMG 的准确路径、大小和 SHA256 来自 `evidence/releases/T23-current.json` 指向的不可变 manifest。当前最终包必须归属 T0.36；历史 T0.27 包保留原身份。安装包以完整 ad-hoc 资源签名密封，Developer ID、公证和 Gatekeeper 接受均为待办；默认系统安全设置与 quarantine 保留，拦截则记录实际结果。
- T0.35 的长播与固定构建证据属于提交 `e6c9b946636adb29afe1bc849f536d2244b1dc38`，音频采样按用户决定跳过。T0.37 的 [线路记忆报告](evidence/acceptance/LINE-PREFERENCE.json) 覆盖后续源码的线路、播放、切集和历史相关回归。T25 报告分别列出历史证据与本次实跑范围。
- Rust 来源支持清单见 [兼容性记录](docs/compatibility-report.json) 和 [T25 证据索引](docs/release-evidence/T25.md)。兼容的 JSON 协议与当次媒体可用性分别记录；公开来源可能改变响应、下架或拒绝访问。XML、原 Android/DEX 插件及需特殊播放器的来源继续暂缓；发布载荷只包含本地前端、原生程序及许可。
- macos-x86_64、windows-x86_64、linux-x11、linux-wayland：**NOT RUN**。真实 macOS 14 最低版本机器：**NOT RUN**；本机隔离测试在 macOS 27.0.1 arm64 完成；干净系统生命周期未验证。
- Vite 仍可能报告主 JS 包体积提示；T24 独立报告中两项搜索海报问题保留。当前交付状态以本卡实际报告为准。

## 后续服务与移动阶段

T1 为独立播放服务，共 15 张功能卡：T1.1–T1.5 元数据服务，T1.6–T1.10 播放服务与 App 选线，T1.11–T1.15 接口适配与联调，对应 [播放服务方案](docs/FOLLOWUP-PLAYBACK-SERVICE.md) 前三期。服务开发等待用户明确启动指令；普通卡只验本卡，三期 App 操作在阶段末卡收口，沿用个人娱乐应用的验收安排（ADR-078/079）。

桌面 T0.36 完成后停止桌面监管，T2.1 等待用户授权（ADR-007/062）：T2.1/M00 Android/iOS 真机可行性与版本基线；T2.2/M01 Android 本地 Rust 来源；T2.3/M02 移动页面与播放器；T2.4/M03 Android 安装发行；T2.5/M04 iOS 本地来源与网关；T2.6/M05 iOS 真机闭环及移动交付。移动适配、签名和真机测试均为 **NOT RUN**。

[需求规格](docs/SPEC.md) · [契约](docs/ARCHITECTURE.md) · [任务/覆盖表](docs/TASKS.md) · [决定](docs/DECISIONS.md) · [原前端历史](docs/FRONTEND_BASELINE.md)。历史资料保存其当时状态，当前实现与发布范围沿已采纳 ADR 和上述真实证据追溯。

## 字体设置

播放设置 → 字体可选择应用内置的 Noto Sans SC 或 Noto Serif SC，并调整 90%–120% 的文字与控件缩放。字体随应用打包，离线可用；字体加载期间使用系统字体。字体偏好通过原设置队列保存，旧配置缺省使用 Noto Sans SC / 100%。两款字体采用 SIL OFL 1.1 许可，完整许可可在字体设置中查看；来源和校验信息见 [字体许可](licenses/fonts/README.md)。

## 重置应用

设置 → 重置应用 → **清理全部缓存并重置**，确认后删除当前应用的设置、添加的来源、观看历史、收藏及全部媒体和来源缓存。应用自动重启，恢复首次启动的默认来源与设置；内置字体继续可用。原“清理媒体缓存”按钮仍可单独清理媒体缓存。

重置目录按平台解析：Windows 使用应用专属 AppData；macOS 包含 Application Support、Caches、Logs、WebKit/HTTPStorages 与临时媒体目录；Linux 跟随 XDG 数据、配置和缓存目录，覆盖 WebKitGTK 存储。重启沿用 Tauri 的 macOS `.app` / Linux AppImage 处理，并在写入重置标记前检查可执行文件。只清理当前应用标识的目录，其他应用及共享父目录保留。同一资料目录只允许一个实例写入。清理失败时进入独立恢复窗口，可重试、取消剩余清理并打开保留的数据，或退出；再次启动会重试待完成的重置。恢复窗口使用系统字体，无需联网。

## 当前数据格式

墨客视频使用独立标识 `app.moc.video`、`moc-video.db` 和 `moc-video.*` 偏好键。设置与历史数据库仅接受当前版本格式；首次使用直接创建当前格式，旧版本数据不迁移、不回退读取，磁盘上的旧文件保持原样。

## 源码仓库

源码仓库：https://github.com/lves-han/moc-video 。仓库包含应用、测试、文档与第三方许可；本机运行证据、安装包、依赖目录和私有配置按 `.gitignore` 保留在本机。
