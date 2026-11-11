# 文档审查报告 round-50：T0.27 简化验收与 T0.36/A2 生命周期（ADR-058）

## 受审基线

- Git HEAD：`dc49b61e048b9855c85a03e027455302f73e8c29`，工作树干净。修订范围为 `2877cb7..dc49b61`，涉及 ARCHITECTURE、DECISIONS、TASKS。
- `task.py status` 的 protected：`3cb33da3ae5e54f7f607793e55fb9aff8a1627bf28f476562a1b7152d63ed137`。
- `state.protected`：`d441fca11eb084cd9b4e82156ff1e3e1c867de608b66187879689ff03d07572a`（待 rebaseline，属协调方说明的预期状态）。
- ADR 分段摘要：ADR-058 `bdd5c7152f610298ccb8f8bd7b48bfda1652263f80b0e592682f00e72b2abab3`；ADR-057 `5b8dae50baade73ea4b9f6217ef21d8f42636df4f4a4512ca75fc478d1898d1a`（未变）。
- current=T0.27，blocked=`T0.27`。`task.py check`：`roadmap OK: 42 cards`，退出 0。

## 结论

ADR-058 的需求移动合理，T0.27/A1 可完成，N1、N3、N4 正确。存在一项阻断：T0.36/A2 沿用的 lifecycle 判据与 C17、T0.29 的最终产品矛盾，且发现时已晚于用户的人工观察。

| 用户要求 | 落点 | 结论 |
| --- | --- | --- |
| T0.27 重验不走 UTM，简化验收标准 | T0.27/A1 `--suite package` | 已落实，不启动窗口或虚拟机，无人工步骤 |
| 真机或虚拟机测试在 T0.36 做 | T0.36/A2 `--suite lifecycle` | 卡位正确，判据需修订（B1） |

## BLOCKING 问题

### B1 T0.36/A2 的 lifecycle 判据与最终产品矛盾，首次发现时须重做人工观察

- 需求 ID：U04、R30、R50、REQ-T25-LIFECYCLE；C17 数据条款；TV-SOURCE-UPGRADE。
- 位置：TASKS.md T0.36 工作义务（`t23.py --suite lifecycle` 一条）；ADR-058“原有的……门槛不变”；`tests/acceptance/t23.py` 的 `lifecycle()`、`check_lifecycle()`、`artifact()`；`packaging/release.py:316`。
- 因果链：
  - `lifecycle()` 对 `upgrade_before`、`upgrade_after`、`uninstalled` 都要求 `PRAGMA user_version == 1`。T0.32 按 C17 把数据库升到 2，最终构建的升级后和卸载后数据库必然是 2，A2 必然失败。
  - `check_lifecycle()` 要求升级前后设置除 `window_bounds` 外完全相等。T0.29 新增 `Settings.metadata_preset_applied` 并可能补入 TMDB 配置，升级后写入即失败。
  - `release.py:316` 写死 `card="T0.27"`，`artifact()` 校验 `manifest["card"] == "T0.27"` 和构建收据 `card == "T0.27"`。T0.36 工作义务只写“资源归属使用当前卡 T0.36”，没有覆盖 manifest 和构建收据两处。
  - `tests/` 和 `packaging/` 都在 source 摘要内，观察记录又绑定 manifest、source、protected。若到 A2 运行时才改判据，源码变化会使刚完成的 VM 观察和用户声音确认作废，用户须重做一次。
  - ADR-058 写“门槛不变”，开发者据此无法判断改写这些断言是否属于弱化既有断言。
- 影响：A2 按现文档无法通过；修复时机错误时人工步骤重复。
- 最小修订（T0.36 工作义务增加一条，并在 ADR-058 加补充说明）：
  - 在 `release.py --build` 和人工观察之前，先把 lifecycle 判据更新为最终产品值：`upgrade_before` 的 `user_version` 为 1，`upgrade_after`、`uninstalled` 为 2。
  - 设置比较增加排除 `metadata_preset_applied`，并规定升级前配置已含 Stremio（不触发补 TMDB），其余字段仍逐项相等；`watch_history` 逐行相等不变。
  - manifest 的 `card`、构建收据和资源归属在 lifecycle 路径统一为 T0.36；同步 `test_t23.py` 正负例。
  - 声明上述修改属于跟随 C17、T0.29 的预期值更新，判据强度不变。

## 核实项

### 1. T0.27/A1 package

准确、可完成。

- 现有 `environment`、`artifact`、`payload`、`archives`、`signing`、`oracles`、许可检查都不需要启动窗口；`artifact()` 的 card 与 `resources(card="T0.27")` 在本卡一致。
- 前置：rebaseline 后需重新运行 `diag-t23-release-build`，使 manifest 绑定当前 source、protected、generation；T0.26 前序 app 和报告需仍在 evidence 中（`archives()` 校验）。
- 卡内义务足以约束结果：“只输出 U04、R33、R45、REQ-T23 及具名行为用例”决定了 `REQUIREMENTS`、`IDS` 按 suite 切分。`BEHAVIORS` 若未去掉 `clean-macos-lifecycle`，package 的 `required-behavior-inventory` 用例自身会失败，具备自校正。
- 未明文项见 N1。

