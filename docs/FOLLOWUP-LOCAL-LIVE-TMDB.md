# 后续工作：本地视频、直播、设置重组、通用 TMDB、MKV

- 整理日期：2026-11-11
- 来源：用户在 2026-11-11 会话中提出的需求与决定（下文“需求原文”一节）
- 原则：不做旧版本兼容，不做数据迁移（D5）。
- 状态：已写入 `docs/TASKS.md` 正式任务卡 T0.38–T0.44（ADR-065）。本文的 T2.1–T2.7 依次对应 T0.38、T0.39、T0.40、T0.41、T0.42、T0.43、T0.44。
- 编号说明（ADR-079）：本文 T2.1–T2.7 为早期方案内部编号，其正式卡号沿上方 T0.38–T0.44；当前工作流 T2.1–T2.6 指移动阶段，见 `docs/TASKS.md`。

## 1. 需求原文与需求 ID

| ID | 用户需求 | 对应卡 |
|---|---|---|
| REQ-X01 | 左侧菜单加入「本地视频」，默认显示 | T2.2、T2.5、T2.7 |
| REQ-X02 | 左侧菜单加入 IPTV、直播源；在设置里配置后才显示。经确认两者合并为一个「直播」菜单 | T2.2、T2.3 |
| REQ-X03 | 「来源管理」合并到设置的「播放设置」 | T2.1 |
| REQ-X04 | 「可添加的播放源」与「HTTP 来源」合并为同一列表 | T2.1 |
| REQ-X05 | TMDB 配置移到设置的「语言与元数据」，做成通用 TMDB 源 | T2.1、T2.4 |
| REQ-X06 | 通用 TMDB 源同时支持 TMDB 官方 API 与 Stremio TMDB 扩展 | T2.1、T2.4 |
| REQ-X07 | 支持 MKV；使用 ffmpeg-next（zmwangx/rust-ffmpeg）在程序内处理，放弃 ffmpeg.wasm 和外置 ffmpeg 可执行文件 | T2.6、T2.7 |

## 2. 已做决定

- **D1 IPTV 与直播源合并。** 对播放器来说两者都是直播频道地址列表，区别只在列表格式：IPTV 常见 M3U/M3U8（`#EXTINF`、`tvg-logo`、`group-title`、`x-tvg-url`），直播源常见 TXT（`频道名,地址`，`#genre#` 分组）。合并为一个「直播」菜单，按内容自动识别格式。
- **D2 TMDB 两种方式并存。** 设置里切换「Stremio 扩展」（默认，无需 Key）与「TMDB 官方 API」（需要 Key）。
- **D3 MKV 用 ffmpeg-next。** 在 Rust 网关里重新封装，FFmpeg 库编译进应用（或作为 dylib 随包分发，见待决定事项 Q2），不附带 ffmpeg 可执行文件。
- **D4 放弃 ffmpeg.wasm。** 依据官方文档：输入 2 GB 硬上限（电影 MKV 常见 4–20 GB）；单线程比原生慢约 25 倍、多线程约 12 倍；默认 core 含 x264/x265 等 GPL 库；需在 CSP 加 `'wasm-unsafe-eval'`；边看边拖动需要自建分段＋MediaSource 管线。
- **D5 不做旧版本兼容，不做迁移。** 当前处于开发阶段，设置结构、默认配置、路由按新设计直接修改，旧设置文件不做任何处理：
  - 旧的 TMDB Stremio 配置（`config_id` `…355001`）及引用它的观看记录不保留。
  - 旧地址 `#/sources` 直接删除，不做跳转。
  - `schema_version` 按新结构直接修改，不写迁移代码。

## 3. 现状要点（2026-11-11 读代码确认）

- 左侧菜单：`src/App.vue` 的 `desktopNavigation`；移动端底栏用同文件的 `nav`。
- 来源管理：`src/views/NativeSourcesView.vue`。「可添加的播放源」是 `src/data/sourcePresets.ts` 从 `src/data/defaultSources.json` 过滤出的 MacCMS 预设，与「HTTP 来源」的 `source_configs` 同一数据类型。
- TMDB 现为一个普通 Stremio 配置：`https://tmdb.elfhosted.com/zh-CN/manifest.json`，语言写死中文。按 D5 直接改为新的 TMDB 设置。
- 观看记录靠 `origin.config_id` 找回元数据来源（`App.vue` 中 `source.origin.config_id === identity.config_id`）。新 TMDB 源需要一个固定的标识，保证新版本内切换语言或方式后观看记录仍可用。
- 设置模型：`protocol/app.schema.json` → 生成 `src/models.ts`（`tests/acceptance/t05_models.py`），Rust 端校验在 `src-tauri/src/settings.rs`。设置 v1 迁移已在 2026-11-11 按 ADR-064 删除；当前只接受 `schema_version` 2，格式不符时报错并保留诊断副本。
- 播放：CSP 的 `media-src` 只允许 `'self' blob: http://127.0.0.1:*`，媒体统一经 `src-tauri/src/gateway.rs` 本机网关；网关已支持 HLS 清单改写（`transfer_manifest`）与 Range。
- 新增后端命令需要在 `src-tauri/capabilities/main.json` 登记权限。

