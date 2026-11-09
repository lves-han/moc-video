# 文档审查报告 round-48：桌面 Apple TV 化（ADR-057）

## 受审基线

- Git HEAD：`93897b046d1d72acb6bd76b679940521a74a0600`，工作树干净（`git status --short` 无输出）。
- `task.py status` 的 protected：`d441fca11eb084cd9b4e82156ff1e3e1c867de608b66187879689ff03d07572a`，与卡状态 `state.protected` 一致。
- ADR-057 分段摘要（含两条补充）：`be46469cc901d460d5fcdc136f43b8d435c2a8f0c04b25a3b821f6b4b13f2139`。
- `task.py status`：current=T0.27，blocked 为空，sessions=2，generation=1，rebaselines=1（2026-11-09T01:48:35，previous `b98bb8d6…`），rebaseline_scope=`docs/ARCHITECTURE.md`。
- `task.py check`：`roadmap OK: 42 cards`，退出 0。该结果只证明结构合法。

## 需求覆盖结论

| 用户原话 | 落点 | 结论 |
| --- | --- | --- |
| 改造成类似 Apple TV，只改桌面，含 app 页面和使用体感 | C17 按 960px 分界；T0.28 外壳/焦点，T0.30 首页与分类，T0.32 详情与转场，T0.33 播放器，T0.34 搜索/资料库/来源/设置 | 页面面已覆盖；窄屏保持有 `TV-SHELL/narrow-unchanged` |
| 推荐按分类区分，如电影、电视剧、年份、类型 | T0.30 | 部分覆盖：电影、电视剧有内容排；年份、类型只在筛选栏出现（B4） |
| 更多分类，丰富筛选，全部按 Apple TV 来做 | T0.30 四类，T0.31 综艺/短剧/体育，T0.29 和 T0.30 筛选 | 分类已覆盖；五个筛选维度中的“地区”“排序”没有数据通路（B3） |
| 整理成任务卡放到 T0.27 后，原卡顺延 | T0.28–T0.34，T0.35、T0.36 | 已落实（见“顺延核对”） |
| 0.28–0.34 详细规划页面风格 | 各卡“页面风格规格”及 visual cases | 规格可实现，存在 B2、B5 的验收缺口 |
| 授权执行；加入 4 个文件并重验 | PROGRESS RESOLVED，rebaseline，T0.27 范围 | 记录符合流程 |

- 双向对应：覆盖表 TV-* 与 REQ-TV1..7 共 23 行，每行都能在对应卡的 `cases` 中找到以该 ID 开头的用例，未发现孤立用例。
- 排除伦理片、福利、擦边短剧的来源是 ADR-057 决定段，由用户“授权执行”批准。ADR-057 引用的方案文档位于 claude.ai，本轮无法读取，已列入未验证项。

## BLOCKING 问题

### B1 契约检查脚本与 t05.rs 未进入范围，T0.29/T0.32/T0.34 的完整 verify 必然失败

- 需求 ID：TV-CATALOG-DTO、TV-WATCHLIST、TV-SEARCH、REQ-TV2/5/7。
- 位置：TASKS.md:1681（T0.29 范围）、1887（T0.32 范围）、2035（T0.34 范围）；`scripts/check-contracts.py:25,30,36`；`src-tauri/tests/t05.rs:76`；ARCHITECTURE.md:167,169,184。
- 因果链：
  - `MediaCard`、`HomeSection`、`HomeRequest` 定义在 `protocol/bridge.openapi.json`，且均为 `additionalProperties:false`，T0.29 必须改这个文件。
  - check-contracts.py:25 断言 `bridge == original.read('bridge.openapi.json')`，任何改动都会让 `verify-working-contracts` 失败。
  - 第 30 行要求既有 `$defs` 相等。T0.29 若把“已补过 TMDB”的标记放进 Settings，也会触发这里。
  - 第 36 行要求 `additions == HISTORY | CACHE`，T0.32 的 3 个 watchlist 命令和 T0.34 的 `search_catalog` 都会失败。
  - t05.rs:76 硬编码 `COMMANDS.len() == 23`。`src-tauri/tests/` 不在 T0.32、T0.34 范围内（范围里的 `tests/` 只匹配根目录），`cargo test` 会失败。
  - `task.py done` 需要完整 verify，三张卡都无法在范围内完成。先例：ADR-054 把原契约检查脚本列入了 T0.24 范围。
