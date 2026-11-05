# 映象视频交接：从 Android 兼容层改为 Rust 重写播放源

写于 2026-11-05。接手方：Codex。配套提示词见 [HANDOFF-RUST-REWRITE-PROMPT.md](HANDOFF-RUST-REWRITE-PROMPT.md)。

## 1. 一句话现状

桌面前端和 Rust 宿主已做到来源管理（T0.1 至 T0.17 共 17 张卡完成，每张都有准确 A1、完整 verify 和独立巡检）。用户决定放弃 Android 兼容层，改用 Rust 重写播放源。**重写尚未开始，规格和任务卡尚未修订。** 第一步是修订文档并取得用户确认。

## 2. 用户的决定

| 决定 | 用户原话 | 记录 |
|---|---|---|
| 用 Rust 重写，不做 Android 兼容层 | “我想使用rust重写” | ADR-047 |
| 由 Codex 接手 | “后续交给codex” | ADR-047 |
| 先做桌面，移动端在稳定上线后扩展 | “调整顺序，先做桌面端，移动端作为稳定上线后的产品扩展” | ADR-046 |
| 验收按娱乐软件标准，没有泄漏即可 | “基本上都维持在300MB附近，没有出现内存泄露即可，这是一个娱乐软件，不是工业软件” | ADR-044 |

用户尚未决定的事项见第 6 节，不要替用户决定。

## 3. 当前代码状态

- 分支 `main`，最新提交见 `git log`；T0.17 完成提交是 `82e429c`。
- `scripts/task.py next` 现在指向 T0.19，它已被 `BLOCKED` 暂停，后续卡都依赖它。
- T0.19 的 `sessions` 是 0，因为 T0.14 起都在对话内开发，未经 `drive.py --once`（ADR-043、ADR-045）。
- 当前 protected 摘要：`034b3b6a7baaa956956d0e29fe6a3b3c859ff2671fdb85df8d5ee310ebc7d129`，generation 视卡而定。

### 已完成的产品能力

| 卡 | 内容 |
|---|---|
| T0.13 | AppState、路由、双语、五色系、会话内历史 |
| T0.14 | `HomeView`：骨架、栏目原位失败、分页、4 至 7 列、过期图片恢复 |
| T0.15 | `DetailsView`：选集、分组、确认弹层、相关内容 |
| T0.16 | `PlaybackView`：控制项、快捷键、全屏、选集浮层、自动下一集、原生历史快照 |
| T0.17 | `NativeSourcesView`：HTTP 来源增改禁用删、独立选择、诊断；`CatalogView` 已删除 |

### 后端结构（重写的落点）

- `src-tauri/src/source_dispatch.rs`：`SourceKind` 现有 `Cloudstream`、`Maccms`、`Stremio`。
- `src-tauri/src/maccms.rs`：现成的 MacCMS 适配器，SuperVod 应优先复用。
- `src-tauri/src/matching.rs`：跨源匹配。`external_identity` 目前只有已验证映射才会自动匹配，MacCMS 的是 `None`，所以真实匹配总是要用户确认。
- `src-tauri/src/resolution.rs`、`gateway.rs`、`hls.rs`、`subtitle.rs`：解析与媒体网关，原生 HTTP 来源已在用。
- `src-tauri/src/host.rs`：`start_interaction`、`finish_interaction`、`runtime_prepare(cloudstream)` 目前返回未就绪错误。
- 前端 `src/state.ts`、`src/views/NativeSourcesView.vue` 里有约 22 处 Cloudstream 相关逻辑，其中状态和交互界面只用合成数据测过。

## 4. Android 这条线留下的东西

全部可以删除或保留作参考，由修订方案决定：

| 路径 | 内容 | Git |
|---|---|---|
| `android-engine/engine.patch`、`overlay/`、`.gitignore` | engine flavor 补丁和入口代码，**从未编译成功** | 已跟踪 |
| `android-engine/dependencies.lock.json` | 候选依赖 | 已跟踪 |
| `tests/acceptance/t02_build.py` | 拉取和构建脚本 | 已跟踪 |
| `android-engine/upstream/` | Cloudstream 上游 `a7ac454`（27MB） | 忽略 |
| `android-engine/packages/` | 7 个原始 `.cs3`，SHA-256 与清单相符 | 忽略 |
| `android-engine/.tools/` | JDK 17 和部分 Gradle 缓存（约 650MB） | 忽略 |

Android 构建被主动中断（`evidence/runs/20261105T024100841304-diag-t02-build-2`，exit 125），没有任何成功证据。

## 5. 重写的已知事实

下表来自对 7 个包的**静态读取**。没有运行插件，也**没有验证这些站点现在是否可访问**。

| 插件 | 静态依赖 | 初步难度 |
|---|---|---|
| SuperVod | 多个 MacCMS 接口、AES | 最低，可能接近配置预设；AES 用途未确认 |
| Mxplayer | HTML 解析、站点 API | 低 |
| Ddys | HTML 解析、拦截器 | 低到中 |
| Libvio、Zxzj | HTML 解析、AES | 中 |
| Dramacool | `loadExtractor` | 中到高，要移植用到的解析器 |
| Yjys | `CloudflareKiller` | 最高，可能要隐藏 WebView，也可能放弃 |

插件源码在 `https://github.com/821938089/cloudstream-extensions`，提交 `25cdd2119be03f62ae6a5a2cf46b16f9484224a2`。**该提交的 `LICENSE` 返回 404。**

