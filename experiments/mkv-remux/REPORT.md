# T0.43 MKV 可行性实验报告（ffmpeg-next）

日期：2026-11-11 · 本机 macOS arm64 · FFmpeg release/9.0（commit c179411bbb）· ffmpeg-next =9.0.0 / ffmpeg-sys-next 9.0.1
验收：`tests/acceptance/x6_mkv_spike.py`（运行日志见 `docs/PROGRESS.md` T0.43）

## 结论
可行。用 ffmpeg-next 在程序内把 MKV 重新封装为 fMP4 HLS，真实 WKWebView 能播放 H.264+AAC 与 HEVC+DTS（DTS 转 AAC）两类样本，FFmpeg 保持 LGPL 2.1+，不依赖系统 brew FFmpeg。

## 两条路线实测
| | 路线 A：`ffmpeg-sys-next` 的 `build` | 路线 B：自编精简 FFmpeg + `FFMPEG_DIR` 静态链接 |
|---|---|---|
| 组件 | FFmpeg 全部内置组件，只能按整库开关，无法裁剪编解码器 | `--disable-everything` 后只开启所需组件（见 `build_ffmpeg.py`） |
| 程序体积（空 Rust 程序 0.47 MB） | 17.5 MB（增加 17.0 MB） | 5.5 MB（增加 5.0 MB） |
| 静态库 | — | libavcodec 16.1 MB、libavformat 5.9 MB、libavutil 2.8 MB、libswresample 0.4 MB（链接时只取用到的部分） |
| 编译耗时（-j2） | 85 秒（含 git clone） | 69 秒（不含 clone） |
| 许可证 | LGPL version 2.1 or later | LGPL version 2.1 or later |
| 动态依赖 | 仅 macOS 系统框架 | 仅 macOS 系统框架 |

所需工具：clang、pkg-config、nasm、git、make（本机均已具备）。构建时从 GitHub 克隆 FFmpeg 源码。

## 处理能力
- 视频：H.264 直接复制（avc1）；HEVC 直接复制并标 `hvc1`（苹果播放器要求）。
- 音频：AAC/MP3/AC3/E-AC3/FLAC/Opus 直接复制；DTS/TrueHD/Vorbis 解码后转 AAC（DTS 样本 48 kHz 双声道，0.14 秒处理 12 秒）。
- 字幕：SRT 提取为文本（2 条均正确，含中文），交给现有 `subtitle.rs` 转 VTT 即可。

## 踩到的坑（T0.44 要注意）
1. `--disable-everything` 同时去掉了 H.264/HEVC 解码器，导致探测阶段拿不到 B 帧延迟，复制的视频包没有 DTS，HLS 封装报「DTS 非单调」。只需开启这两个解码器用于探测，不解码画面。
2. Rust 会把 `static=` 原生库打进 `ffmpeg-sys-next` 的 rlib；重新编译 FFmpeg 后必须清理 `ffmpeg-sys-next` 再构建，否则仍链接旧库。
3. WebKit 会暂停隐藏或未激活窗口中的视频，自动化检查需把窗口放在屏幕上（可几乎透明）并激活应用；未激活时曾出现加载完成但不前进的间歇失败。
4. `ffmpeg-sys-next` 静态链接时固定加入一串系统框架（含 QTKit，会出现无害的链接警告）。

## 本实验未覆盖
- 边播边生成：实验是整文件离线转换。产品需要在网关里按请求生成分片，并用 MKV 的 Cues 建索引支持拖动，大于 4 GB 文件的表现由 T0.44 验证。
- 发行打包、签名与 FFmpeg 许可证声明。

## 建议（待用户决定 Q2）
采用路线 B（精简组件）。LGPL 合规有两种做法：
- **dylib 放进 .app**（推荐）：把精简 FFmpeg 编成动态库随应用分发，用户可自行替换，合规最直接；体积与静态相近。
- **静态链接**：需随发行提供可重新链接的目标文件。
