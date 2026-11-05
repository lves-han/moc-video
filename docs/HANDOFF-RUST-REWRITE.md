# 映象视频交接：从 Android 兼容层改为 Rust 重写播放源

写于 2026-11-05。接手方：Codex。配套提示词见 [HANDOFF-RUST-REWRITE-PROMPT.md](HANDOFF-RUST-REWRITE-PROMPT.md)。

## T0.19 开发会话2交接（2026-11-05，WIP，待外层原入口验证）

- `task.py check`通过35卡，`start T0.19`恢复成功，开始时工作树干净；当前protected为`5f24a743a02cebf238fccdc755a1f5c8ae2aecd3617d3fc2d240d577554de7d1`，generation=2，sessions=2/2。冻结定义、控制脚本、预算和任务勾选保持；本会话仅处理T0.19。
- 已实现待验：公共HTTP的测试专用nameserver注入扩展为多服务器；受控UDP验证A/AAAA、SERVFAIL/静默单服务器可用性、双服务器失败、hosts/系统无回退、双静默服务器deadline/取消、混合公网内网/Fake-IP、真实重定向中的DNS重绑定，复用原peer/重定向安全反例。正式解析仍固定1.1.1.1与8.8.8.8，无新增设置或依赖。
- 公开fixture：`tests/fixtures/t02/public-responses.json`从既有固定公共DNS报告`evidence/t02-public/3c1aeb86-0ebf-4baf-b06f-2f0e851bc02f/capture.json`提取8来源各7阶段的脱敏响应，保留原始字段、原响应SHA、列表、分类两页、搜索及完整组选集顺序。媒体与图片使用原投影的自有fixture地址，原始媒体签名未入库。Rust测试覆盖7候选公开协议、1280集样本原始顺序、淘片缺code拒绝、HTML/错误code/畸形/超限与直接媒体解析；这是待执行的测试覆盖，真实来源支持保持0。
- 最小产品修复：代码检查发现详情之后上游清空全部播放组时，解析原返回普通“组缺失”。`maccms::Catalog::resolve`现复用`resolution::no_links`，返回`UPSTREAM_FAILED`与`reason=no_links`；已添加“详情后移除媒体”回归。当前环境尚未执行该回归，运行前后差异仍待验证。
- A1入口已实现于`tests/acceptance/t02.py`，复用原验收收据/资源验证、测试清理、T01生产/自动化分包、原生GUI/RSS生命周期。先执行独立证据负例与10个新原生测试，再执行原not_found/no_links/cap隔离3项回归；真实链路复用手工MacCMS配置、受控Stremio仅作人工匹配身份、公开JSON/完整详情/Host resolve/真实Rust网关、打包WKWebView实际解码与5秒短播/音频。未有合格媒体、锁屏、媒体/原生错误都会使A1失败。无成功来源时保留报告且在线闭环保持NOT RUN。原始上游媒体地址仅在Rust内存，短寿命测试桥仅传递网关DTO；报告移除cap参数。
- 取证修正：原诊断只选首组，部分公开接口首组为HTML播放器、后组为直接HLS。现按公开JSON显式选择原始组中的MP4/HLS并记录组号。已明确403的优质/无尽在候选清单保持停止；量子前次解析HTTP失败未记录数字状态，按不确定访问控制保持停止；淘片缺成功code保持排除；卧龙非JSON保持排除。红牛/光速/极速/豪华的后续直接组待真实验证，未切换端点、未绕过访问控制。
- 已执行仅静态检查：两Python文件AST解析成功；`git diff --check`无空白告警；`task.py scope`为`all changes inside scope`。当前新增Rust/TypeScript的编译、格式、Clippy、行为测试、准确A1、完整verify、包排除及独立代码巡检全部NOT RUN。旧17项回归与取证exit0仍为历史诊断，未替代本次验收。
- 按本会话明确交接及ADR-010，已确认的sysctl/Git权限限制交外层处理，未重复重任务权限探测、未绕过原run、未扩权限或重置会话。旧环境收据现为finished/exit125/raw_exit=null，原失败保留。开发会话以稳定WIP退出；外层先核对本会话已退出、PID+创建时间/两锁/无活跃写入者，再提交`WIP T0.19: add public JSON and DNS acceptance gates`。新代码须先经原run执行cargo fmt，再执行Clippy与准确A1；通过后直接完整verify、真实完成记录/勾卡/done/受验提交及原review-only独立CLEAR。
- 可执行argv与顺序见`evidence/task-state/T0.19/session-2/outer-validation-argv.json`。格式化会改变源码，后续所有收据必须绑定格式化后的稳定摘要。外层须检查剩余截止时间覆盖完整命令；保留7200秒A1上限及原资源预算。若真实媒体或验收失败，保留日志，先note原因；本卡两会话已使用，所需新增开发/授权按原工作流报告。本会话未提交WIP、未写BLOCKED/RESOLVED/AUTHORIZED、未运行done或推进其他卡。

