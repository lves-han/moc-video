# 独立文档审查 round-5

本轮是独立只读复审。审阅者没有修改任何文件，也没有保存报告、勾卡、提交 Git、启动开发/驱动/巡检，或运行 drive.py/run.py/verify.py。临时核对只用 scratchpad 里已有的 `chk.py` 和标准输入里的只读 Python 片段。

## 受审基线

- 唯一项目根：`/Users/eason/jp/application`。本机为 Apple M3 Max、arm64。
- Git HEAD：`e14b974467dd3e16c091d92790de60ff3b17cbc7`，与 round-4 相同。
- 工作树有 4 个文件未提交，以内容摘要绑定：
  - docs/ARCHITECTURE.md
  - docs/DECISIONS.md
  - docs/PROGRESS.md
  - docs/TASKS.md
- `git status --porcelain` 只列出这 4 个文件。SPEC、replica-spec、workflow.json、scripts、AGENTS.md、protocol 均与 HEAD 一致。
- `task.py status` 的 protected 摘要，审前和审后一致：`0c4710cb082b788f215ee76746ebc1192fcb3d82dbbd18f7244667a109db6ecf`。
- round-4 受审基线 `a0303689…c040a` 已被替代，round-3 的 CLEAR 早已失效。

| 文件 | SHA-256 | 相对 round-4 |
| --- | --- | --- |
| AGENTS.md | `1f334de4b44e24595aa085a780eb3b2b5edfbc9131ef62044db30c1a8b730cca` | 相同 |
| CLAUDE.md | `336cc4fbf19beaada7ccf9986414fa91851a8d7a07dfb3ccbe800a69eed0ab49` | 相同 |
| docs/PROMPT-DOC-REVIEW.md | `ac99a36871ae364b09801d36c57c686148f9090b08a3d558668904374cbac869` | 相同 |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` | 相同 |
| docs/ARCHITECTURE.md | `f170eb5b3dddbf0c5dc0289d494d74628f14e2c91b638956b027a50e94e7e8fd` | 相同 |
| docs/TASKS.md | `2280f044576f9eceea825b7b08da56779b6a26e088c13f652736f5543a1a8d3b` | 变化 |
| docs/DECISIONS.md | `157c70c884466a78dee2a0d69adf8adf2b572e38185f124bc640795dc70f3653` | 仅追加 |
| docs/PROGRESS.md | `3977f3581a82e675ac83fbbea9bf38aae708c0d1055131326c4bdc5e799ad6bb` | 仅追加 |
| workflow.json | `e09a371ac0935556695fa2023c163cb25b20be61601193b5a7d3d02ffd50de1f` | 相同 |
| scripts/workflow.py | `793327a31ca73b0bdabf8d3197dae9cb54db95b918f5e9a277b62dcafc67d246` | 相同 |
| scripts/check-project.py | `80bfa5a3ad4f5627705039ced9329fdfa7b0ecdb7127bfc536abd0b0a1d0b5ea` | 相同 |
| scripts/check-contracts.py | `050b17757385b4d7161dc08393c4d01ba0a8f2aba4cefb7f8071662d2721aa78` | 相同 |
| replica-spec/AI_TASKS.md | `89acb3b77e37366e6df6b7645faac06c345620a337d84a1c7d46eee847827c61` | 相同 |
| docs/SOURCE_MANIFEST.json | `8e92ab4f72a498d5c64a05dbac23c5839197021423e02edf575ef5838c4d127a` | 相同 |
| protocol/app.schema.json | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` | 相同 |
| protocol/bridge.openapi.json | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` | 相同 |
| protocol/examples/contract-cases.json | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` | 相同 |

ADR 分段摘要沿用 round-3/4 的方法：从本 ADR 标题起，到下一个 ADR 标题前为止，取原始 UTF-8 内容。

