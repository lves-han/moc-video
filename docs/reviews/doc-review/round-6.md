# 独立文档审查 round-6

这是一轮独立只读复审。审阅期间没有修改、创建或删除项目内任何文件，也没有勾卡、提交 Git、启动开发/驱动/巡检，或运行 drive.py、run.py、verify.py、validate.py。临时脚本只放在 scratchpad：`/private/tmp/claude-501/-Users-eason-jp/91ce2c43-8451-4166-890a-47182011d28d/scratchpad/chk.py`，以及标准输入里的只读 Python 片段。这些片段导入 `scripts/workflow.py`，调用 `cards`、`config`、`frozen_cards`、`authorized`、`blocked`。审前和审后 `git status --porcelain` 都是 16 行，一致。

## 受审基线

- 唯一项目根：`/Users/eason/jp/application`。本机为 macOS 27.0.1 arm64。
- Git HEAD：`b1afca6e8e60a315c444f4da8efd774abf1b64e4`。
- 未提交改动：14 个已修改文件（AGENTS.md、docs/ARCHITECTURE.md、DECISIONS.md、PROGRESS.md、PROMPT-CARD.md、PROMPT-DOC-REVIEW.md、PROMPT-PLAN.md、PROMPT-SUPERVISE.md、TASK-TEMPLATE.md、TASKS.md、WORKFLOW.md、scripts/tests/test_workflow.py、scripts/workflow.py、workflow.json），另有 2 个未跟踪项（docs/PROMPT-START.md、docs/reviews/）。
- `task.py status` 的 protected 摘要，审前和审后一致：`d6cdedc4a4ca0160b71a44991a61ba98482d1315798e51a27dd430f96d1f0625`。current=null，state、blocked、runs 均为空。
- round-5 的 CLEAR 绑定旧基线 `0c4710cb…6ecf`，已失效。

`task.py status` 的 adr 分段摘要：

| ADR | 摘要 |
| --- | --- |
| ADR-001 | `31756257a1974108f9c1c8426c9bcb8f80128e2b867e9703f4b766b2e1754407` |
| ADR-002 | `d67dcfe2ddb2fd858acd1b1f4cb17440fa47dc15b4948811b1018d062b52e368` |
| ADR-003 | `26e4e12c4ae38b3ddcfbea18c04a1602799586a08610125f66567ae8dd516c5c` |
| ADR-004 | `a08db017049e19a1481fa314938b5eb88dfcb5598287af24e70bd93058c6eaae` |
| ADR-005 | `df0b0d68cd7a4595bd621ce153ba914da82496311c112089fd5b843d8685606d` |
| ADR-006 | `335827cabfced06c4cc2e2fc99978940e01818abbd7af6aedd12a66161b883c1` |
| ADR-007 | `028d7df4ecd18afa281cb45703aaf4e26857f114163be10b0c2d8860d718b227` |

这些摘要用控制器新的 adr_digests 方法计算（每段先去掉段尾空白），所以和 round-3..5 手工算出的分段摘要无法直接对比。DECISIONS.md 相对 HEAD 没有删除行，ADR-001..006 正文没有改动。

受审文件 SHA-256：