## 4. 任务卡草案

### T2.1 设置页重组与 TMDB（Stremio 方式）
- 依赖：无
- 需求：REQ-X03、REQ-X04、REQ-X05、REQ-X06（Stremio 部分）
- 范围：`src/views/SettingsView.vue`、`src/views/NativeSourcesView.vue`（拆为设置页内的组件或删除）、`src/App.vue`、`src/i18n.ts`、`src/data/sourcePresets.ts`、`src/data/defaultSources.json`、`protocol/app.schema.json`、`src/models.ts`（生成）、`src-tauri/src/settings.rs`
- 交付：
  - 「语言与元数据」加入 TMDB 设置：启用开关、方式（本卡只实现 Stremio 扩展）、实例地址（默认 elfhosted，可改）、语言（默认跟随应用）。manifest 地址按语言生成。其他自定义 Stremio 扩展也放在这一节。
  - 「播放设置」加入：当前元数据源与播放源选择、统一播放源列表（已添加的可编辑、启用/禁用、删除；未添加的预设排在后面带「添加」）、诊断信息（可折叠）。
  - 按 D5：`defaultSources.json` 删除 TMDB Stremio 条目，默认设置改为启用新 TMDB 设置；删除已无用途的 TMDB 预设标记 `metadata_preset_applied`；删除 `#/sources` 路由与 `sources`/`setup` 页面。
- 开工前验证：elfhosted 地址换成 `en-US` 等语言路径能否正常返回 manifest。
- 验收要点：
  - 全新安装后首页直接显示 TMDB 内容，语言与应用一致。
  - 切换应用语言后 TMDB manifest 地址随之变化；手动指定语言时不跟随。
  - 切换 TMDB 语言后，之前的观看记录仍能打开详情。
  - 预设已添加时显示为列表中的普通来源，未添加时显示「添加」，同一 API 地址不会出现两次。

### T2.2 左侧菜单
- 依赖：T2.1（删除来源管理入口）、T2.3 的 `live_configs` 字段（菜单显示条件；可先按空列表处理）
- 需求：REQ-X01、REQ-X02
- 范围：`src/App.vue`、`src/i18n.ts`、`src/components/Icon.vue`（如需新图标）
- 交付：在「资料库」前加入「本地视频」`#/local`（一直显示）和「直播」`#/live`（存在已启用的直播配置时显示）；删除「来源管理」项。移动端底栏同步。
- 验收要点：无直播配置时菜单无「直播」；新增并启用一条后立即出现；全部禁用后消失；焦点导航（`ui/focus.ts`）能走到新菜单项。

### T2.3 直播
- 依赖：T2.1
- 需求：REQ-X02
- 范围：新增 `src-tauri/src/live.rs`、`src-tauri/src/gateway.rs`、`src-tauri/src/lib.rs`/`commands.rs`、`src-tauri/capabilities/main.json`、`protocol/`、`src/state.ts`、新增 `src/views/LiveView.vue`、`src/views/SettingsView.vue`
- 交付：
  - 设置新增 `live_configs: [{config_id, name, url, epg_url?, enabled}]`，在设置页「直播」一节管理。
  - `live.rs` 用现有 `requests` 下载列表，自动识别 M3U 与 TXT，输出统一频道列表（名称、分组、台标、地址、是否支持）。
  - 直播页：左侧分组与频道，右侧播放器。
  - 播放复用 `gateway.open`，新增直播模式：不进媒体缓存、m3u8 清单每次重新拉取、不写观看记录、不显示进度条。
  - 只支持 http/https 和 HLS；`rtp://`、`udp://`、`rtmp://` 频道标为不支持。
  - `epg_url` 本卡只保存，不解析。
- 验收要点：M3U 与 TXT 夹具各自解析出正确分组和频道数；不支持协议的频道被标记；直播播放期间清单持续刷新且不写入媒体缓存与观看记录；列表下载失败时明确报错。