## T0.19 开发会话1交接（2026-11-05T03:12:33Z，WIP）

- 当前文档基线已经迁移且独立round-42 CLEAR；原check通过，start T0.19成功，初始工作树干净。protected=`8da3c6c5d17adbd958f588e4635df813ab500066d47e8dac85bc82b3747e2b13`。本次只处理T0.19。
- 原run.py环境预检exit125；`evidence/runs/20261105T030511501379-diag-t02-environment/receipt.json`为`cleanup_failed`，macOS进程枚举sysctl返回EPERM，raw_exit/tests未知。已task note；未重试、未recover、未绕过执行器。按ADR-010由外层在开发会话退出后确认原PID/创建时间、清理边界并用原task.py recover收尾。开发CLI权限保持。
- 当前增量仅为取证准备：`tests/fixtures/t02_capture.rs`复用生产Http的DNS/peer/TLS/重定向/取消边界，按来源记录列表、详情页、按ID详情、正/零搜索、分类两页以及现有Catalog详情/首集resolve/64KiB直接媒体前缀。媒体/图片URL全部替换；原始响应、媒体签名和请求头留在原生内存；HTTP状态与JSON/适配判定分开，访问控制停止该候选。取证输出逐来源保存到新的`evidence/t02-public/<UUID>/capture.json`，所有`supported`保持false。
- `docs/compatibility-report.json`记录9候选、AES既有核实结论、公开帮助页和各层NOT RUN。它同时是取证入口的候选清单，属于测试/文档输入，产品尚未内置预设。优质准确端点与淘片详情仍需本机取证确认；卧龙维持排除。原包/补丁保留，当前tauri配置resources为空、Cargo无Android运行依赖，实际包扫描待A1。
- 已实际执行：rustfmt解析/格式化新Rust文件成功；git差异空白检查无告警；task.py scope=`all changes inside scope`。Rust编译、脱敏回归、真实取证、A1及完整verify均NOT RUN。Web工具读到量子/红牛/光速/极速公开帮助页；量子和无尽API正文工具不可达，未据此判定本机站点状态。
- 外层下一步：先按真实身份处理上述cleanup_failed；通过原run.py依次执行下方两个准确argv。第一项只验证新脱敏投影；第二项是显式忽略的公开取证诊断，退出0只表示取证循环结束，各来源的HTTP/协议/媒体结果须读capture.json，仍未达到A1。需要修复时沿本卡剩余第2会话，保留原会话/失败，不重置额度。

```json
[
  ["{python}", "-B", "scripts/run.py", "diag-t02-projection", "--", "cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "host::maccms::tests::t02::t02_capture_projection_removes_signed_urls_and_preserves_group_order", "--", "--exact", "--test-threads=1", "--nocapture"],
  ["{python}", "-B", "scripts/run.py", "diag-t02-public", "--", "cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "host::maccms::tests::t02::t02_capture_public_json", "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"]
]
```