| 文件 | SHA-256 | 相对 HEAD |
| --- | --- | --- |
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` | 追加 2 行 |
| CLAUDE.md | `336cc4fbf19beaada7ccf9986414fa91851a8d7a07dfb3ccbe800a69eed0ab49` | 相同 |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` | 相同 |
| docs/ARCHITECTURE.md | `bc0cf0f076e33bf1684f66ece496c6792bc21333c2f9af6e0fbe8ece6d571ac5` | 改 1 行 |
| docs/TASKS.md | `4624055b45ad42e092feed09aa793a954066ee835f1a8c3c5247b13fbfdafa84` | 变化 |
| docs/DECISIONS.md | `c69e36906a6f09a631272d0ac4fa283571f3c9d339316b93d13fdfccd93e1c66` | 仅追加 |
| docs/PROGRESS.md | `351c7338b23a2e0fd7975138e982f618cc475de0a36a53e45a5e30fb4f764985` | 仅追加 |
| docs/PROMPT-DOC-REVIEW.md | `399371107d92e4172312182ba7e888fbe41f377a35c57d6109ccb852ef7382c0` | 与模板相同 |
| docs/PROMPT-START.md | `f72455f542af1f92f63c641a342cbcec286b49479356dd56c444612eb39e1d0a` | 新增，与模板相同 |
| docs/PROMPT-SUPERVISE.md | `2089bb44078270100a9299d4de55cdee62bb43c3a88a3253ef382abed020f88a` | 与模板相同 |
| docs/PROMPT-PLAN.md | `91bf23e92263a03a67d7b1175ea2acd720b3277338ed30f40575c194a9c6c7b9` | 与模板相同 |
| docs/PROMPT-CARD.md | `62d776ad2ae53a6bd8cc8d774215f4a0b789c9f65a037b478140249ca1d6beed` | 只差项目名替换 |
| docs/TASK-TEMPLATE.md | `b3b97acc404d5e40560e673461e195ebe3db8282583e93edecaf8bd3db71f989` | 与模板相同 |
| docs/WORKFLOW.md | `a315d2fa7ee35e236e193427274c930b33b91a0503c624cdefcca47d2b81085a` | 与模板 README 相同（替换项目名后） |
| workflow.json | `c905a09678abefa3f6c119864a5620714ee94b83f811f77fbb102888d2143354` | 只改 verify-project 超时 |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` | 与模板相同 |
| scripts/tests/test_workflow.py | `59f28b00e06b2f1580acdc7aa71b503a1c07251f57af6c192d3356cb7fa3654c` | 与模板相同 |
| scripts/check-project.py | `80bfa5a3ad4f5627705039ced9329fdfa7b0ecdb7127bfc536abd0b0a1d0b5ea` | 与 HEAD 和 round-5 相同 |
| scripts/check-contracts.py | `050b17757385b4d7161dc08393c4d01ba0a8f2aba4cefb7f8071662d2721aa78` | 与 HEAD 和 round-5 相同 |
| replica-spec/AI_TASKS.md | `89acb3b77e37366e6df6b7645faac06c345620a337d84a1c7d46eee847827c61` | 相同 |
| docs/SOURCE_MANIFEST.json | `8e92ab4f72a498d5c64a05dbac23c5839197021423e02edf575ef5838c4d127a` | 相同 |
| protocol/app.schema.json | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` | 相同 |
| protocol/bridge.openapi.json | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` | 相同 |
| protocol/examples/contract-cases.json | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` | 相同 |
| docs/reviews/doc-review/round-1..5.md | `39d818ae…8dfd8e`、`54787b45…8b230`、`bcf76239…201016`、`90d15825…f86246f54`、`839d1a5a…bd5bf5` | 新增，与 evidence/doc-review/round-N/report.md 逐字节相同 |

## 一、同步一致性（重点 1）

模板取自 `~/.codex/skills/moc-init/assets/project/`。模板文件修改时间为 2026-10-26 14:49–15:08（+08:00），早于 ADR-007 的 07:17Z。用 cmp/diff 逐个比对：

**与模板字节一致：**
- 脚本：workflow.py、tests/test_workflow.py、task.py、run.py、verify.py、drive.py、validate.py、platform_runtime.py、windows_job.py、tests/test_platform_runtime.py。
- 提示词：PROMPT-PLAN、DOC-REVIEW、SUPERVISE、START、REVIEW、RESUME，以及 TASK-TEMPLATE。

**只差项目名替换：**
- PROMPT-CARD.md。
- WORKFLOW.md（对应模板 README.md）。

**AGENTS.md：**
- 结构为：全局 AI 原则段（与模板 global-AGENTS.md 相同，另加两条表述风格规则）+ 模板项目规则段（替换项目名后逐字一致）+ 原有“本项目初始化约定”。
- 相对 HEAD 只追加了 2 行工作流规则。

**workflow.json：** 相对 HEAD 只把 verify-project 的 `timeout_seconds` 从 7200 改为 3600。其余项目 verify 项（contracts、working-contracts、frontend）沿用原值。

**项目自有文件：** check-project.py、check-contracts.py、SPEC、replica-spec、protocol，以及 src/public/tests/package.json 相对 HEAD 都没有差异。

结论：同步与模板一致，项目自有文件没有被改动。

## 二、需求覆盖结论

### TASKS 正文改动

