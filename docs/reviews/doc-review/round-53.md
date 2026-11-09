# 文档复审报告 round-53：ADR-059

## 受审基线

- 项目根目录：`/Users/eason/jp/application`
- Git HEAD：`7f7a26ee732e32f43bebaf06713a448a56fa463a`；工作树干净。
- `task.py status` 的 `protected`：`656ef0e94d4e126541a141ffd59171bc3dd9427950babdc010eee48b3e15024e`。
- ADR 分段摘要：ADR-057 `5b8dae50baade73ea4b9f6217ef21d8f42636df4f4a4512ca75fc478d1898d1a`；ADR-058 `0b5d21df283072c98822e489c7e530a1250ccc25f7eb396999be40d1d3fba5c3`；ADR-059 `9b1a90a73b659e0f4f65ce5b90f74c57ed63b9c0743fa5f4f84c8c555b374914`。本轮未修改 ADR；ADR-001 至 ADR-058 的原文相对 `7f8bdc3` 保持不变，完整内容固定于受审 Git 版本。
- 当前卡 T0.33，T0.30 为暂停卡；两卡旧基线相同，均为 `d26c7b3c0e7bd447cbf049df27b05877ea25b037e5d3c8f33738751934e9dfd5`。

## 结论

**CLEAR。** round-52 的三项阻断及空值建议已解决，本轮修订未发现新的阻断矛盾。结论针对 ADR-059 及其受影响数据流、验收和原控制器迁移路径；业务实现仍须通过各卡验收。

## round-52 问题复核

| 项目 | 修订及判断 |
| --- | --- |
| B1：栏目描述与真实空结果混淆 | C17 明确描述阶段为待加载，真实首请求完成后才判断空结果；T0.30 新增 `TV-HOME/catalog-pending`，覆盖零内容请求、可见栏加载、空态、失败重试和来源切换。已解决。 |
| B2：同引用多筛选排互相覆盖 | C17 和 T0.30 明确以来源及修订、语言世代、栏目引用、规范化筛选条件区分状态；`TV-HOME/filtered-row-isolation` 要求真实 Host 与 fixture 覆盖并发、乱序、分页、重试及来源/语言切换。已解决。 |
| B3：恢复定位缺少准确验收 | T0.33 新增 `TV-PLAYER/resume-seek-race`，要求非零数量的零位置竞争、非零/超时长恢复、旧会话隔离回归及真实窗口暂停检查。已解决。 |
| 空值形式 | C17 和 T0.30 明确 `section_ref`、`filter` 缺省与 `null` 等价，`filter: {}` 在 `catalog_only=true` 时拒绝，并要求正反例。已解决。 |
| 非 Stremio 来源 | 保持明确拒绝 `catalog_only=true`，T0.31 使用该来源支持的路径。已明确。 |

原 T0.30 的 16 项、T0.33 的 11 项 cases 均保留；分别新增 2 项和 1 项。准确 argv、超时、原平台与资源门槛未改变。

## 基线迁移前提

只读核对显示：当前 T0.33 活跃、T0.30 暂停；未完成卡集合恰为这两张，旧 protected 相同；两卡追加文档前缀完整，当前无未结束重命令且重任务锁未占用。T0.33 的范围外变更仅为受保护的 `docs/ARCHITECTURE.md`。

继续前仍须按真实用户决定追加相应 RESOLVED，绑定当前 protected，并在 T0.33 的决定中准确记录 `迁移暂停卡：T0.30`，再通过原 `rebaseline --include-paused` 入口迁移。原失败、会话数和暂停状态须保留，受影响验收重新运行。

## 实际验证与限制

实际执行了 Git 状态、固定版本及差异检查，`task.py status`、`task.py check`，并只读核对原控制器的未完成卡、范围、追加前缀、收据及锁状态。`task.py check` 返回 `roadmap OK: 42 cards`；`git diff --check` 通过。

T0.30/A1、T0.33/A1 和完整业务验证在本次文档复审中均为 **NOT RUN**。本报告复审 round-52 对当前修订提出的阻断项，并沿用此前固定版本审查对未受影响历史需求的结论；未重新逐条独立验证全部历史需求。

DOC-REVIEW-VERDICT: CLEAR