- 剩余开发：审阅真实脱敏响应并固化fixture；按实测差异最小修复maccms，保持正常零搜索与错误code分离、无媒体no_links；实现`tests/acceptance/t02.py`完整A1和至少一个候选真实Rust网关→打包WebView短播；逐来源晋级、包排除、安全负例/cap隔离；新源准确A1通过后完整verify、勾卡/done、受验提交及独立巡检。当前未修改产品源码、冻结定义或任务勾选，未执行done或Git提交；.git写入属于外层既有分工。

## 当前接续更新（2026-11-05，ADR-048）

用户已明确批准：“许可选择依据站点公开行为独立实现，暂时仅供个人使用，首版先做可用的 SuperVod JSON 来源，其余按你的建议。批准修订文档和迁移基线，独立审查通过后按任务顺序开始开发，先核实 AES 用途。”

- 文档当前已修订为T0.19 JSON适配→T0.20身份/完整性→T0.21上下文/隔离→T0.22整合→T0.18→其余桌面卡；当前迁移/独立审查/实现状态以PROGRESS、task status和原收据为准。此处不预填rebaseline或CLEAR成功。
- 许可：依据公开HTTP行为独立实现，未见许可证的插件源码只作范围风险识别。个人使用范围已确认；源码翻译许可、内容版权授权和公开发行授权未由本次决定授予。其他六站、XML、特殊播放器API暂缓；移动沿本地Rust路线但阶段门保持。
- AES静态核实：固定提交SuperVod 15个kt中aesDecrypt仅Utils.kt定义，无调用；BaseVodProvider直接输出媒体，encrypt1/2转义/Base64；Haiwaikan/JiangNan注册注释。普通JSON无需新增插件AES解密；HLS标准AES-128保留现有网关验收。实现以公开响应独立完成。
- 公开普通GET：优质、量子、红牛、无尽、光速、极速、豪华`ac=list&pg=1`及`ac=detail&pg=1`返回code1、list20，detail有播放字段；无尽/光速/极速/豪华page为字符串。淘片简略JSON待详情核实；卧龙返回非JSON暂排除。全部仅为协议候选，真实媒体闭环NOT RUN，逐个晋级支持。
- 本机普通请求的其余结果：百度/天空/U酷/iKun/樱花/金鹰/闪电/非凡/暴风/速播/新浪/索尼/魔都/神马聚合/影图/p2100为403；快车/inmi证书校验失败；飞速/八戒/快看/394tv为TLS握手异常；海外看吧405；Haiwaikan连接关闭、江南TLS异常。Mxplayer首页200/API根403，Libvio403，Ddys/Yjys握手异常，Dramacool/Zxzj证书失败。遇失败即停止该候选，未规避访问控制、未关闭TLS校验；结果仅说明本机当次情况，永久关闭尚未证实。
- 状态核对：初始Git工作树干净，T0.19/sessions0/protected摘要与下方旧快照一致；check通过35卡。无存活driver、heavy任务或未完成收据。当前聊天已建立每5分钟监管，旧暂停任务保持；记录于evidence/drive/supervision.md。首次核实屏幕锁定值unknown，打包验收前再核对，锁定时请用户解锁。
- 磁盘更新：本次约106GiB可用、已用89%，下方旧快照20GB/98%已过时；历史证据保持，未清缓存。

以下第1至10节为迁移前交接快照，涉及“待决定”“尚未修订”“Android卡”的状态由上述授权、ADR-048及当前TASKS/ARCHITECTURE取代；保留原决策和失败依据。

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
4. 运行 `task.py check`，然后 `task.py rebaseline T0.19 --protected <摘要>`。注意：`rebaseline` 会拒绝范围外的非冻结文件，而这两份 `docs/HANDOFF-RUST-REWRITE*.md` 目前在 T0.19 范围外。修订 T0.19 的范围时把它们列入，先用 `task.py scope` 确认为空。修订后 T0.19 不再需要的 `android-engine/` 改动同理。
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
