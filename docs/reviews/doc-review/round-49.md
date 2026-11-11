# 文档审查报告 round-49：桌面 Apple TV 化（ADR-057）复审

## 受审基线

- Git HEAD：`b98473d9d32b63195cf616e30c2cdc5dd0a4336b`，工作树干净。修订范围为 `93897b0..b98473d`，涉及 ARCHITECTURE、DECISIONS、PROGRESS、TASKS 和 round-48 报告；`scripts/`、`workflow.json`、`AGENTS.md`、`docs/SPEC.md` 未变化。
- `task.py status` 的 protected：`c20a76916c58a9c072fa655d3b73eb4aec45bfbcbfda25c71af06cc780e0133e`。
- `state.protected`：`d441fca11eb084cd9b4e82156ff1e3e1c867de608b66187879689ff03d07572a`，与现行摘要不同，属协调方说明的预期状态。
- ADR-057 分段摘要（含第三条补充）：`5b8dae50baade73ea4b9f6217ef21d8f42636df4f4a4512ca75fc478d1898d1a`。
- current=T0.27，blocked 为空。`task.py check`：`roadmap OK: 42 cards`，退出 0。

## 结论

round-48 的 B1–B6 均已解决，修订未引入阻断性矛盾或覆盖缺口。规划足以进入下一步，业务验收全部 NOT RUN。

| 问题 | 修订 | 判定 |
| --- | --- | --- |
| B1 契约脚本与 t05.rs 范围 | T0.29/T0.31 加入 `check-contracts.py`，T0.32/T0.34 加入该脚本和 `src-tauri/tests/t05.rs`；工作义务写明按卡收紧，t05 总数 26、27 | 已解决。`scripts/check-contracts.py` 不在 PROTECTED，加入范围即可修改；t05.rs 无 MediaCard/Settings 形状断言，T0.29 无需改它 |
| B2 验收宽度 | `automation.rs` 进入 T0.28 范围，白名单增加 1512，宽度改 960/1200/1512，断点改 1440 | 已解决（见决定 1） |
| B3 地区与排序 | `sort` 四值及目录映射，TMDB 地区禁用，维度可用性声明，禁用维度明确报错；新增 `sort-routing`、`facet-availability` | 已解决（见决定 2） |
| B4 年份、类型推荐 | 首页和分类页默认内容排写明按类型、按年份及目录来源；新增 `TV-HOME/genre-year-rows` | 已解决 |
| B5 MediaRow 验收 | T0.28 写明夹具边界，T0.30 新增 `TV-HOME/media-row-spec` 做真实页面核对 | 已解决 |
| B6 安全断言 | T0.28、T0.32、T0.34 新增 `permissions-exact`，权限集合精确相等，CSP 与基线一致 | 已解决 |

## 作者决定核实

### 决定 1：1512 宽度与 1440 断点

足够让验收可达，也未虚假覆盖。

- 可达性：1512 进入白名单后可由现有 `t01_control` 的 `width` 动作设置。逻辑宽度来自作者在 PROGRESS 记录的 Finder bounds，本轮 `system_profiler` 的 3024×1964 Retina 与之相符；未单独复测。
- 断点：1200 落在 <1440 区间，1512 落在 ≥1440 区间，两侧都有真实窗口断言。TASKS 中已无残留 1600；1920 只在 T0.28 工作义务中记为 NOT RUN。
- 已知未测部分（归入 1920 NOT RUN）：横版卡 360px 上限从约 1636px 起生效，竖版 220px 上限从 1760px 起生效，Hero 字号 72px 上限从约 1636px 起生效。这些上限在 1512 下取不到，文档也未声称已覆盖。

### 决定 2：排序、地区与 T0.30 范围

可判定，T0.30 不需要范围外文件。

- `sort` 四值、目录映射、“选语言或年份时排序不可用”、剧集仅趋势和最新、TMDB 地区禁用均可观察，并有用例和“请求禁用维度明确报错”的反向判据。与 TASKS 原有“仅类型时电影用 `tmdb.top`、剧集用 `tmdb.latest`”的默认路由不冲突。
- T0.30 改写后的 A1 与禁用规则一致，“五个胶囊可组合”的矛盾已消除。`FilterBar.vue` 在 `src/components/`，`BrowseView.vue`、`state.ts`、`App.vue` 在范围内；协议和 `models.ts` 由 T0.29 提供，`api.ts` 的调用入口是泛化的。

### 决定 3：待执行前提（不判 BLOCKING）

