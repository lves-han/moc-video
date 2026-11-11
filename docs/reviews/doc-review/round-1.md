# 独立文档审查 round-1

审阅者与任务卡作者分离。本轮仅执行读取、摘要、解析与只读检查；未修改项目文件、写报告、启动开发或巡检。报告由外层代理保存。

## 受审基线

项目唯一根目录：`/Users/eason/jp/application`。

Git HEAD：`4efb6a354fb125d26001b5955010f55c951817db`。受审文件包含未提交修改，因此以内容摘要绑定。

原控制器 `task.py status` 在审查前后返回相同 protected：

`02bfaf1064a5c20f58eab5da4b0b85a23c3ba1c508759afa0804d7da3901a57c`

补充 SHA-256：

| 文件 | 摘要 |
| --- | --- |
| AGENTS.md | `44a6bfdeae21c92fe8350d8c5972396ba21708e8fce87c9c3ff26afe80c6f5d5` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `46e4871ba10b2cc540a70b9a043f7c386a37cec53717fda5252cb38261638e0d` |
| docs/TASKS.md | `c02d3e7c81ac05154fcca8023329162656d2176ffca3308f290a7b908ed9008f` |
| docs/DECISIONS.md | `ad9e7abd29b413399745cb48dd85db315e64e4683ec60963a992c2de97353654` |
| workflow.json | `584eb4fd9b979b4fc8c07e9450192d231428cfc251ba089072d78a63cf6162c5` |
| scripts/check-project.py | `80bfa5a3ad4f5627705039ced9329fdfa7b0ecdb7127bfc536abd0b0a1d0b5ea` |
| docs/SOURCE_MANIFEST.json | `8e92ab4f72a498d5c64a05dbac23c5839197021423e02edf575ef5838c4d127a` |
| docs/FRONTEND_BASELINE.md | `d0c945e3060992f3bca29cffc6a8718c249a1bfdabaccd23f48937b8aad3c5fc` |

已采纳 ADR 分段摘要，按标题起至下一个 ADR 标题前的原始 UTF-8 内容计算：

- ADR-001：`9377c96f9791c3416582254a4df6fef38676e67f0f2e1b780a99e737b80b7f21`
- ADR-002：`d7b006f8b5c9053401682f590513a89405044334db022a6213efb4ea763760b2`

`docs/SOURCE_MANIFEST.json` 的全部 16 个摘要均与项目内归档文件相符，且各文件与原 `/Users/eason/jp/replica-spec` 逐字节一致。该 manifest 摘要绑定本轮完整归档输入。

## 需求覆盖结论

35 张原任务卡正文均完整保留，并建立一对一工作流编号映射。U01..U09、R01..R67、路径与资源约束、G0-Core/G0-CS/G1..G5、MG0..MG3均有规划落点。HTTP先行、原7包及全部provider、主客世代隔离、匹配、SQLite、双语与外观、三平台安装和后续移动要求得到保留。性能表维持原目标强度，AVD及缓存预算仍作为起点；Docker、容器硬限额合计与宿主RSS、身份清理有明确要求。

卡内验收描述普遍包含错误实现反证，覆盖具体输入、输出、失败及生命周期场景。以下问题影响路线可实施性或验证可靠性，当前版本须修订并独立复审。

## BLOCKING 问题

### B1：T18验收提前依赖尚未实现的RuntimeManager

- 需求：R11、R36、REQ-T18；SPEC §5.3、§11.2。
- 位置：`docs/TASKS.md:800` 的 T0.18；A1要求实际“自身进程/流/forward/AVD清理”，当前依赖仅为T0.16/T0.17。真实RuntimeManager由 `docs/TASKS.md:910` 的T0.21交付，Cloudstream实际接线由T0.22交付。
- 因果：按当前选卡顺序，T18先于真实AVD管理实现。其清理验收会依赖后卡，或只能以合成清理冒充实际行为。
- 最小修订：给T18补上T0.22依赖，并相应说明其实际执行位置。依赖图仍可保持无环；HTTP闭环先由T16/T17完成。另一可行方式是明确将T18的HTTP/SQLite验收与Cloudstream清理验收分配至已有后卡，并更新覆盖映射，保留原完整清理义务。

### B2：必要依赖和接线文件落在卡片允许范围外