| ADR | 摘要 | 说明 |
| --- | --- | --- |
| ADR-001 | `9377c96f9791c3416582254a4df6fef38676e67f0f2e1b780a99e737b80b7f21` | 与 round-4 相同 |
| ADR-002 | `52d26c5e832ad756de1a9b5a4655759a270facf46b333bd1c9f149a34c864fa9` | 与 round-4 相同 |
| ADR-003 | `0372eebdfa8a92e0536f37ead4f4ba87f28e7b5aaa852f89308d036abf708975` | 与 round-4 相同 |
| ADR-004 | `2c96f53b7a0a87fbffed9600b27879a6c33a01bd20c1f6363acb687ec9dfbab6` | 与 round-4 相同 |
| ADR-005 | `470f3f9ede30afe65f05a7db1a54306f84f3678cb14d6a0d51b6d15b4299585e` | 正文未改；后面追加了 ADR-006，本段多一个分隔空行，所以摘要和 round-4 的 `b87a60a6…` 不同 |
| ADR-006 | `332ee2125a2553e976dc3dab5dace5e30775fd962f4f01fbb31d3c49768ab01d` | 新增 |

### 增量来源核对

**TASKS.md。** 在当前文件上做 4 处逆替换，得到的摘要正好是 round-4 受审的 `170b29cab42e…35cc4`，说明本批正文改动只有这 4 处：
1. 第 5 行“平台范围（ADR-005，…）”改为“（ADR-005/ADR-006，…）”。
2. T0.28“独立验收”中，`evidence/independent-review/T24/<轮次>/report.md` 改为 `…/round-<N>/report.md`。
3. 同一句里，“t24.py只读取最新轮次”改为“只读取N数值最大的轮次”。
4. T0.29“阶段门”句末追加“门禁记录与RESOLVED各自由监管以`docs:`提交，保持完成树干净。”

**DECISIONS.md。** 去掉 ADR-006 段后的摘要等于 round-4 的 `2bf9babe…10a4`，属于纯追加。

**PROGRESS.md。** 相对 HEAD 的 diff 没有删除行，只追加了 ADR-005、round-4 和 ADR-006 三条记录。

**ARCHITECTURE.md。** 与 round-4 字节一致；相对 HEAD 的差异就是 round-4 已审过的 ADR-005 改动（C12 和 C14）。

## 需求覆盖结论

### 结构与追溯（脚本复核）

- 覆盖表共 113 行，没有指向不存在的卡号/验收 ID 的目标。
- 每张卡“需求”行的 ID 集合，与覆盖表按卡号反查的结果完全一致，两个方向都没有差异，也没有只出现在卡里的 ID。
- 36 项验收都是 kind=test，并带 `report: junit:evidence/acceptance/<原卡号>-<suite>.xml`。argv 里的 `--report`、`--junit` 和 report 字段两两一致，文件名后缀与 suite 对应。
- 没有残留对 T0.26/A3..A6，或 macos-x86_64、windows-x86_64、linux-x11、linux-wayland 套件的引用。

### 未回退的项

本批 4 处文字改动没有触及以下内容：
- 卡号、依赖、原卡全文和原始义务；
- T18 依赖 N02，以及 C16、ADR-003/004 正文；
- SPEC §13 的性能指标、单位、时长和阈值；
- T22 的单项 7200 秒超时；
- workflow.json 和控制器。

round-4 已确认落位的 ADR-005 各点都还在原位置：
- 平台范围段；
- C14/C15 的 macOS 改写；
- T0.12 范围的 App.vue，T0.21 范围的 Cargo 清单；
- N02/A1 的 DASH 补查；
- T21/A1 的同一受控自有媒体集合；
- T0.27/A1 缺 Apple 凭据时的处理。

### 平台排除的授权来源

授权来源已补齐，见 R4-B1 复核。macOS x86_64（SPEC 第 39 行、第 252 行）被排出必需目标，现在有了用户确认的来源。

### 结论

覆盖充分，没有发现遗漏、虚假覆盖、无来源扩展或未经批准的删减。

## round-4 问题复核

### R4-B1 已解决：macOS x86_64 降级的授权

**用户原话核对。** ADR-006 的“来源”引用的原话，与本轮外层提示转述的原话逐字一致：