顺序本身合理。按控制器语义，现行 protected 与 `state.protected` 不一致时 `run.py` 的 `check_frozen` 会拒绝运行，因此必须先 rebaseline。`rebaseline` 所需条件在以下顺序下全部可满足，且 T0.27 的改动文件仍在范围、DYNAMIC 或 `rebaseline_scope` 内。

待执行步骤：

1. 取得用户对本轮修订的重新授权。round-48 的修订尚无用户批准记录，旧决定已被消费。
2. 追加 T0.27 的 BLOCKED，再追加 RESOLVED，批准摘要须等于当时的 protected（当前 `c20a7691…`）。
3. 执行 `task.py rebaseline T0.27 --protected <同一摘要>`。
4. 重验 T0.27：`t23.py` 要求人工观察的 source、protected 与 manifest 一致，需重做专用 UTM 生命周期观察并由用户确认声音；失败后的 `task.py note` 规则继续适用。

在 RESOLVED 之前，追加 round-49 报告和 PROGRESS 记录不会改变 protected；对 TASKS、ARCHITECTURE、protocol 等冻结或受审定义的任何再修改，都会使本次 CLEAR 失效。

## 新发现（非阻断，无需在开工前修改）

| ID | 位置 | 说明与处理建议 |
| --- | --- | --- |
| N1 | ARCHITECTURE.md C17“契约检查”；TASKS.md T0.29 工作义务 | C17 写“只放行本节列名的可选字段”，`Settings` 的 `metadata_preset_applied` 只在 T0.29 列名。按 T0.29 明确列出的四个 DTO 实现即可；下次修订冻结文档时顺带把 Settings 写入 C17 |
| N2 | `src-tauri/src/stremio.rs:428`；T0.29/T0.30 | 含 `isRequired` extra 的目录在同步时被跳过。若 `tmdb.trending`、`tmdb.year`、`tmdb.language` 的 genre 为必填，这些目录当前不会成为栏目，T0.29 需保留可路由的目录登记（`stremio.rs` 在范围内，A1 的请求断言能发现遗漏）。实现时先核对真实 manifest |
| N3 | T0.30 FilterBar | 已选排序后再选年份或语言的交互未写明，手写 URL 组合也会触发禁用维度报错。实现时清除排序或显示明确提示，并以现有 `filter-bar-combination` 用例覆盖 |
| N4 | T0.30 A1 描述 | `genre-year-rows` 已冻结，但描述未写对应“会被拒绝”项。用例足以约束，文字可在下次修订时补 |
| N5 | S12（round-48） | 状态行未更新，仍为非阻断项 |

S1–S11 的采纳已核对（`history.ts`、EOS 后倒计时、C17 的 IPC 字段、迁移失败用例、`scanned_count`/`matched_count`、`state.ts:716`、`src/ui/` 进入 T0.33、`--tv-glass-filter`、圆角登记、View Transitions 夹具、PROGRESS 实测记录、需求汇总 cases），未引入矛盾。`poster_cache_key` 为现有 watch_history 字段（SPEC:645、`history.rs`），与 C17 的待播输入一致。

## 可行性推演（修订后）

1. T0.27：按“决定 3”完成重新授权、rebaseline 和人工重验后可完成。
2. T0.28：1512 可达，完成条件满足。
3. T0.29：协议改动与收紧的 `check-contracts.py` 同卡，verify 可通过。
4. T0.30、T0.31：依赖和范围满足。
5. T0.32、T0.33、T0.34：命令追加集和 t05 按卡递增，权限精确断言按累积期望。
6. T0.35、T0.36：依赖和内容不变。
7. T1.1：依赖 T0.36，阶段门保持，桌面完成后等待移动授权。

未发现路线上第一张无法完成的卡。

## 实际验证

运行了 `git rev-parse/status/log/diff`、`task.py status`、`task.py check`；逐行审阅 `93897b0..b98473d` 的 TASKS、ARCHITECTURE、DECISIONS、PROGRESS 修订；核对 `check-contracts.py` 全部断言、`t05.rs` 相关断言、`stremio.rs` 目录过滤、`history.rs` 字段和残留宽度。未修改任何文件，未运行写操作命令。

## 未验证项

- T0.27 重验及 T0.28–T0.34 全部业务验收：NOT RUN。
- 1920 宽度：NOT RUN（作者决定，需外接显示器）。
- 真实 macOS 14：NOT RUN（本机 macOS 27.0.1，由禁用 API 的夹具覆盖）。
- 窗口实际达到 1512 未在本轮运行；TMDB 真实 manifest 的必填 extra 未联网核对；ADR-057 引用的 claude.ai 方案文档未读取。
- 本结论只确认规划充分，实现由后续验收证明。

DOC-REVIEW-VERDICT: CLEAR
