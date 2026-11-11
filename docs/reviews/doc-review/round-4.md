# 独立文档审查 round-4

本轮是独立只读复审，审阅者没有修改任何文件，也没有保存报告、勾卡、提交 Git、启动开发/驱动/巡检，或运行 drive.py/run.py/verify.py。

## 受审基线

唯一项目根是 `/Users/eason/jp/application`，本机为 Apple M3 Max、arm64、36 GiB 内存、macOS 27.0.1。

Git HEAD 是 `e14b974467dd3e16c091d92790de60ff3b17cbc7`。工作树里有 4 个文件未提交：docs/ARCHITECTURE.md、docs/DECISIONS.md、docs/PROGRESS.md、docs/TASKS.md，以内容摘要绑定。git diff 显示 SPEC、replica-spec、workflow.json、scripts、AGENTS.md、protocol 与 HEAD 一致。

原控制器 `task.py status` 的 protected 摘要，审前和审后一致：

`a03036895150128ed5343fe9a58553ec7e9d26809f2c225e32fde429488c040a`

round-3 CLEAR 对应的旧基线 `66dd658f…9520` 已经失效。

| 文件 | SHA-256 |
| --- | --- |
| AGENTS.md | `1f334de4b44e24595aa085a780eb3b2b5edfbc9131ef62044db30c1a8b730cca` |
| CLAUDE.md | `336cc4fbf19beaada7ccf9986414fa91851a8d7a07dfb3ccbe800a69eed0ab49` |
| docs/PROMPT-DOC-REVIEW.md | `ac99a36871ae364b09801d36c57c686148f9090b08a3d558668904374cbac869` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `f170eb5b3dddbf0c5dc0289d494d74628f14e2c91b638956b027a50e94e7e8fd` |
| docs/TASKS.md | `170b29cab42e8a6b6e9f63d49f0d6fcae8aea32fa007d8cadc42c9dfa3835cc4` |
| docs/DECISIONS.md | `2bf9babe07670076da7cbefa45ee8e699295a04375dc5b0be9272302e8b310a4` |
| docs/PROGRESS.md | `268efa3be43aa99ba9690ddab73891e3566b2af89818ca717a27597b4233aa3c` |
| workflow.json | `e09a371ac0935556695fa2023c163cb25b20be61601193b5a7d3d02ffd50de1f` |
| scripts/workflow.py | `793327a31ca73b0bdabf8d3197dae9cb54db95b918f5e9a277b62dcafc67d246` |
| scripts/check-project.py | `80bfa5a3ad4f5627705039ced9329fdfa7b0ecdb7127bfc536abd0b0a1d0b5ea` |
| scripts/check-contracts.py | `050b17757385b4d7161dc08393c4d01ba0a8f2aba4cefb7f8071662d2721aa78` |
| replica-spec/AI_TASKS.md | `89acb3b77e37366e6df6b7645faac06c345620a337d84a1c7d46eee847827c61` |
| docs/SOURCE_MANIFEST.json | `8e92ab4f72a498d5c64a05dbac23c5839197021423e02edf575ef5838c4d127a` |
| protocol/app.schema.json | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` |
| protocol/bridge.openapi.json | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` |
| protocol/examples/contract-cases.json | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` |

已采纳 ADR 的分段摘要按 round-3 的方法计算：从本 ADR 标题起，到下一个 ADR 标题前为止，取原始 UTF-8 内容。

- ADR-001：`9377c96f9791c3416582254a4df6fef38676e67f0f2e1b780a99e737b80b7f21`
- ADR-002：`52d26c5e832ad756de1a9b5a4655759a270facf46b333bd1c9f149a34c864fa9`
- ADR-003：`0372eebdfa8a92e0536f37ead4f4ba87f28e7b5aaa852f89308d036abf708975`
- ADR-004：`2c96f53b7a0a87fbffed9600b27879a6c33a01bd20c1f6363acb687ec9dfbab6`
  - 正文与 HEAD 一致。摘要和 round-3 的值不同，原因是后面追加了 ADR-005，本段现在多带一个分隔空行。
- ADR-005：`b87a60a61d23e9cac063f0f596293661b07c3b16a6033f662a29c0ff7a8f5242`

## 需求覆盖结论

### 结构与追溯（脚本核对）

- 覆盖表共 113 行，所有目标都对应到现存的卡号/验收 ID。
- 每张卡“需求”行里的 ID 集合，与覆盖表按卡号反查的结果完全一致，双向都没有差异。
- 没有残留对已删除的 T0.26/A3..A6，或 macos-x86_64、windows-x86_64、linux-x11、linux-wayland 套件的引用。

### 验收入口（36 项）

- 全部 36 项验收都是 kind=test，并带 `report: junit:evidence/acceptance/<原卡号>-<suite>.xml`。
- argv 里的 `--report`、`--junit` 路径和 report 字段两两一致，文件名后缀与 suite 对应。
- T0.26 只保留 startup-ui 和 macos-arm64 两项，路径分别是 T22-startup-ui 和 T22-macos-arm64。

### 修订内容逐项落位

以下各点在正文中都能找到对应位置：
- TASKS 开头的“平台范围”段
- C14/C15 的 macOS 改写
- T0.12 范围补 `src/App.vue`
- T0.21 范围补 `src-tauri/Cargo.toml` 和 `Cargo.lock`
- N02/A1 补查 DASH
- T21/A1 补“同一受控自有媒体集合”
- T24 独立报告路径与末行结论
- T0.29 阶段门

### 未回退的项

- 卡号、依赖和原卡全文都保留，原始义务没有改动。
- T18 依赖 N02，C16 和 ADR-003/004 正文都没有变化。
- 契约检查仍覆盖 17 命令、179 结构用例、89 语义用例、6 个生命周期向量。
- 性能指标、单位、时长和阈值保持 SPEC §13 原值。
- DPI 改写为“125/150/200%（或 macOS 等效缩放）”，强度与原文相当。

### 结论

覆盖整体充分。唯一阻断点是平台降级的授权范围，见 R4-B1。

## 控制器可实施性核对（scripts/workflow.py）

### 选卡与 BLOCKED 语义

- `blocked()` 读取每条 PROGRESS 记录标题行里的 `T\d+\.\d+`；标题里没有卡号的记录归为 global。
- `next_card()` 只要发现任何未解除的 BLOCKED，就以代码 11 全局停止。
- 删掉 T01 五环境硬门之后，T0.1→T0.2→… 的拓扑选卡不再必然停在 T0.2。

### T1.1 门禁

阶段门的记录 `## T1.1 移动阶段授权门 <UTC>` 加 `- 结果：BLOCKED`，会登记到 T1.1，使 next_card 全局停止。

