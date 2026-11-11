# 独立文档审查 round-7

本轮为独立只读复审。审阅者读取当前 AGENTS、SPEC、ARCHITECTURE、TASKS、DECISIONS、工作流配置、启动与审查提示词、工作协议及既往 round-6 报告，核对配置与测试夹具变更。审阅者未写入项目文件、勾卡、提交、启动开发/驱动/巡检，未运行 validate、run 或完整 verify；实际运行结果通过外层生成的日志和原始收据核验。

## 受审基线

- 唯一项目根：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`，Python 3.12.14。
- Git HEAD：`df51163c43a5f235b8ccd4da9cfcceb8a97bdf84`。
- 未提交定义变更：`workflow.json`、`scripts/tests/test_workflow.py`、`docs/DECISIONS.md`，合计新增 12 行。
- 审前及审后 `task.py status` 的 protected 均为：`bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a`。
- current=null，state、blocked、runs 均为空；35 张业务卡均未开始。
- 原 round-6 CLEAR 绑定旧配置，本报告重新绑定当前定义。

ADR 分段摘要：

| ADR | SHA-256 |
|---|---|
| ADR-001 | `31756257a1974108f9c1c8426c9bcb8f80128e2b867e9703f4b766b2e1754407` |
| ADR-002 | `d67dcfe2ddb2fd858acd1b1f4cb17440fa47dc15b4948811b1018d062b52e368` |
| ADR-003 | `26e4e12c4ae38b3ddcfbea18c04a1602799586a08610125f66567ae8dd516c5c` |
| ADR-004 | `a08db017049e19a1481fa314938b5eb88dfcb5598287af24e70bd93058c6eaae` |
| ADR-005 | `df0b0d68cd7a4595bd621ce153ba914da82496311c112089fd5b843d8685606d` |
| ADR-006 | `335827cabfced06c4cc2e2fc99978940e01818abbd7af6aedd12a66161b883c1` |
| ADR-007 | `028d7df4ecd18afa281cb45703aaf4e26857f114163be10b0c2d8860d718b227` |
| ADR-008 | `9c1b6bc1afa7ae46d9c714f2da6ff25775bb4f84fa2a894666823fabbc9f00cc` |
| ADR-009 | `2af85af3e9a0d33cd2fe5088a27bc3075ed73986e2e6da5b904129753f6e62c6` |

主要受审文件摘要：

| 文件 | SHA-256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `bc0cf0f076e33bf1684f66ece496c6792bc21333c2f9af6e0fbe8ece6d571ac5` |
| docs/TASKS.md | `4624055b45ad42e092feed09aa793a954066ee835f1a8c3c5247b13fbfdafa84` |
| docs/DECISIONS.md | `0c0eb5342b58c5856413398cded023fd27015ae4210b9072c9113ce5029bf65c` |
| workflow.json | `d28b4ef54bb71c7cc3720c90e3233bb9922ca2560a06a2f01750a73ed3d1cb31` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| scripts/tests/test_workflow.py | `596487772cb4f71cdd015c34fc18ed86f6a8bfec76ff57dc4178fee9c9e1378d` |
| docs/PROMPT-START.md | `f72455f542af1f92f63c641a342cbcec286b49479356dd56c444612eb39e1d0a` |
| docs/PROMPT-DOC-REVIEW.md | `399371107d92e4172312182ba7e888fbe41f377a35c57d6109ccb852ef7382c0` |
| docs/PROMPT-SUPERVISE.md | `2089bb44078270100a9299d4de55cdee62bb43c3a88a3253ef382abed020f88a` |
| scripts/check-contracts.py | `050b17757385b4d7161dc08393c4d01ba0a8f2aba4cefb7f8071662d2721aa78` |
| scripts/check-project.py | `80bfa5a3ad4f5627705039ced9329fdfa7b0ecdb7127bfc536abd0b0a1d0b5ea` |
| protocol/app.schema.json | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` |
| protocol/bridge.openapi.json | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` |
| protocol/examples/contract-cases.json | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` |
| docs/SOURCE_MANIFEST.json | `8e92ab4f72a498d5c64a05dbac23c5839197021423e02edf575ef5838c4d127a` |

## 变更与授权审查

### 两条命令

当前配置分别为：

