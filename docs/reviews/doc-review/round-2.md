# 独立文档审查 round-2

本轮为只读独立复审。审阅者未修改文件、保存报告、启动开发或巡检。

## 受审基线

唯一项目根：`/Users/eason/jp/application`。沿用round-1的Git HEAD，受审修改以内容摘要绑定。

审前、审后原控制器protected均为：

`383ef9bb87f989ee3184c5c2c62ae7a7465d72c59252ebb20a1da1a3ba32f44a`

| 文件 | SHA-256 |
| --- | --- |
| AGENTS.md | `1f334de4b44e24595aa085a780eb3b2b5edfbc9131ef62044db30c1a8b730cca` |
| docs/PROMPT-DOC-REVIEW.md | `ac99a36871ae364b09801d36c57c686148f9090b08a3d558668904374cbac869` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `ef365b4c4d90e2cec6da3d33db4d42a872e16c603e4e8f87827b51b914bf2f5f` |
| docs/TASKS.md | `f74c9ebda0c79afeaef0b26d77a39a00f09cee0ad4cea6a7645929194d5e5eaa` |
| docs/DECISIONS.md | `7ce116e3c330f3f270bbdc1c86ea284bf12852b7567edde310cd0b44d513c3c1` |
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
- ADR-003：`ce0cc3a373fe49d564e650d4c74706ff0344e45c9e0a7dfcd25c090503082e1a`

分段摘要按当前标题起至下个ADR标题前的原始UTF-8内容计算，包含分隔空行。

## 已解决的round-1问题

- B1：T18正文已移到N02之后，并增加T0.22依赖。实际AVD/forward清理已有前置交付。
- B2：T01/T05已补根依赖清单、锁文件及入口配置；相关原生功能卡补Cargo清单与锁；相关页面卡补api/state接线范围。
- B3：实际`protocol/`已存在；完整verify增加工作协议入口。检查器验证原DTO与17命令兼容、引用、结构正负例、语义与生命周期，并检查未来历史四命令整体追加及正负例。
- B4：归档契约报告改到已存在的`evidence/`根目录，原验证器字节保持一致。

35张原卡完整正文仍全部保留；需求、平台、原7包、性能目标、资源、Docker及后续移动覆盖继续成立。

## BLOCKING问题

### R2-B1：历史保存资格仍依赖临时match及当前播放世代，可能丢弃合法最终快照

- 需求：U09、R26、R42、R43；SPEC §9.1、§11.4、§18.6；DATA_STRUCTURES §10。
- 位置：`docs/ARCHITECTURE.md` C16的保存入口与“已失效match或旧播放世代返回CANCELLED”；T18 A1。
- 因果：原契约允许MatchRecord空闲5分钟回收，同时活动Session独立保有网络上下文。C16尚未规定历史写入如何继承已确认身份，直接要求临时match继续有效，会使match回收后的活动播放历史保存失去资格。此外，切集、切源、离页、退出前已捕获并排队的合法快照，可能在提交时因旧世代或match已撤销被拒绝，丢失原规格要求的最后进度。
- 最小修订：明确区分捕获/接纳资格与实际数据库提交。活动Session应保有已确认的元数据/video/播放来源及世代绑定；合法快照经宿主核验接纳后，排队保存资格持续至该次提交结束，允许在match回收或旧Session关闭后完成。删除/清空屏障、单调写序号及身份核验继续拒绝晚到旧写和伪造输入。复用现有Session记录与写队列即可。
- 所需反证：活动播放超过5分钟并强制回收MatchRecord后继续保存；切集、切源、离页和退出时延迟提交捕获旧快照；不同影片/集的快照归属准确；删除/清空后旧快照仍被屏障拒绝；伪造身份、旧事件新生成的非法快照被拒绝。
- 状态：上述业务验收 **NOT RUN**。

### R2-B2：save_history将match_id限定UUID，与既有不透明句柄格式冲突

- 需求：U06、R31、R65、R67；SPEC §18.6及既有应用机器契约。
- 位置：`docs/ARCHITECTURE.md` C16的`save_history.input.match_id: UUID`；实际`protocol/app.schema.json`的`PlaybackContext.match_id`与`ResolveEpisodeInput.match_id`。
- 因果：既有match_id格式为长度1..160的不透明字符串，允许合法的非UUID句柄。新增历史契约单独限制UUID，使同一个有效匹配能够解析播放，却在保存历史时遭到参数拒绝。
- 最小修订：复用既有match_id的准确格式。若结合R2-B1采用Session或已接纳快照作为核验依据，应明确其既有句柄格式及生命周期，保持原17命令兼容。
- 所需反证：使用合法非UUID句柄贯通匹配、解析、会话及历史保存；空、超长、未知与归属错误的句柄被拒绝。
- 状态：上述业务验收 **NOT RUN**。

## 实际验证

使用`/Users/eason/jp/application/.venv/bin/python`：

1. `-B scripts/task.py status`：审前审后protected一致；当前卡为空。
2. `-B scripts/task.py check`：`roadmap OK: 35 cards`。
3. `-B scripts/check-contracts.py`：退出0，17命令、179结构用例、89语义用例、6生命周期向量。
4. 归档manifest：16/16摘要相符。
5. 35张原卡正文保留检查：35/35。
6. 三份实际工作协议与相应原归档逐字节一致。
7. 读取并核对工作协议检查器、完整verify配置、当前全部卡依赖/范围及修订验收描述。

本轮未重复慢速控制器测试、前端构建及完整verify。外层代理报告的前端8测试/build和原App13段字节复核结果，仍应引用其真实运行证据。本报告的实际验证清单限于上述独立执行项目。

## 未验证与阶段

所有未来业务卡验收均为 **NOT RUN**，包括Rust/Tauri、Android原包、真实来源、SQLite持久化、三平台媒体/安装及移动真机。当前无开发授权、开发CLI或定时监管。

R2-B1/R2-B2修订并绑定新摘要后，重新独立复审。

DOC-REVIEW-VERDICT: BLOCKING