这条记录追加在 T0.29 完成并提交之后，对 T0.29 的完成判定没有影响：
- `done()` 只在完成当时要求“最后一条是本卡 DONE”。
- `completed()` 只核对完成提交时 PROGRESS 的前缀摘要。
- PROGRESS 属于 DYNAMIC，不进入源码 snapshot。

之后追加同卡 RESOLVED 即可解除。门禁能按描述生效，残留问题见 N1。

### kind=test 与 JUnit

`test_counts` 对 JUnit 报告的要求：
- 文件位于 evidence/ 下。
- 本次运行生成：mtime 晚于启动前 1 秒，并且元数据与旧文件不同。
- 按 testcase 统计：passed≥1 且 failed=0，否则退出码改为 3。

`verify_evidence` 会再次校验非零通过数和报告副本摘要。

因此，零用例、旧报告、存在失败或没生成报告的错误实现都会被控制器拒绝。另外两条规则——“macOS 必需项 skipped 视为失败”和“每个需求 ID 至少一条 testcase”——由卡内验收脚本执行，控制器只按计数判定。这一点与冻结控制器相容，见 N3。

## 逐项问题

### R4-B1（BLOCKING）：macOS x86_64 的降级缺少明确授权

- **需求 ID**：U04、R07、R09、R47、R48；REQ-T01/T20/T22/T23/T25。
- **位置**：
  - DECISIONS ADR-005“决定”
  - TASKS 第 5 行“平台范围”段
  - 覆盖表 U04、R07 两行
  - ARCHITECTURE C14 第 98 行
  - T0.2/A1、T0.24/A1、T0.26/A1–A2、T0.27/A1 的描述
- **事实**：
  - ADR-005 引用的用户原话是“所有跨平台问题都只限定在macos里, macos first,对跨平台不做严格要求”。
  - SPEC 第 39 行和第 251–252 行把 macOS 14+ 的 arm64 与 x86_64（Intel，x86_64-apple-darwin）同列为 macOS 系统目标。
  - 修订把必需目标收窄为“本机 macOS arm64”，macOS x86_64 和 Windows、Linux 一起降为非阻断记录项。
  - 用户原话把范围限定在 macOS，字面上包含 macOS Intel。ADR 里也没有用户对“仅 arm64”的确认。
- **影响**：
  - 一项 macOS 目标被当作已获授权排除，但缺少真实授权来源，违反 PROMPT-DOC-REVIEW 中“批准排除必须有真实授权来源”的要求。
  - 后续验收会把 x86_64-apple-darwin 的构建、Rosetta 下的运行，以及 Intel 安装包，都记为允许 skipped 的 NOT RUN 项，造成未经批准的删减。
  - 本机是 Apple silicon，可以交叉构建 x86_64-apple-darwin，也可以在 Rosetta 2 下做功能运行。这部分要求在本机可以实施，降级也就没有可行性上的理由。
  - 真 Intel 硬件上的性能，以及 x86_64 AVD（依赖“系统支持的硬件加速”），本机确实无法实测。
- **最小修订**（二选一）：
  - **选项 A**：请用户在聊天中明确确认“macOS 仅要求本机 arm64，Intel 列为 NOT RUN”，然后追加 ADR-006 引用这句原话。正文不用改。
  - **选项 B**：恢复 macOS x86_64 的部分要求：
    - T0.2/A1、T0.27/A1 增加 x86_64-apple-darwin（或 universal）构建；
    - 在 Rosetta 2 下做启动和基础播放的功能运行，并把它列为必需 testcase；
    - Intel 硬件性能和 x86_64 AVD 列为 NOT RUN；
    - 同步改写 TASKS“平台范围”段、C14、覆盖表 U04/R07 的注记，以及 ADR。

