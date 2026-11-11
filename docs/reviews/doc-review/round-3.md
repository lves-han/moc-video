# 独立文档审查 round-3

本轮为独立只读复审。审阅者未修改文件、保存报告、启动开发或巡检。

## 受审基线

唯一项目根：`/Users/eason/jp/application`。

Git HEAD：`4efb6a354fb125d26001b5955010f55c951817db`。未提交受审文件以内容摘要绑定。

审前、审后原控制器protected一致：

`66dd658f514e260908070127ffcf180f419bbf88a04460acf89908fcbea59520`

| 文件 | SHA-256 |
| --- | --- |
| AGENTS.md | `1f334de4b44e24595aa085a780eb3b2b5edfbc9131ef62044db30c1a8b730cca` |
| docs/PROMPT-DOC-REVIEW.md | `ac99a36871ae364b09801d36c57c686148f9090b08a3d558668904374cbac869` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `717f04075f1c78f302dae9461d5c38e373208adad77d8a53a82c1ed29eb1c774` |
| docs/TASKS.md | `780157b7d01a66e8e5f3ae55499cfe48d3bbc5a381518b201fe3a7b36cac932d` |
| docs/DECISIONS.md | `686518a958989c8b8e442de61c26ab182c6d50a2936370c64af5ef6b90484514` |
| workflow.json | `e09a371ac0935556695fa2023c163cb25b20be61601193b5a7d3d02ffd50de1f` |
| scripts/check-project.py | `80bfa5a3ad4f5627705039ced9329fdfa7b0ecdb7127bfc536abd0b0a1d0b5ea` |
| scripts/check-contracts.py | `050b17757385b4d7161dc08393c4d01ba0a8f2aba4cefb7f8071662d2721aa78` |
| docs/SOURCE_MANIFEST.json | `8e92ab4f72a498d5c64a05dbac23c5839197021423e02edf575ef5838c4d127a` |
| README.md | `5cea268d407c2c0fab24191d21a6d47d31456a4c21acab44a40bd0f98eded5dc` |
| protocol/app.schema.json | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` |
| protocol/bridge.openapi.json | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` |
| protocol/examples/contract-cases.json | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` |

已采纳ADR分段摘要：

- ADR-001：`9377c96f9791c3416582254a4df6fef38676e67f0f2e1b780a99e737b80b7f21`
- ADR-002：`52d26c5e832ad756de1a9b5a4655759a270facf46b333bd1c9f149a34c864fa9`
- ADR-003：`0372eebdfa8a92e0536f37ead4f4ba87f28e7b5aaa852f89308d036abf708975`
- ADR-004：`f6a275ac10ea52edcb1ebe95df61ca6ebe36f1ca7e8ab80d1dba66fe09bdb33a`

分段摘要按当前标题起至下个ADR标题前的原始UTF-8内容计算，包含分隔空行。

## 需求覆盖与路线结论

规划覆盖充分，当前版本通过文档审查。

35张原卡正文完整保留，一对一映射、需求覆盖、反向追溯与验收入口齐备。桌面29卡和后续移动6卡保持原阶段要求。U01..U09、原7包及全部注册provider、真实三平台、跨源匹配、媒体代理、取消与恢复、SQLite、双语与外观、发行及移动MG均有准确落点。

路径、venv、原控制脚本、证据及共享锁绑定得到保留。性能指标、单位、负载、时长、平台与目标强度保持原义；资源预算起点与待实测项有明确记录。真实集群的Docker部署、每容器硬限额、容器限额合计与宿主RSS预算、实际采样及自产资源清理纳入对应验收。

round-1的依赖顺序、修改范围、工作协议持续验证及报告路径问题均已解决，修订在当前版本继续有效。

## round-2问题复核

### R2-B1：已解决

C16现在由Session独立保有已确认元数据、video、播放来源及match绑定。活动Session在MatchRecord及候选回收后仍可接纳合法历史快照。

快照接纳与数据库提交资格已分开。已接纳快照保有确认身份至提交结束，页面变化、临时match回收及旧Session关闭均保持其合法提交资格。切集、切源、离页和退出的最终快照顺序得到明确说明。旧Session关闭后新发起的未接纳请求受到拒绝。

单调序号、删除/清空屏障、同序号同载荷重试和不同载荷拒绝规则继续成立。T18明确要求超过5分钟并强制GC、延迟最终快照、旧事件、错误归属及删除屏障反证。

### R2-B2：已解决

`match_id`复用既有长度1..160不透明字符串格式；新增`session_id`准确复用现有`PlaybackViewSession.session_id`格式。T18加入合法非UUID句柄贯通及空、超长、未知、归属错误负例。

ADR-004说明修订来源及影响，保持原17命令和原Session所有权。

## 实际验证

使用`/Users/eason/jp/application/.venv/bin/python`：

1. `-B scripts/task.py status`：审前审后protected一致，当前卡为空。
2. `-B scripts/task.py check`：`roadmap OK: 35 cards`。
3. `-B scripts/check-contracts.py`：退出0，17命令、179结构用例、89语义用例、6生命周期向量。
4. 归档manifest：16/16摘要相符。
5. `docs/SPEC.md`与归档开发规格逐字节一致。
6. 35张原卡正文保留检查：35/35。
7. 当前工作协议、验证脚本及配置摘要与round-2一致，复用前轮独立审查；本轮重新审查C16、T18验收和ADR-004。

本轮未重复运行完整verify、慢速控制器测试或前端构建。外层代理执行的验证结果应继续引用其真实日志及收据。

## 未验证与开发前提

业务实现验收均为 **NOT RUN**。四个历史IPC属于T18待实现交付物，当前机器契约保持原17命令。Rust/Tauri、Android原包执行、真实来源、SQLite运行、三平台媒体/安装、移动真机与长性能验收均待相应任务实施。

开发CLI、独立巡检CLI、模型、权限、预算及平台访问仍待开发阶段核实。当前无开发授权与定时监管。文档CLEAR确认规划与验收定义充分；实际完成状态由后续真实业务验收和原工作流收据确定。

DOC-REVIEW-VERDICT: CLEAR