```json
["codex","exec","-m","gpt-6.1-sol","-s","workspace-write","--color","never","-"]
["codex","exec","-m","gpt-6.1-sol","-s","read-only","--color","never","-"]
```

模型有当前聊天用户“再次尝试一下 模型使用`gpt-6.1-sol`”的直接来源。正式开发有“现在正式开发任务”的直接来源。ADR-008/009保留两项来源、默认范围、预算和停止条件，未将规划资料中的命令记作人类授权。权限参数保持开发 workspace-write、巡检 read-only。

现有 CLI 自检均退出 0。开发项目临时文件写入、读取、删除均成功；真实 `.git` 临时文件创建被沙箱拒绝，日志包含准确 PermissionError。PROMPT-START 第2步已经规定此情形由外层默认 Git 代办收尾，当前处理与该规则一致。

### 一行测试夹具修订

位置：`scripts/tests/test_workflow.py:148`，`WorkflowTests.setUp` 的临时仓库配置增加 `review_command=None`。

因果链清楚：setUp复制真实 workflow.json，原实现只替换开发命令；配置真实巡检后，原本验证“无巡检”的测试继承真实巡检命令，出现模型调用及预期错误消息偏移。新增赋值使临时仓库明确从“无巡检”状态开始；需要巡检的测试继续通过 `enable_review`（约第983行）配置本地 reviewer 替身。测试断言、用例数量和运行控制逻辑保持。

这是启动前的验收隔离修复。current=null，尚无业务卡冻结基线；PROMPT-START 第3步明确允许外层处理验收可执行性问题后重新复审。ADR-009准确说明这一处理依据，并将作者解释与用户逐字指令分别记录。未发现借此更改业务需求、弱化验收、扩大权限或重置状态的问题。

## 需求覆盖与契约结论

当前 SPEC、ARCHITECTURE、TASKS、AGENTS、工作协议及运行控制器摘要与 round-6 对应文件一致。新增内容集中于启动命令、测试隔离和授权记录，原产品定义保持。

逐项核对结果：

- 113 条需求覆盖记录具有有效卡号及验收 ID；各卡需求行均可反查覆盖表。
- 35 张卡、36 项验收；每项冻结 cases 与该卡需求 ID 列表完全一致，均使用同次 JUnit 报告，未配置 skippable。
- 29 张桌面卡覆盖原 U01..U09 的当前桌面范围，6 张移动卡保留完整后续要求。T1.1具有真实阶段门，其余移动卡通过依赖冻结。
- macOS arm64为必需平台，x86_64、Windows、Linux的降级具有ADR-005/006中的真实用户来源。SPEC及原始义务原文保留，TASKS总范围与卡内环境字段统一解释。
- 原17命令、DTO、请求预算与协议归属保持；T18按C16补四个历史命令、样例及SQLite实现的顺序明确。
- Stremio元数据→匹配→Cloudstream主播放、MacCMS可选分支、宿主/客体世代、会话独立保有、设置写队列、历史删除屏障及真实SQLite跨重启均有具名任务和反例。
- T06..T11使用受控HTTP与合成接口，真实原包及拦截器由N02接入并由T20/T21最终复测；前端入口、宿主、网关、存储与最终发布证据之间的数据流能够追溯。
- SPEC与归档DEVELOPMENT_SPEC逐字节相同；SOURCE_MANIFEST的16个原始输入摘要全部匹配；实际protocol的三份基础文件与归档逐字节相同。
- HLS/DASH、Range、字幕、SSRF、取消、旧结果、异常、大小限制、设置/SQLite错误及原包事务均有可观察反证。当前未来验收脚本尚待逐卡实现，真实断言仍需每卡独立巡检核对。

未发现新的需求遗漏、无来源扩展或未经批准的范围删减。

## 路线可实施性与运行约束

按原控制器真实函数只读推演：

- 当前next为T0.1；仅T1.1及其依赖的五张移动卡冻结。
- 模拟T0.19被阻塞时，其依赖分支冻结，T0.1..T0.17的HTTP分支保持可选。
- 模拟桌面全部完成时，T1.1..T1.6仍因移动阶段门冻结。
- 第一张由现有定义必然等待人类新授权的卡为T1.1。