### 2. T0.36/A2

除 B1 外可行。

- 预算：A1 7200 + A2 7200 + verify 5040 = 19440 秒 ≤ 21600 秒；单项不超过 `command_seconds`。
- 绑定：`observations.json` 通过 manifest 摘要、source、protected 绑定，旧观察会被拒绝；`T23-current.json` 由 T0.36 的构建覆盖；`receipt["protected"]`、`generation` 的比较在同一卡内成立。
- 范围：`packaging/`、`src-tauri/`、`src/`、`tests/` 足够修复缺陷及更新判据。
- 闭环：T0.36 按 run/verify/done/完成提交推进，之后 T1.1 由阶段门拦截。人工步骤有用户来源（ADR-058），T0.36 不能无人值守完成，需用户到场。

### 3. 需求覆盖

- U04：T0.27/A1 保留包级覆盖，真实安装由 T0.36/A2 承担。R30、R50 移至 T0.36/A2，且保留 T0.2、T0.12、T0.35 等归属，无虚假覆盖。
- REQ-T25-LIFECYCLE 行与 A2 的需求行和 cases 对应。
- T0.35 新增说明与 A1 不矛盾；A1 描述仍含“安装”字样，由正文说明限定（N3）。
- T0.35 的现有脚本 `t24_memory.py` 未引用 T23 产物，依赖链 T0.28–T0.35 无卡需要 T0.27 的生命周期证据；T0.36/A1 所需安装包由 T0.36 自身构建。

### 4. 控制器语义

- 当前 blocked 含 T0.27，`next_card` 会冻结 T0.27 及其依赖卡，符合预期。
- `rebaseline` 取 T0.27 决定记录的最后两条，现有最新为 BLOCKED，追加 RESOLVED 后满足条件；批准摘要必须等于当时的 protected（当前 `3cb33da3…`）。`generation` 递增使旧收据失效，失败历史和额度保留。
- `done` 的 scope：`src/` 四个文件、`tests/`、`packaging/` 均在 T0.27 范围内，文档为 DYNAMIC 或 rebaseline_scope。卡片 `[x]` 保留不影响，`completed` 依赖新的 done 收据。
- T0.27 完成后下一张为 T0.28。
- 待执行前提（不判 BLOCKING）：取得用户授权，并在 RESOLVED 决定中同时覆盖 round-48 修订和 ADR-058，因为现有 BLOCKED 只写了 round-48；修复 B1 后 protected 还会变化，批准摘要须取修复后的值并重新审查。顺序本身合理。

### 5. N1、N3、N4

- N1：C17 已写入 `Settings.metadata_preset_applied`，正确。
- N3：T0.30 已写“自动清除排序并提示”，与 A1 一致，正确。
- N4：T0.30 A1 已增加拒绝条件，正确。

## 建议（非阻断）

| ID | 位置 | 建议 |
| --- | --- | --- |
| N1 | T0.27 工作义务 | 明写 `t23.py` 的 `DEFERRED.reproduce_argv` 改为 `--suite package`，并说明 `--suite all` 是否保留；`t14.save()` 写死 `suite="all"`，package 报告需能标明实际 suite；`save()` 的 limitations 文案改为生命周期由 T0.36/A2 执行 |
| N2 | T0.36 工作义务 | 新观察覆盖 `evidence/releases/T23-clean-macos/` 前先保留旧材料，符合 AGENTS.md“保留所有失败及未完成证据” |
| N3 | T0.35/A1 描述 | 下次修订时把“安装”改为指向 ADR-058；独立验收报告列明生命周期待 T0.36/A2 |
| N4 | 覆盖表 U04、REQ-T23 行 | 注明 T0.27/A1 为包级、原 T23 的生命周期义务由 REQ-T25-LIFECYCLE 接续 |
| N5 | T0.36/A2 | 提醒：观察记录包含用户声音确认，需提前通知用户安排真机或虚拟机 |

## 实际验证

运行了 `git rev-parse/status/log/diff`、`task.py status`、`task.py check`；通读 `tests/acceptance/t23.py` 全部 411 行，核对 `packaging/release.py`、`packaging/T23.md`、`t14.Acceptance`、`t10.resources`、`test_t23.py`、`t24_memory.py` 引用和 PROGRESS 末条记录。未修改任何文件，未运行写操作命令。

## 未验证项

- T0.27 package 重验、T0.36/A2 及全部 T0.28–T0.36 业务验收：NOT RUN。
- `t23.py --suite package` 和 `--suite lifecycle` 尚未实现，本轮按卡内义务和现有代码推演。
- T0.26 前序 app 与报告的当前完整性未逐文件复算。
- 真机或虚拟机的可用性由用户在 T0.36 提供，本轮未核实。
- 文档 CLEAR 也只确认规划充分，实现由后续验收证明。

DOC-REVIEW-VERDICT: BLOCKING