## 6. 需要用户决定的事项

先把这些问题和你的建议一起交给用户，取得答复后再开始实现：

1. **许可。** 上述仓库未见许可证，照源码翻译可能构成衍生作品。选项：联系作者取得许可；只依据站点实际行为独立实现；或接受风险。这需要用户判断，必要时咨询律师。
2. **内容版权。** 这些是第三方影视站点，改成内置实现后由产品直接维护抓取逻辑。是否公开发行、覆盖哪些站点，由用户决定。
3. **首版范围。** 建议先做 SuperVod，再逐个做可访问的站点；Dramacool 和 Yjys 看价值再定。
4. **需求修订。** U02、U05 作废的具体措辞，以及 M 系列里依赖 Android 引擎的 M01、M03、M04 如何处理。
5. **验收样本。** 真实站点不稳定，每个来源要以录制的 fixture 为主、真实站点冒烟为辅，还是其他方式。

## 7. 文档修订路径

规格被冻结，必须走原流程，不能手改状态：

1. `docs/SPEC.md` 必须与 `replica-spec/DEVELOPMENT_SPEC.md` 逐字节相同（`scripts/check-project.py` 校验），保留原文，用 ADR、`docs/ARCHITECTURE.md`、`docs/TASKS.md` 取代相应要求。ADR-044 是现成先例。
2. 修订 `docs/TASKS.md`：T0.19 至 T0.22 改为 Rust 播放源卡。控制器只识别 `T数字.数字`，沿用这些卡号最省事，后续卡的依赖不用大改。T0.23（索引与制品事务）需重写或取消。T0.24、T0.25、T0.27 要去掉 Android 和原包内容。同步更新覆盖表，**不要改动已完成卡（T0.1 至 T0.17 对应卡）的正文**，否则其完成校验会失效。
3. 追加 ADR，并在 `docs/PROGRESS.md` 追加 T0.19 的 `- 结果：RESOLVED`，含 `- 决定：` 和 `- 批准摘要：<protected_hash>`。上一条决定记录已是本卡 `BLOCKED`。
4. 运行 `task.py check`，然后 `task.py rebaseline T0.19 --protected <摘要>`。
5. 按 `docs/PROMPT-DOC-REVIEW.md` 做一次文档独立审查再开发。

## 8. 每张卡的完成流程

1. `task.py check`、`next`、`start <卡号>`，只改卡的范围内文件；无关发现追加到 `docs/BACKLOG.md`。
2. 按 `task.py show` 的准确 argv，经 `scripts/run.py A1 -- ...` 跑验收，不能套后台进程绕过。
3. 直接运行 `scripts/verify.py all`，不能套 `run.py`。
4. 勾选卡片，`docs/PROGRESS.md` 追加 DONE（含 A1 日志和 verify 收据路径），`task.py done <卡号>`。
5. 提交 `<卡号>: <摘要>`，再运行 `scripts/drive.py --review-only`。巡检 `BLOCKING` 必须修；不要自行 `accept-review`。
6. 先写回归、确认在旧代码上失败，再修缺陷。巡检报模型满载（`FAILED`）时，等几分钟用原入口重试。

`tests/acceptance/t14.py` 的 `packaged()` 和 `Acceptance`，以及 `t15.py`、`t16.py`、`t17.py` 可以复用：真实 Rust Host 加打包 `WKWebView`。

## 9. 环境注意事项

- **屏幕：** 打包窗口验收要求桌面解锁、窗口在前台。显示器 10 分钟熄屏就会锁定，已多次打断验收。Node 和 Rust 单测不受影响。
- **磁盘：** 约 20GB 可用，已用 98%。`target/` 12GB 是构建缓存，`evidence/` 3GB 含失败历史，不能删。
- **同机项目：** `/Users/eason/moc/app` 可能在跑容量测试，会抢资源，不要碰它的进程。
- **Git：** 不推送、不改写历史，禁止 `reset --hard`、`clean -fdx`、`rebase`、`commit --amend`。
- **沟通：** 用户用中文。回答遵守 `AGENTS.md` 开头的句式限制，结束报告用 Changed / Why / Verified / Remaining。
- **权限：** ADR-010 记录过 Codex CLI 在 `workspace-write` 下无法写 `.git`、无法用 `heavy.lock` 和枚举进程。如果你也受限，准确说明并交用户处理，不要绕过。

## 10. 未解决的小问题

| 问题 | 位置 | 说明 |
|---|---|---|
| 中文界面下来源表单校验提示是英文 | `validateSourceDraft`（`src/state.ts`） | T17 复巡 P2，确认的小缺陷，尚未修 |
| 原生历史续播入口 | `continueHistory`（`src/App.vue`） | 提示不可用，计划属 T0.18 |
| 原始 RSS 采样文件缺失 | `t01.NativeSession.close` | 证据细项，峰值和退出状态已有 |
| T09 网关测试偶发超时 | `docs/BACKLOG.md` | 出现一次，未复现，原因未确认 |
| T14 空源和整页失败用注入响应 | `tests/fixtures/t14_webview.ts` | 真实上游链路未测 |
| 自动下一集成功路径用已确认 selection | `tests/fixtures/t16_webview.ts` | MacCMS 无已验证身份；Rust 来源要重新考虑匹配 |
| 窗口焦点相关验收失败 | T14、T17 的 A1 历史 | 根因是假设，未证实 |

`T0.16` 约 5 秒周期写入只有单测，网络错误、缓冲和 DPI 未在原生窗口单独测。