相对 HEAD 只删了 2 行，都有对应替换：
- T0.28“独立验收”：报告路径改为 `docs/reviews/T24/round-<N>.md`，其余判定字句不变。
- T0.29：“阶段门”句改为“移动阶段”说明。

其余全部是新增：
- 36 项验收的 `cases`；
- 6 张卡的 `- 环境：`；
- T1.1 的 `- 阶段门：`。

覆盖表、卡号、依赖、原始义务、需求行、argv、report、超时都没有改动。SPEC §13 的指标和 ADR-005/006 的平台范围段也都保持原样。

### cases 核对（脚本）

- 全部 36 项验收的 `cases` 都等于本卡“需求”行的 ID 列表，顺序也一致，没有差异。
- 没有 `skippable`。
- JUnit 报告路径在全卡唯一，`cards()` 唯一性检查通过。

round-5 已确认的 113 行覆盖表、双向追溯和 argv/report 一致性没有受到影响。

结论：覆盖充分。没有发现新的遗漏、虚假覆盖、无来源扩展或未经批准的删减。

## 三、可行性推演（重点 2）

### 选卡模拟

按控制器真实语义，只读调用 `frozen_cards`、`authorized`、`blocked`：

- **当前状态**：blocked={}，冻结集合只有 T1.1（阶段门）和 T1.2..T1.6（依赖 T1.1）。`task.py check` 通过，next 为 T0.1。
- **假设桌面卡全部完成**：冻结集合相同。`next_card` 以 code 11 报“BLOCKED：没有可执行的卡；T1.1（阶段门待人类授权…）…”，与 PROMPT-SUPERVISE“以 11 退出并列出受阻卡时报告用户决定项”衔接。
- **假设 T0.19（T02 Android）BLOCKED**：冻结 T0.18..T0.29 和全部 M 卡。HTTP 分支 T0.1..T0.17 继续推进，与 T00 原卡“SDK/虚拟化缺失只暂停 Cloudstream 分支，继续 HTTP 任务”一致。
- **假设 T0.2 BLOCKED**：只冻结 T0.12、T0.16、T0.18、T0.24..T0.29；T0.3..T0.11、T0.13..T0.15、T0.17、T0.19..T0.23 可以继续。

### 阶段门的识别

- 只有 T1.1 被解析出阶段门（gate=True）。
- T0.29 的新行是 `- 移动阶段：…`，正则 `^- 阶段门：` 不匹配，所以 T0.29 没有被误识别为带阶段门（gate=False）。
- M 卡的 `- 阶段前提：` 也不匹配。
- PROGRESS 里目前没有任何 `- 结果：` 行；现有标题里的时间戳（如 `T07:17:50Z`、`T06:04:20.742281`）都不会命中 `T\d+\.\d+`，不会被误判为卡号 BLOCKED 或全局 BLOCKED。

### 本机环境

- 已有：Rust（cargo/rustc/rustup）、Node/npm、Docker、Java、xcodebuild、Android SDK（含 emulator、platform-tools、licenses），以及一个 API 34 arm64-v8a AVD。
- SPEC 要求的 API 35 `default;arm64-v8a` 镜像未安装。安装 SDK 属于 PROMPT-START 的默认代办（“卡片要求的 SDK/工具链”）。

### 推演结论

在默认授权（含卡内真实验收所需网络）下，从 T0.1 到 T0.29，文档层面没有发现一张必然无法在本机完成的卡。实际最先可能受外部条件影响的有：

- T0.1：需要联网获取 plugins.json 和 7 包。
- T0.22/T0.24/T0.25：依赖在线 provider 的可用性。
- T0.28：独立验收审阅者的启动方式，见 N7。

这些情况都有分支级 BLOCKED 隔离。依赖它们的卡都排在本机可完成的 HTTP 分支之后，不会把 HTTP 分支卡住。第一张由文档规定必然停下的卡是 T1.1：它带阶段门，等待人类授权，有 ADR-002/007 和 AGENTS 项目约定作来源。

## 四、cases 设计（重点 3）

**和 ARCHITECTURE 的一致性。** C 验收契约改为：
- 每个需求 ID 输出一条同名汇总 testcase，其下子用例命名为 `<ID>/<用例>`；
- 汇总只在全部必需子用例通过时通过；
- macOS arm64 必需子用例被跳过时，汇总失败。

TASKS 的 cases 正好列出这些汇总名，两者一致。