- 影响：T0.29 是第一张硬失败的数据卡，其后依赖它的卡全部停住。
- 最小修订：
  - T0.29 范围加入 `scripts/check-contracts.py`；T0.32、T0.34 加入 `scripts/check-contracts.py` 和 `src-tauri/tests/t05.rs`；T0.31 若改 schema 同样加入。该脚本不在 PROTECTED 列表，改范围即可。
  - 在 C17 写明校验收紧为“归档字段逐项相等，仅允许列名的可选字段”，命令追加集按卡递增：T0.32 为 HISTORY|CACHE|WATCHLIST，T0.34 再加 search。ARCHITECTURE.md:184 的“原有 DTO 兼容”随之对齐，避免把校验放宽成全量放行。

### B2 全部视觉验收要求 1920 宽度，自动化入口拒绝该宽度且文件不在范围内

- 需求 ID：TV-THEME/visual-tokens 及 T0.30–T0.34 的 visual-spec。
- 位置：TASKS.md:1590、1757、1831、1894、1969、2042；`src-tauri/src/automation.rs:66`；TASKS.md:1583 等范围行。
- 因果链：
  - 真实窗口宽度由 `t01_control` 的 `width` 动作设置，白名单为 `[360,390,768,960,1200]`，其他值返回 `Unsupported acceptance width`。
  - T0.28–T0.34 无一把 `automation.rs` 列入范围。
  - 规格里的 ≥1600px 断点（右边距 64px、卡片间距 20px、字幕 34px）也只能在该宽度验证。
  - 本机内屏 3024×1964 Retina，按常见 2 倍缩放的逻辑宽度约 1512，窗口能否设到 1920 未经实测。
- 影响：T0.28 是路线上第一张无法通过验收的卡。
- 最小修订：T0.28 范围加入 `src-tauri/src/automation.rs`，白名单增加 1920；工作义务增加开工实测窗口能否达到 1920，达不到时由用户决定改用的宽度或外接显示器，并写入 PROGRESS。

### B3 筛选维度“地区”“排序”没有定义的数据通路

- 需求 ID：TV-FILTER、TV-BROWSE；用户原话“丰富筛选功能”。
- 位置：TASKS.md:1722（filter 只含类型、年份、语言、地区）、1724（路由只有语言、年份、类型，过滤只用 genre、year）、1688（A1 输入无地区）、1757、1795（五个维度含排序）；C17 ARCHITECTURE.md:167。
- 因果链：
  - `HomeRequest.filter` 没有排序字段。T0.30 范围不含 `protocol/` 和 Rust，无法自行补上；排序的选项、取值和服务端/前端归属都未定义。
  - `地区` 列在 filter 中，但 Stremio 路由和 Rust 过滤都没有它，`MediaCard` 也没有地区字段。
  - TASKS.md:1741 允许“该维度胶囊禁用”，T0.30 又要求“五个筛选胶囊可组合”，两处矛盾。TMDB 来源下要么无法通过，要么靠禁用胶囊虚假通过。
- 影响：TV-BROWSE/filter-bar-combination 无法准确判定，错误实现也无法被拒绝。
- 最小修订：
  - T0.29 增加排序定义（选项及映射到目录或客户端排序），或声明排序为前端对已加载结果的排序并列出选项。
  - 明确 TMDB 来源地区维度的处理：禁用，或给出取值依据；MacCMS 由 T0.31 过滤。
  - 据此改写 T0.30 A1，例如“TMDB 下类型、年份、语言、排序可组合，地区按来源声明禁用”，并在 T0.29 cases 增加对应用例。

### B4 “推荐按年份、类型区分”未进入首页规格和验收

- 需求 ID：TV-HOME；用户原话“推荐需要按照视频分类进行区分，如电影，电视剧，年份，类型”。
- 位置：TASKS.md:1793（首页固定顺序）、1757、`TV-HOME/category-rows`；分类页默认内容排未列举。
- 因果链：首页依次为 Hero、继续观看、趋势电影与剧集、最新上线与热门电影、华语韩语日语精选，没有按年份或按类型的推荐排。分类页默认包含哪些排没有规定。开发者照卡实现即可通过验收，该项要求在推荐区仍未落实。
- 影响：明确的用户要求只被筛选功能替代。
- 最小修订：在 T0.30 首页或分类页默认视图增加“按类型”“按年份”内容排并写明数据来源，增加用例如 `TV-HOME/genre-year-rows`。若作者认为筛选栏已满足，需取得用户明确确认并记录。

### B5 T0.28 的 MediaRow 规格在真实窗口无处可测，T0.30 也未接手

- 需求 ID：TV-THEME/visual-tokens、TV-FOCUS。
- 位置：TASKS.md:1583、1590、1625、1672；T0.30 的 `TV-HOME/visual-spec`（1757）。
- 因果链：
  - T0.28 交付 `MediaRow.vue`，A1 要求真实打包窗口计算样式核对“本卡页面风格规格”，其中含 MediaRow 与加载/缺图态。
  - 产品页面 `HomeView.vue` 不在 T0.28 范围，到 T0.30 才接入，T0.28 时没有真实入口。
  - T0.30 的 visual-spec 只核对“本卡”规格，MediaRow 的海报宽度、留白、翻页按钮在真实页面上没有归属验收。