### N1（非阻断）：门禁记录需要随后提交

- **位置**：T0.29“阶段门”。
- **因果链**：
  - 门禁 BLOCKED 和之后的 RESOLVED 都是对 PROGRESS 的工作树改动。
  - 解除后，next_card 会因 `current()==T0.29` 调用 `check_completion_tree(T0.29)`，而该函数要求 `git status --porcelain` 为空。
  - 如果门禁记录没有提交，进入 T1.1 时会报“完成提交后存在未验证改动”。
- **最小修订**：在阶段门句末补一句“门禁记录与 RESOLVED 由监管以 `docs:` 提交”。

### N2（非阻断）：T24 “最新轮次”的排序规则未定义

- **位置**：T0.28“独立验收”。
- **影响**：`<轮次>` 目录名没有排序规则，按字典序 round-10 会排在 round-9 前面。
- **最小修订**：固定为 `round-<N>`，按 N 数值取最大值。

### N3（说明）：skipped 与需求 ID 覆盖由验收脚本自我执行

- **位置**：ARCHITECTURE“验收与证据契约”。
- **说明**：
  - 控制器对 skipped 只计数，不判定。
  - “macOS 必需项 skipped 视为失败”和“每个需求 ID 至少一条 testcase”要靠 t*.py 退出非零并写入 failure 来落实。
  - 每卡的独立巡检需要核对这两条。
- 现有文字已经要求脚本这样做，不需要修订。

### N4（说明）：资源与超时预算待开发前复核

- **超时**：
  - T0.26/A2 的固定媒体时长约 40 分钟（30 分钟播放加 20 Mbps 10 分钟），另有 20 次冷启动、50 次切集、故障注入和构建，单项 7200 秒偏紧。
  - T0.2/A1 如果 MP4 和 HLS 各播放 30 分钟，再加 20 次开关和构建，也偏紧。
  - ADR-005 已写明“开发前按实际时长复核”。
- **内存**：
  - memory_mib=4096 是进程树 RSS 采样。AVD 配 3072 MB，加上宿主进程，可能触发 137。
  - macOS 上 WKWebView 的 WebContent 进程由 XPC 托管，可能不在进程树内，采样会少计。
  - C14 已要求超出预算时先报告，按授权调整配置并重审。
- 开发授权前需要核实这两项；本轮不作阻断。

### N5（说明）：开发与巡检命令尚未配置

- agent_command 和 review_command 目前都是 null。
- T0.29 门禁以“独立巡检 CLEAR”为前提，所以开发前需要配置 review_command，并按原流程重审配置。

### N6（说明）：Apple 签名缺凭据的处理与原规格一致

- T0.27/A1 改为“缺 Apple 凭据时产出未签名包，并列为发行待办；未签名冒充通过即失败”。
- 这与 AI_TASKS T23 “无签名凭据时准确记录发行阻塞，禁止声称已签名”，以及第 15 行“暂停该分支、继续推进不依赖它的卡”一致。
- 交付报告需写明 G5 发行门槛未达成。

## 实际验证

- `.venv/bin/python -B scripts/task.py status`：current=null，state 为空，blocked 为空，runs 为空；protected 为 `a0303689…c040a`。审前和审后各执行一次，结果一致。
- `.venv/bin/python -B scripts/task.py check`：`roadmap OK: 35 cards`。
- `.venv/bin/python -B scripts/check-contracts.py`：PASS，17 命令、179 结构用例、89 语义用例、6 生命周期向量。
- scratchpad 只读脚本（`/private/tmp/claude-501/-Users-eason-jp/91ce2c43-8451-4166-890a-47182011d28d/scratchpad/chk.py`）：导入原 workflow.cards，核对了覆盖表与卡内需求行的双向一致、验收 argv/report/junit 路径一致、依赖顺序和超时。结果全部一致。
- 关键字检索：验收描述里非 macOS 的平台词只出现在 T0.27/A1 的 “NOT RUN” 记录句；原始义务保留原文，由“平台范围”段统一覆盖。
- 逐段阅读 workflow.py：validate_checks、cards、blocked、next_card、completed、check_completion_tree、done、completion_evidence、test_counts、execute_receipt、verify_evidence、drive 和 review 流程。
- `git diff`：逐项核对了 ARCHITECTURE、DECISIONS、TASKS 的全部改动。

## 未验证项

- 用户原话由 ADR-005 和外层转述提供，审阅者无法独立查看聊天原文。
- 35 卡业务验收全部 NOT RUN：tests/acceptance/*.py 尚不存在，src-tauri 尚未创建。
- macOS 14 最低版本兼容、本机长测的实际时长和峰值 RSS、AVD 预算、Rosetta 可运行性，均未实测。
- 完整 verify 未由审阅者运行，依据是 PROGRESS 引用的收据 `evidence/verify/20261026T062209394159.json`。
- 文档 CLEAR 只确认规划充分，不代表实现已经验证。

DOC-REVIEW-VERDICT: BLOCKING