> 我确认：macOS 只要求本机 arm64，Intel（x86_64）列为 NOT RUN。按此追加 ADR-006，顺带修复 N1、N2，然后再启动独立复审。

这正是 round-4 给出的选项 A：用户确认后追加 ADR，正文不改。

**范围核对。** ADR-006 把 x86_64 具体写成以下 5 项，均列为 NOT RUN 非阻断记录项：
- x86_64-apple-darwin/universal 构建；
- Rosetta 运行；
- Intel 安装包；
- Intel 性能；
- x86_64 AVD。

这 5 项都在用户所说“Intel（x86_64）”的范围内，与 round-4 R4-B1“影响”段列出的项目一一对应。
- x86_64 AVD 对应 SPEC 第 252 行 macOS Intel 行的 x86_64 镜像。
- universal 构建包含的 arm64 部分，仍由必需的 aarch64 构建覆盖。

**越权核对。** ADR-006 只确认平台范围，再加 N1、N2 两处执行细节，没有扩大到其他需求、卡号、依赖或验收。

**正文核对。** 正文只在 TASKS 第 5 行加了 ADR-006 标注。ARCHITECTURE C14、覆盖表 U04/R07 仍写 ADR-005，两者内容一致。

### N1 已解决：门禁记录需要提交

**修订内容。** T0.29 阶段门补了：“门禁记录与RESOLVED各自由监管以`docs:`提交，保持完成树干净”。

**控制器行为核对（scripts/workflow.py）。**
- `completed()` 在 `receipt.head..HEAD` 的提交里找主题为 `T0.29:` 且 snapshot 匹配的那条。只改 PROGRESS 的 `docs:` 提交不改变 snapshot，因为 PROGRESS 属于 DYNAMIC。
- PROGRESS 前缀摘要按完成提交那一版计算。
- 所以 T0.29 在后续 `docs:` 提交之后，仍判定为完成。
- `check_completion_tree()` 要求 porcelain 为空且 snapshot 一致。两条记录各自提交以后，这两个条件都成立。
- evidence/ 已被 .gitignore 排除，不会影响 porcelain。

**授权核对。** 该操作符合 AGENTS.md 的规定：“独立文档维护使用 `docs: <摘要>`”和“仅在已有授权……内代办 Git”。

### N2 已解决：T24 轮次排序

**修订内容。** 目录固定为 `round-<N>`，t24.py 按 N 数值取最大的轮次，round-10 排在 round-9 之后的问题消除。

**拒绝能力核对。** 以下 4 种情况仍然判定失败：
- 提交/摘要与本次构建不一致；
- 末行缺失；
- 末行为 BLOCKING；
- 复制旧报告。

## 逐项问题

没有阻断问题。下面是非阻断记录。

### N1′（非阻断，可选文字收紧）

- **需求 ID**：U04、R07、REQ-T23。
- **位置**：TASKS 第 5 行“平台范围”段；T0.27/A1。
- **说明**：
  - 第 5 行列出的降级项是“运行、安装、性能与签名要求”，没有写“构建”。
  - T0.27/A1 的 NOT RUN 句只点名了 Windows WebView2 和 Linux GStreamer。
  - x86_64/universal 构建的 NOT RUN 地位，目前要靠 ADR-006 和平台范围段的“各卡原始义务……降为非阻断”合起来确定。
  - ADR-006 写“正文平台范围无需改动”，但第 5 行实际加了 ADR-006 标注。这只是标注，不影响要求。
- **影响**：实施者读得出正确范围，验收强度不变，不影响判定。
- **最小修订（可选）**：在第 5 行的“运行、安装”前加“构建、”；或在 T0.27/A1 的 NOT RUN 句里补“macOS x86_64”。不改也可以。

### N3（沿用 round-4 说明）：skipped 与需求 ID 覆盖由验收脚本自己执行

- 控制器对 skipped 只计数，不判定。
- “macOS 必需项 skipped 视为失败”和“每个需求 ID 至少一条 testcase”要靠各卡 t*.py 退出非零并写入 failure 来落实。
- 每卡的独立巡检需要核对这两条。