**控制器的拒绝能力（`case_problems` 与 `test_counts`）：**
- 缺少某个汇总 testcase，判失败（“未执行”）。
- 汇总 testcase 失败，判失败。
- 汇总 testcase 被跳过，判失败（没有 skippable）。
- 任一子用例 failure/error，失败计数大于 0，rc=3。
- 零个通过测试，判失败。
- 报告缺失、不是本次生成或被改写，判失败。

所以“漏掉某个需求 ID”“伪称已执行”“沿用旧报告”这三类错误实现会被控制器直接拒绝。

**无法满足的 case。** 逐卡检查需求 ID，没有发现在本卡完全没有 macOS 可判定内容的 ID：
- 非 macOS 平台的内容由子用例跳过处理。
- 移动 ID 只出现在带阶段门的 M 卡；桌面卡上的 U07/R52 有桌面 cfg/入口部分可测。
- T0.26 的 A1 和 A2 都携带 5 个 ID，与覆盖表“R09/R47/R48/C-RESOURCE/REQ-T22 → T0.26/A1, T0.26/A2”一致。

剩余的局限见 N3′。

## 五、verify-project 超时与预算（重点 4）

**来源。** 7200 来自规划作者的提交 e14b974。SPEC、AI_TASKS 和用户指示里都没有这个数字。它是 verify 门的执行上限，与产品阈值无关；超时只会让命令以 124 显式失败，验收内容和断言保持不变。我把它归为可接受的执行参数调整，不属于强度降低。

**ADR-007 的处理。** ADR-007 写明了原因、算式（14400+8640=23040>21600，调整后 19440），以及“恢复 7200 并提高 max_sessions”这个用户选项，透明度足够。

**预算覆盖：**
- **总额检查**：verify 合计 5040s。每卡“验收超时 + verify”的最大值是 T0.26 的 19440s，不超过 21600s（3h×2）。没有单项超过 10800s。
- **会话内装箱**：总额检查不保证单个会话装得下。T0.26 的第二个会话里，A2 的 7200s 加完整 verify 的 5040s 是 12240s，超过单会话 10800s。T0.1、T0.2 等单项 7200s 的卡，也要把实现工作和验收分到两个会话里。
- **结论**：只要实际时长接近超时上限，就可能耗尽额度。这延续了 round-5 的 N4，ADR-005 已要求开发前按实测复核，所以不阻断（见 N4′）。

## 六、ADR-007 授权来源（重点 5）

**原话核对。** ADR-007 引用的用户原话，与外层提示转述的原话逐字一致：

> 帮我修改掉application里的内容  让其与最新的skill匹配

**改动范围。** 全部改动都能在最新模板里找到对应机制：
- 控制器和提示词同步；
- 分支级 BLOCKED 与阶段门；
- `cases`；
- `- 环境：` 字段；
- 预算检查；
- docs/reviews 路径。

**唯一的数值改动：** verify-project 超时下调。它是让新预算检查通过所需的最小配置适配，ADR-007 也写明了用户可选的回退方案。

**越权核对：**
- 没有改需求、覆盖表、平台范围（环境行只复述 ADR-005/006）、性能指标或卡依赖。
- 用 ADR-002（M 系列另获授权）和 ADR-005/006 来源核对：T1.1 阶段门与原决定相同，只是执行机制由控制器承担。
- 没有已开始的卡，所以不需要 RESOLVED 或 rebaseline。

结论：授权来源与改动范围一致，没有越权扩大。

## 七、既往问题回退与 ADR 冲突（重点 6）

- **R4-B1**：ADR-006 没有改动。6 张卡的环境行按 arm64 必需、x86_64 记录 NOT RUN 写明。没有回退。
- **N1**（门禁记录以 docs: 提交）：原机制被 ADR-007 的 AUTHORIZED 机制取代。AGENTS 和 PROMPT-SUPERVISE 都规定 AUTHORIZED 由外层以 `docs:` 提交；PROGRESS 属于 DYNAMIC，`completed`/`check_completion_tree` 在后续 docs: 提交之后仍成立，和 round-5 对 N1 的分析一致。
- **N2**（T24 按轮次数值取最大）：保留“N数值最大”。新路径 `docs/reviews/` 在 DYNAMIC 里，提交报告不会改变 snapshot，也不计入范围（`scope_problems` 的 allowed 含 DYNAMIC）。
- **ADR 冲突**：ADR-007 明确写“取代ADR-005中T0.29由监管追加BLOCKED的做法”，并改了 T24 路径，按 AGENTS“已有ADR用新ADR取代”处理，没有矛盾。ADR-006 中 N1 那条也随之被取代，但 ADR-007 没有点名（见 N8）。