- 影响：验收依赖后卡，且最终接线无人覆盖。
- 最小修订：T0.28 A1 写明 MediaRow 由夹具挂载核对并说明边界；T0.30 A1 增加“首页内容排按 T0.28 的 MediaRow 规格核对”，并加用例。

### B6 C17 的安全边界没有可观察断言

- 需求 ID：C17 IPC 与权限；TV-SHELL、TV-WATCHLIST、TV-SEARCH。
- 位置：ARCHITECTURE.md:169；TASKS.md:1622；`tests/`、`src-tauri/tests/`、`scripts/` 中检索 `capabilities/main.json`、`allow-get-media-cache`、`start-dragging` 均无结果。
- 因果链：C17 规定主窗口权限只增加 4 项 IPC 和 `core:window:allow-start-dragging`，CSP、CORS、TLS 保持。现有测试和新 cases 都没有核对这些内容，授予 `core:window:default` 或放宽 CSP 的实现可以通过。
- 影响：安全契约只有文字，缺少验收证据。
- 最小修订：T0.28、T0.32、T0.34 的 A1 各增加一条断言：`capabilities/main.json` 的权限集合与卡内累积期望完全相等，`tauri.conf.json` 的 CSP 与基线一致。

## 可行性推演（控制器语义）

1. **T0.27**：`completed()` 因 generation 不一致返回 False，`next_card` 继续返回当前卡，`[x]` 保留不影响。base 到 HEAD 的改动文件都落在范围、DYNAMIC 或 rebaseline_scope 内。失败最高同签名 3 次，无 hard_sessions。sessions=2 按 ADR-045 由当前对话直接执行，不走 drive --once。
   - 前置条件：`t23.py` 要求 `observations.json` 的 source、protected 与 manifest 相等，含 `human_observer` 和用户声音确认。源码和 protected 都已变化，重验需要在专用 UTM 机上重做完整生命周期观察，并由用户确认声音。该路径上轮已走通，用户也授权完整重验，判为可达。
   - 运行前需先 `task.py note`，因为控制器要求最新失败之后有 note；本轮未核对当前时间戳关系。
2. **T0.28**：第一张无法完成的卡，原因见 B2。
3. **T0.29**：verify 失败，原因见 B1。
4. **T0.30**：A1 无法准确判定，原因见 B3。
5. **T0.31**：依赖正确；若改 schema 同样受 B1 影响。
6. **T0.32、T0.34**：verify 失败，原因见 B1。T0.33 依赖 T0.28，按文件顺序排在 T0.32 之后，无阻碍。
7. **T0.35、T0.36**：依赖链覆盖 T0.28–T0.34。
8. **T1.1**：依赖 T0.36，阶段门保留；未发现 AUTHORIZED，`frozen_cards` 会拦截，桌面完成后停在等待移动授权。

## 顺延核对

- 旧 T0.28、T0.29 与新 T0.35、T0.36 逐行 diff，除标题外仅依赖行变化（T0.33,T0.34 和 T0.35），内容与验收强度未变。
- T1.1 依赖已改为 T0.36。TASKS 中已完成卡正文未改；`tests/acceptance/t01.py` 的 “T0.28/A1” 由 ADR-057 映射和 TASKS.md:3 说明，足够。

## T0.27 流程记录

- BLOCKED 写明事实、已尝试方案、选项、证据；RESOLVED 标题含卡号，含用户原话和批准摘要 `d441fca1…`，与 rebaseline 事件一致。失败历史和会话额度保留，符合 AGENTS.md 和 WORKFLOW.md。
- 修订提醒：修复上述问题会改变 TASKS.md 和 ARCHITECTURE.md，protected 随之变化。当前决定已被消费（“此用户决定已经使用”），需要用户重新授权，并追加 BLOCKED、RESOLVED 和新一次 rebaseline。
- 应在 T0.27 人工重验之前完成修订，否则 protected 变化会使观察记录失效。

## 建议（非阻断）