### N4（沿用 round-4 说明）：资源与超时预算待开发前复核

- **超时**：T0.26/A2 和 T0.2/A1 的完整长测时长逼近单项 7200 秒。
- **内存**：memory_mib=4096 是进程树 RSS 采样。AVD 配 3072 MB，加上宿主进程，可能超限；WKWebView 的 XPC 子进程可能不在进程树内，采样会少计。
- ADR-005 和 C14 已要求开发前按实测复核，超出预算时先报告，按授权调整配置并重审。

### N5（沿用）：开发与巡检命令尚未配置

- agent_command 和 review_command 都是 null。
- T0.29 阶段门以独立巡检 CLEAR 为前提，开发前需要配置 review_command，并按原流程重审配置。

### N6（沿用）：缺 Apple 签名凭据时的处理

- 缺凭据时产出未签名包并列为发行待办，这与原 AI_TASKS T23 一致。
- 交付报告需写明 G5 发行门槛未达成。

### 证据位置提示

- doc-review 报告和 T24 独立验收报告都位于 git 忽略的 evidence/ 下，只存在本机。
- PROMPT-DOC-REVIEW 规定“原始报告缺失使旧 CLEAR 失效”，迁移或清理工作树时需要保留这些报告。
- 这是既有安排，本批没有引入。

## 实际验证

- `.venv/bin/python -B scripts/task.py status`，审前和审后各执行一次，结果一致：current=null，state、blocked、runs 均为空，protected 为 `0c4710cb…6ecf`。
- `.venv/bin/python -B scripts/task.py check`：`roadmap OK: 35 cards`。
- `.venv/bin/python -B scripts/check-contracts.py`：PASS，17 命令、179 结构用例、89 语义用例、6 生命周期向量。
- scratchpad 只读脚本 `/private/tmp/claude-501/-Users-eason-jp/91ce2c43-8451-4166-890a-47182011d28d/scratchpad/chk.py`（导入原 workflow.cards）：113 行覆盖表，坏目标为空，双向不一致为空，argv/report/junit 不一致为空。
- 用逆替换重建 round-4 的 TASKS，摘要与 `170b29ca…35cc4` 一致；DECISIONS 去掉 ADR-006 段后与 `2bf9babe…10a4` 一致。
- `git diff` 和 `git status`：PROGRESS 没有删除行；ARCHITECTURE 与 round-4 字节一致；其余受保护文件与 HEAD 一致。
- 关键字检索 x86_64、Intel、Rosetta、universal：
  - TASKS 第 254 行是 T01 原始义务原文，由平台范围段统一覆盖；
  - 其余命中都是降级或 NOT RUN 的说明。
- 阅读 workflow.py 中的 blocked、completed、check_completion_tree、next_card、review、unreviewed_card，确认 N1 的提交路径可行。
- 核对 PROGRESS 引用的 verify 收据 `evidence/verify/20261026T063151250090.json`：
  - status=finished，exit=0，protected 与当前一致；
  - 6 个 verify-* 运行的 exit 都是 0；
  - verify-workflow 98 通过、0 失败、7 跳过。
- PROGRESS 提到日志中有一行 exit=130，对应收据 `20261026T063238063363` 在 evidence/verify 中不存在，与“来自控制器自测夹具”的说法一致。

## 未验证项

- 用户原话由本轮外层提示和 ADR-006 转述，审阅者看不到聊天原文。本轮只核对了两处转述逐字一致，以及授权范围与 round-4 选项 A 对应。
- 完整 verify 不是审阅者运行的，结论依据上面的收据。
- 35 卡业务验收全部 NOT RUN：tests/acceptance/*.py 和 src-tauri 都还不存在。
- 以下几项未实测：macOS 14 最低版本兼容；本机长测的实际时长和峰值 RSS；AVD 预算；WKWebView XPC 进程的 RSS 计入情况。
- 本文档 CLEAR 只确认规划充分，不代表实现已经验证，也不授权开始开发。开发需要用户另行明确指示，并配置 agent_command/review_command 后重审配置。

DOC-REVIEW-VERDICT: CLEAR