## 逐项问题

没有阻断问题。以下都是非阻断记录。

### N3′ 汇总级 cases 冻结的粒度
- **需求 ID**：全部卡。
- **位置**：ARCHITECTURE C 验收契约；TASKS 各验收的 `cases`。
- **影响**：
  - 控制器冻结的是需求 ID 级的汇总名，子用例集合由开发者在验收脚本里决定。
  - 如果一个脚本给汇总 testcase 只挂一个很弱的子用例，控制器照样通过。
  - 这类错误实现要靠卡内冻结的原始义务、ARCHITECTURE“逐项关联”规则和每卡独立巡检来拒绝。
  - 这比 round-5 时“控制器不核对 ID”更严格，但没有做到 TASK-TEMPLATE 示例里冻结细粒度用例的那一层。
- **最小修订（可选）**：开发前或在各卡开始前，把关键子用例（如 `R27/range-416`）补进 `cases`。或者在每卡巡检提示里要求逐条核对子用例与原始义务的对应。

### N4′ 会话内装箱与实测时长
- **需求 ID**：C-RESOURCE、REQ-T22、REQ-T00、REQ-T01。
- **位置**：workflow.json 预算；T0.26/T0.1/T0.2 等单项 7200s 的验收。
- **影响**：总额检查已通过，但没有证明单个会话装得下“实现工作 + 长验收 + 完整 verify”。verify-project 的 3600s 也还没有经过 Rust 冷构建实测；当前“数秒”是因为 src-tauri 还不存在。
- **最小修订**：T0.3 首次加入 Rust 后，记录 verify-project 的实测时长和峰值 RSS。接近上限时按 ADR-007 给用户的选项报告，不要自行调整。

### N5 环境矩阵分散
- **位置**：TASKS 平台范围段、ADR-005/006、T00 验收、各卡环境行。
- **影响**：新 PROMPT-PLAN 要求先列执行环境矩阵（本机/CI/用户机器/凭据/设备）。目前这些信息分散在上述位置。T0.1、T0.25 正文提到“三平台”，但没有环境行。字段本身是可选的，平台范围段统一覆盖了这两张卡，所以不影响判定。
- **最小修订（可选）**：在 TASKS 路线段加一张矩阵表，例如：

  | 环境 | 状态 | 决定来源 |
  | --- | --- | --- |
  | macOS arm64 | 本机可达，必需 | ADR-005/006 |
  | 其他桌面平台 | NOT RUN | ADR-005/006 |
  | Apple 签名 | 缺凭据时未签名 | ADR-005 |
  | 移动真机 | 阶段门 | ADR-002/007 |
  | 在线来源 | 联网，按卡实测 | — |
  | CI | 只用于脚本可移植性 | — |

### N6 阶段门授权内容
- **位置**：T1.1 `- 阶段门：`。
- **说明**：阶段门写明了需要“真实设备、签名与引擎位置”。`authorized()` 只核对 AUTHORIZED 和“授权：”两行。外层记录 AUTHORIZED 时，应当同时取得这些前提，否则 T1.1 会立即因为缺设备写 BLOCKED。这是执行提醒，不需要修订。

### N7 T24 独立验收审阅者的启动者没有写明
- **需求 ID**：REQ-T24、U06、R51。
- **位置**：T0.28“独立验收”；PROMPT-SUPERVISE/PROMPT-CARD。
- **影响**：
  - t24.py 要求 done 之前已经存在 CLEAR 报告。
  - 控制器的 review_command 只在完成提交之后运行，PROMPT-SUPERVISE 也没有写“验收前由外层启动独立审阅”。
  - 默认开发 CLI 的权限（Claude 只允许 venv python 和 git；Codex workspace-write）未必能启动独立只读审阅者。
  - 这个安排在 round-5 时已经存在，本轮只换了路径，不算回退；但它影响 T0.28 的可执行性。