| ID | 位置 | 建议 |
| --- | --- | --- |
| S1 | T0.31 范围，`src/data/history.ts:42` | `historyPoster` 只找 stremio 来源，MacCMS 历史在继续观看中没有海报；范围加入该文件 |
| S2 | T0.33，TASKS.md:1969、2022 | 片尾 20 秒显示卡片与 5 秒倒计时的关系未定义；C10/C12 以 EOS 计完成，建议倒计时从 EOS 起算并遵守 `auto_next` |
| S3 | C17 | 4 项新 IPC 未像 C16 那样给出 input/data；建议写明字段、拒绝 URL/cap/Cookie（C12）、搜索多来源时的并发与预算 |
| S4 | T0.32 cases | 工作义务有“迁移失败显式报错并保持旧库可用”，但无对应用例；建议增加失败路径用例 |
| S5 | T0.29，TASKS.md:1722、1725 | 已扫描数、命中数未列入 DTO 字段；“补过一次”标记的存放位置未定 |
| S6 | T0.31，`src/state.ts:716` | `navigate` 同样要求详情来源为元数据源，工作义务只点了 428、435 行，建议一并列出 |
| S7 | T0.33 范围 | Esc 逐级逻辑在 `src/ui/focus.ts`（T0.28 交付），T0.33 范围不含 `src/ui/`；建议加入或由 T0.28 约定注册接口 |
| S8 | T0.28 规格 | `--tv-glass` 同时含颜色和模糊，无法作为单一 CSS 变量，建议拆分；筛选面板、搜索框、下一集卡片圆角 16px 偏离“面板 20px”，建议在共用规格中登记 |
| S9 | T0.32，TASKS.md:1894 | 本机为 macOS 27.0.1，“macOS 14 回退”只能靠模拟 API 缺失；建议写明方法并将真实 macOS 14 记为 NOT RUN。隐藏滚动条建议同时写 `::-webkit-scrollbar` |
| S10 | TASKS.md:1725、1864 | “2026-11-08 实测”的数据在 PROGRESS 中检索不到，建议补记录或注明来源 |
| S11 | 新卡 cases | ARCHITECTURE.md:178 要求 cases 含需求行全部 ID 的同名汇总，新卡只列子用例和 REQ-TVx；建议补 `TV-SHELL` 等汇总项或在 C17 说明 |
| S12 | TASKS.md:3、ARCHITECTURE.md:3 | 状态行仍写 T0.17、T0.19 进度，可顺手更新；T0.35、T0.36 的 “C1..C16” 按“内容不变”保留，由 C17 优先条款解释 |

## 已确认无问题的项

- JUnit 路径唯一，cases 以需求 ID 开头，单项超时 3600 加 verify 5040 秒在 21600 秒预算内。
- IPC 所需的 `protocol/`、`contracts.rs`、`commands.rs`、`host.rs`、`lib.rs`、`build.rs`、capabilities、`src/api.ts` 均在 T0.32、T0.34 范围。
- `src/models.ts` 由 `tests/acceptance/t05_models.py` 生成并校验，`tests/` 在范围内。
- 侧栏入口的归属明确：T0.28 现有入口，T0.30 四个分类，T0.31 综艺/短剧/体育，T0.34 搜索和资料库。
- T0.28 的 Overlay 标题栏可在 `lib.rs` 的 Builder 中设置（`tauri.conf.json` 的 windows 为空）。
- T0.29 的 backdrop 能走现有递归图片登记（`details::images` 按 `resource_id` 遍历），CSP 无需放宽；get_home 路径在 `host.rs` 与 `stremio.rs`，在范围内。
- 引用的代码位置 `state.ts:428/435`、`maccms.rs:1008`、`stremio.rs:469` 与 HEAD 一致；`@videojs/html` 10.3.0 含 hotkey、gesture、slider preview。
- `-webkit-backdrop-filter` 加 View Transitions 回退的处理方向合理；SQLite 当前 `user_version = 1`，升到 2 且 `watch_history` 不变，与 C12/C16 一致；C17 对 C10、C11、ADR-055 的优先关系清楚。

## 实际验证

运行了 `git rev-parse/status/log/show/diff`、`task.py status`、`task.py check`；只读解析了 protocol 两份 schema；核对了 `workflow.py` 的选卡、冻结、completed、scope_problems、rebaseline、失败计数逻辑，以及 `check-contracts.py`、`t05.rs`、`automation.rs`、`t23.py`、`build.rs`、capabilities 和相关前端源码。未修改任何文件。

## 未验证项

- T0.28–T0.34 全部业务验收：NOT RUN（脚本尚不存在）。T0.27 重验 NOT RUN。
- ADR-057 引用的 claude.ai 方案文档未读取。
- 真实窗口能否达到 1920 宽度未实测。
- T0.33 含 50 次切集的 A1 在 3600 秒内是否足够未实测（收据缺少可直接读取的时长字段）。
- TMDB 目录实际筛选项、MacCMS 接口对 year、area 的行为未联网复核。
- 文档 CLEAR 也只确认规划充分，实现结果需由后续验收证明。

DOC-REVIEW-VERDICT: BLOCKING