### T2.4 TMDB 官方 API 方式
- 依赖：T2.1
- 需求：REQ-X06
- 范围：新增 `src-tauri/src/tmdb.rs`、`src-tauri/src/source_dispatch.rs`、`src-tauri/src/settings.rs`、`protocol/`、`src/views/SettingsView.vue`
- 交付：
  - TMDB 设置新增：API Key（支持 v3 Key 与 v4 访问令牌）、API 地址、图片地址（两者可改为代理，因 `api.themoviedb.org`、`image.tmdb.org` 在国内常不可达）。
  - `tmdb.rs` 输出与现有 Stremio 数据源相同的首页、详情、搜索数据；图片经网关 `image()`；复用 `matching.rs` 匹配 MacCMS 播放源。
  - 内容标识两种方式统一为 `tmdb:<id>`，切换方式后观看记录仍可用。
- 验收要点：Key 无效时显示明确错误；两种方式下同一部影片的标识一致；自定义 API 地址生效；Key 不出现在日志与诊断报告中。

### T2.5 本地视频（mp4/mov/m4v）
- 依赖：T2.2
- 需求：REQ-X01
- 范围：`src-tauri/Cargo.toml`（新增 `tauri-plugin-dialog`）、`src-tauri/src/gateway.rs`、`src-tauri/src/settings.rs`、`src-tauri/capabilities/main.json`、`protocol/`、新增 `src/views/LocalView.vue`
- 交付：选择文件或文件夹加入本地库（路径存设置）；网关新增 `file` 资源，复用现有 Range 逻辑；观看记录用文件路径哈希作为 `content_id`；WebView 无法解码的格式显示「格式不支持」。
- 验收要点：拖动进度条正常；文件被移动或删除后显示明确错误；重启后本地库与续播位置保留；网关只读取用户加入本地库的路径。

### T2.6 MKV 可行性实验（ffmpeg-next）
- 依赖：无（可与 T2.1–T2.5 并行）
- 需求：REQ-X07
- 范围：独立实验分支或 `scripts/`/临时 crate，不改产品代码
- 已知事实（2026-11-11 查阅）：`ffmpeg-next` 最新 9.0.0（2026-08 发布），README 自述只做维护；默认通过 pkg-config 链接系统 FFmpeg；`build` feature 从源码编译 FFmpeg 并静态链接；需要 clang（bindgen）与 pkg-config；许可证相关 feature 有 `build-license-gpl`、`build-license-version3`、`build-license-nonfree`。
- 待验证：
  - 能否只编译 matroska/mp4/hls 封装与解封装及少量音频编解码器，体积增加多少。
  - macOS arm64 构建耗时、是否还需要 nasm 等工具、能否接入现有打包流程。
  - LGPL 合规方式：静态链接并提供可重新链接的目标文件，或编成 dylib 放进 .app。
  - 一个 H.264+AAC 和一个 HEVC+DTS 的 MKV 样本能否在 WKWebView 中经 fMP4 HLS 播放。
- 交付：实验报告，并在 `docs/DECISIONS.md` 追加 ADR 记录结论。

### T2.7 MKV 支持
- 依赖：T2.5、T2.6（ADR 结论为可行）
- 需求：REQ-X07
- 范围：`src-tauri/Cargo.toml`、`src-tauri/src/gateway.rs`、新增 `src-tauri/src/remux.rs`、`src-tauri/src/subtitle.rs`、`licenses/`
- 交付：
  - 网关用 libavformat 读取 MKV，视频与音频轨道直接复制，输出 fMP4 分片的 HLS。
  - 直接复制：H.264/HEVC 视频；AAC/MP3/AC3/E-AC3/FLAC/Opus 音频。
  - DTS/TrueHD 等 WebView 不支持的音频，用 libavcodec 转成 AAC。
  - 内嵌 SRT/ASS 字幕提取后交给 `subtitle.rs` 转 VTT。
  - 用 MKV 的 Cues 建立分片索引，支持拖动。
  - 更新 `licenses/THIRD_PARTY_NOTICES.txt` 与依赖清单。
- 验收要点：大于 4 GB 的样本可以播放与拖动；DTS 样本有声音；内嵌字幕可切换；停止播放后 FFmpeg 相关资源全部释放；视频编码 WebView 不支持时明确报错。

## 5. 建议顺序

T2.1 → T2.2 → T2.3 → T2.4 → T2.5 → T2.6 → T2.7。T2.6 不改产品代码，可提前并行做。

## 6. 待用户决定

- **Q1 TMDB API Key 存放位置**：随设置文件明文保存（实现最简单），或存 macOS 钥匙串。
- **Q2 FFmpeg 分发方式**：静态链接并附可重新链接的目标文件，或 dylib 放进 .app。T2.6 结束后结合体积与构建结果再定。
- **Q3 MKV 范围**：只支持本地 MKV，或 MacCMS 等网络来源返回的 MKV 也走同一套重新封装。