- **最小修订**：在 T0.28 或 PROMPT-SUPERVISE 里写明流程：开发会话在固定提交处提交 WIP 并交接；外层按 review_command 同等权限，启动独立只读审阅；报告以 `docs:` 提交到 `docs/reviews/T24/round-<N>.md`；之后再续卡。

### N8 ADR-007 没有点名取代 ADR-006 的 N1 条
- **位置**：DECISIONS ADR-007“适配”。
- **影响**：读者可以从“取代ADR-005中…做法”推断出来，执行不受影响。
- **最小修订（可选）**：在下一条 ADR 里补一句“ADR-006 的 N1 条同时被 AUTHORIZED 机制取代”。

### N9 审查报告尚未提交
- **位置**：docs/reviews/（未跟踪）。
- **影响**：PROMPT-DOC-REVIEW 规定原始报告缺失会使 CLEAR 失效。PROGRESS 对 round-1..5 的引用仍指向 evidence 路径，那里的副本和 docs/reviews 下的逐字节一致。
- **最小修订**：保存本轮报告后，把 docs/reviews/ 连同本批改动一起以 `docs:` 提交。

### 延续
round-5 的 N5（agent_command 和 review_command 都还是 null）和 N6（缺 Apple 签名凭据时产出未签名包）仍然适用。

## 实际验证

- `.venv/bin/python -B scripts/task.py status`，审前和审后各执行一次，结果一致：protected 为 `d6cdedc4…0625`，adr 为上表 7 段，current=null，blocked 为空。
- `.venv/bin/python -B scripts/task.py check`：`roadmap OK: 35 cards`，退出 0，没有超额或跨会话提示。
- `.venv/bin/python -B scripts/check-contracts.py`：PASS，17 命令、179 结构用例、89 语义用例、6 生命周期向量。
- **与模板比对**：用 cmp/diff 比对 21 个文件，结果见第一节。
- **相对 HEAD 的 diff 统计**：
  - SPEC、replica-spec、protocol、check-project.py、check-contracts.py 和前端都没有差异；
  - TASKS 删 2 行，均有对应替换；ARCHITECTURE 改 1 行；
  - DECISIONS、PROGRESS、AGENTS 的删除行都是 0。
- **scratchpad 脚本**（导入 workflow.cards/config）：
  - 36 项验收的 cases 与需求行完全一致，没有 skippable，JUnit 路径唯一；
  - 预算：verify 合计 5040s，每卡最大 19440s，额度 21600s；
  - 环境行有 6 张卡，阶段门只在 T1.1。
- **冻结语义模拟**：当前状态、桌面全部完成、T0.19 BLOCKED、T0.2 BLOCKED 四种情形，结果见第三节。
- **阅读 workflow.py**：cards、blocked、global_blocks、authorized、frozen_cards、next_card、start、case_problems、test_counts、budget_problems、scope_problems、snapshot、DYNAMIC 和 PROTECTED。
- **核对 PROGRESS 引用的 verify 收据** `evidence/verify/20261026T071557733715.json`：
  - status=finished，exit=0，protected 与当前一致；
  - 6 个 verify-* 运行的 exit 都是 0；
  - verify-workflow 109 通过、0 失败、7 跳过。
- **round-1..5 副本**：逐字节等于 evidence/doc-review/round-N/report.md。
- **本机工具探测**：只用了 command -v 和 ls。

## 未验证项

- 用户原话由外层提示和 ADR-007 转述，审阅者看不到聊天原文。本轮只核对了两处转述逐字一致，以及改动范围与“匹配最新 skill”对应。
- 没有运行 validate.py、test_workflow 和完整 verify（属于禁止范围），结论依据上面的收据。控制器测试与模板一致，但本轮没有独立执行。
- 以下都未实测：
  - verify-project 在 Rust 加入后的实际时长；
  - 各长测的实际时长和峰值 RSS；
  - API 35 arm64 镜像下载及 AVD 预算；
  - 在线 provider 和网络的可用性；
  - 开发与巡检 CLI 能否启动 T24 的独立审阅者。
- 35 张卡的业务验收全部 NOT RUN：tests/acceptance/*.py 和 src-tauri 都还不存在。
- 本文档 CLEAR 只确认规划充分，不证明实现已验证，也不授权开始开发。

DOC-REVIEW-VERDICT: CLEAR