本机可见Rust、Node/npm、Java、Docker、Xcode与Android SDK；现有镜像为API34，API35按T00/T02/T04固定并实测。SDK获取、在线provider可用性及长测实际资源消耗仍在后续卡核验。环境缺失按已有分支级阻塞规则处理。

项目根、venv、原scripts、driver锁及evidence绑定一致。heavy_lock=null实际解析为`/Users/eason/Library/Caches/agent-project-workflow/heavy.lock`，保持原全局共享路径。

资源与性能定义保持：

- 4096MiB、jobs=2、测试线程1、10800秒/会话、每卡2会话；完整verify配置超时合计5040秒。
- 最大单卡“验收超时合计+verify”是T0.26的19440秒，小于总额度21600秒；原控制器budget_problems返回空问题及空提示。
- 1.5秒窗口、100ms交互、本地桥接p95低于100ms、20次冷启及60秒目标、1080p30连续30分钟、20Mbps连续10分钟、50次切集等指标、单位、规模与来源保持。
- C14将macOS资源后端准确写为RSS采样；AVD的2核3072MB保留为预算起点。实测超出预算时仍须按既有授权流程处理。
- 真实集群Docker服务成员、每容器硬内存、容器硬限额合计加宿主RSS、实际用量与自产资源清理，在C14、T21及M04验收中明确。当前未预设未经批准的集群拓扑或容器预算；普通fixture、桌面应用与Android AVD按已有部署类型分别记录。

## 非阻断执行提醒

1. **长卡预算实测。** T0.26总额检查通过，但单会话装箱仍依赖实际时长；A2超时上限7200秒加完整verify上限5040秒可能超过单会话10800秒。首次Rust及AVD实测应保留时长/RSS，按完整验收边界交接，额度及门槛沿原流程处理。
2. **汇总cases粒度。** 冻结的是需求ID汇总名，逐卡独立巡检仍须将全部必需子用例反查原始义务，核对汇总成功条件。
3. **T24审阅时机。** T24验收需要固定提交的独立报告先存在。到达该卡时，外层应依据既有独立审查授权，先取得固定源码的只读报告、保存到指定round目录，再续卡进行该项验收；完成提交后的常规巡检按原驱动另行执行。
4. **启动收尾。** 本报告须由外层原样保存并提交；真实heartbeat建立成功、工作树整理完毕后再启动原`drive.py --once`。

## 实际验证

审阅者实际执行：

- 原`task.py status`审前、审后核对protected及9段ADR，一致。
- 原`task.py check`：退出0，`roadmap OK: 35 cards`。
- 只读Python核对113条覆盖引用、36项cases/JUnit、预算、阶段门及分支冻结语义。
- 16个原始输入摘要、SPEC归档一致性、protocol逐字节一致性及受审文件SHA核对。
- 阅读测试setUp、enable_review、相关失败测试及当前差异；运行控制器文件摘要与round-6相同。
- 核对开发/巡检自检原始日志和退出码JSON。
- 核对外层生成的`evidence/drive/formal-start/validation.json`：PASS，116测试、109通过、0失败、7平台条件跳过。
- 核对完整verify收据`evidence/verify/20261026T102513614415.json`：finished、exit=0、generation=0，protected与当前受审基线相同，source为`0ddfe49546587b4c031a5e9648deec3d75054d1bc460af1c5023fb15059e9d46`。
- 六份verify子收据均finished/exit=0、source/protected/generation一致；通过原只读`verify_evidence`复核支撑证据。verify-workflow实际109通过、0失败、7跳过。

审阅过程一个只读辅助片段首次调用frozen_cards时漏传两个参数而退出1；按读取的真实函数签名修正后完成上述推演。项目文件与状态未受影响。

## 未验证项

- 35张业务卡验收全部NOT RUN。
- Rust/Tauri、真实WebView长测、Android原包、API35镜像/AVD预算、在线来源、SQLite、安装发行及移动真机均待对应卡实际运行。
- src-tauri尚未建立，当前原生fmt/check/clippy/test为NOT RUN。
- 独立审阅者未重新运行完整validate或verify；上述通过结论来自已核验的外层本次收据及日志。
- heartbeat和首个开发CLI尚待外层在本报告保存、提交后建立及核对。

当前文档与启动配置满足开发前规划审查，未发现阻断项。实现结果与后续每卡质量门由原工作流持续验证。

DOC-REVIEW-VERDICT: CLEAR