- 需求：R08、R30、R33、REQ-T01、REQ-T05及HTTP/网关相关卡。
- 位置：`docs/TASKS.md:220`、`:256`以及多张仅允许`src-tauri/src/`的后端卡；实际范围检查见`scripts/workflow.py:440`。
- 事实：当前`package.json`只有Vue与Naive UI，缺少Tauri API/CLI及hls.js。T01范围未包含`package.json`、`package-lock.json`；T05增加类型化invoke时也未包含。N00及T09等新增原生HTTP/服务器能力时，其范围也排除了Cargo清单与锁文件。
- 因果：正常安装必要依赖或完成真实入口接线后，原控制器会报告范围外修改。将所有依赖预先塞进T00/T01又与按实际需要增加依赖的原则冲突。
- 最小修订：逐卡仅补入直接所需的根依赖清单、锁文件及入口配置。T01至少包含`package.json`、`package-lock.json`；T05允许其invoke接线依赖。需要新增Rust依赖的卡允许`src-tauri/Cargo.toml`与`src-tauri/Cargo.lock`。同时核对T01样机入口、后续详情/播放状态接线实际涉及的现有文件。

### B3：完整verify仅校验归档协议，实际protocol契约缺少持续验证入口

- 需求：U06、R17、R31、R51、C3。
- 位置：`workflow.json` 的`verify-contracts`；`scripts/check-project.py:11`起；`docs/ARCHITECTURE.md:25`、`:131`；T05与T18的`protocol/`交付。
- 因果：`verify-contracts`固定运行冻结的`replica-spec/validate_contracts.py`。`check-project.py`复核归档摘要及Rust构建测试，未验证未来实际使用的`protocol/bridge.openapi.json`、应用schema及样例。因此工作协议发生引用错误、正负例漂移或历史IPC定义脱节时，完整verify仍可仅凭原归档协议通过。
- 最小修订：现在为实际`protocol/`预设明确验证入口，并使完整verify在其创建后执行结构、跨文件引用、正负例和必要语义检查；原始规格包保持字节冻结。T05验收应拒绝工作协议缺失或既有17命令漂移，T18应检查新增历史IPC与样例。具体行为测试继续由各卡交付。

### B4：verify-contracts首次运行的报告父目录缺失

- 需求：U06、R51；真实可执行verify。
- 位置：`workflow.json` 中报告参数`evidence/contracts/validation.json`；`replica-spec/validate_contracts.py`末尾直接`args.report.write_text(...)`。
- 事实：受审版本的`evidence/contracts`目录不存在，验证器未创建父目录。外层作者报告完整verify已实际遇到该`FileNotFoundError`；本审阅者另行确认路径缺失及代码行为。
- 因果：契约校验即使全部成功，写报告时仍退出失败，阻断每卡完整verify。
- 最小修订：将输出改至已有`evidence/`根目录，例如`evidence/contracts-validation.json`，或在调用入口明确创建父目录；保持原归档验证器字节一致。

## 应明确的实施前事项

以下已在文档标注待确认，当前无实现通过结论：

- CLI、模型、权限与会话预算保持待定。开发前核实长验收的完整时长，尤其T22的多平台套件和累计会话额度。
- 外部平台证据须与当前构建、源码摘要及准确命令绑定。报告规则应允许有效的跨机器证据汇总，同时明确陈旧报告及预填结果的拒绝条件。
- T18历史IPC目前明确留待定义。建议在冻结前固定最小命令、参数、结果和写入世代规则；若作为T18既定交付物补齐，应明确它与受审需求变更的处理区别，避免常规实现必然进入额外决定流程。
- 图像/字幕具体大小与像素阈值、参考机和桥接样本数按对应卡实证固定，并保留来源与测量条件。

## 实际验证

使用解释器：

`/Users/eason/jp/application/.venv/bin/python`

已执行：

1. `-B scripts/task.py status`，审前审后protected一致，当前卡为空，无开发状态。
2. `-B scripts/task.py check`：`roadmap OK: 35 cards`。
3. 原35张任务卡正文保留检查：35/35完整出现。
4. manifest全部16文件摘要及与原输入逐字节比较：相符。
5. `-B replica-spec/validate_contracts.py --report /dev/stdout`，退出0；报告仅写标准输出，未创建项目报告。验证包含17条路径、23个操作、54个桥接schema、88个应用定义、17命令、461引用、179结构用例（98正例/81负例）、89语义用例、6生命周期向量。
6. Git根目录核对：`/Users/eason/jp/application`。

## 未验证

业务卡验收均为 **NOT RUN**。本轮未执行前端构建、Rust/Tauri构建、Android原包运行、真实来源请求、三平台媒体/安装、SQLite运行或移动真机验证。原App制品字节复核为`not_requested`。原归档中的历史验证报告按来源资料保留。

B1..B4修订后，需绑定新摘要重新独立审查。

DOC-REVIEW-VERDICT: BLOCKING
