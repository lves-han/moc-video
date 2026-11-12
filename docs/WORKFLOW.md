# 墨客视频

## 当前任务验收规则（2026-11-10）

按用户要求，自 T0.34 起每张任务卡只验收本卡内容；其他任务仅在本次改动影响其相关行为时做必要回归。任务卡只列直接验证需求所需的检查，不为凑覆盖重复已有任务。实际验证结果和未验证事项如实记录。

每卡不再强制执行全量 verify、Git 提交或独立巡检；按变更风险和需要自行选择相关检查。冻结文档或任务定义按用户已授权范围调整，无须为每次调整手工更新基线或先做独立文档复审。已完成的 T0.28–T0.34 验收标准不得擅自扩大，确需扩大须用户批准。保留已有状态、证据、失败记录及当前任务范围；任何任务都不得伪造验收。

本节优先于下文与之冲突的逐卡全量验证、提交、独立巡检和冻结修订流程；显式任务需求、用户决定以及必要的安全和数据完整性检查仍然有效。


本仓库预置“单会话单卡”的 agent 开发流程，可配合任何能非交互运行的 agent CLI（如 Codex、Claude Code）。业务目标、技术栈和真实验证入口由规划阶段确定。
根目录 `AGENTS.md` 是唯一规则文件，内置完整的全局开发原则并追加项目工作流规则；`CLAUDE.md` 只导入它，其他工具若不自动读取 `AGENTS.md`，在其配置中指向该文件。

需要 Python 3.11+ 和 Git。脚本只用项目 `.venv` 的解释器以 argv 调用（POSIX `.venv/bin/python`，Windows `.venv\Scripts\python.exe`），依赖见 `scripts/requirements.txt`。不需要 Bash、GNU timeout 或 WSL。

## 初次使用

1. 完整需求放在 `docs/SPEC.md`（初始化时 `--spec` 导入）。按 `docs/PROMPT-PLAN.md` 编写契约和任务卡，在 TASKS.md 维护需求 → 卡 → 验收覆盖；不擅自增加或缩减产品要求。
2. 配置真实业务 verify，按 `docs/PROMPT-DOC-REVIEW.md` 由独立只读 agent 审查固定文档版本。报告原样保存在 `docs/reviews/doc-review/round-<N>.md`；作者修订后复审直至 CLEAR。文档定义或配置变化使旧 CLEAR 失效；基线使用 `task.py status` 的 protected 摘要及相关已采纳 ADR，正常完成勾选与 PROGRESS 记录追加不失效。`task.py check` 不等于需求覆盖正确，业务验收此时写 NOT RUN。
3. 执行 `scripts/validate.py`、`scripts/task.py check` 并提交初始文档。默认业务入口会失败，模板自检不代表业务完成；开发命令未定可保留 null，不影响文档阶段交付。
4. 用户说“按照开发文档执行开发任务”或同义指令后，外层 agent 按 `docs/PROMPT-START.md` 自动启动（默认授权见该文件）；由外层 agent 确认有效文档 CLEAR、两条非交互 CLI、预算和权限，建立当前聊天定时监管（默认每 5 分钟），随后后台启动 `scripts/drive.py --once`。每次续卡仍用 `--once`；不增加会话/时长，不要求用户手工运行命令。
5. 定时监管按 `docs/PROMPT-SUPERVISE.md` 操作原脚本。每卡完成后仍由原 `review_command` 启动独立只读巡检。Git 收尾后或完成提交待补巡检时，可用 `scripts/drive.py --review-only` 单独巡检，不重复开发。用户暂停/停止优先，同时停止原驱动并暂停监管；全部完成后停止监管。

文档获同意不等于开发授权，文档阶段不启动开发或定时任务。调度工具不可用/创建失败时停在可启动状态，不伪称已开启监管。已有项目接续前读当前规则和状态，不重新初始化或覆盖已冻结文件。

## Agent 命令

驱动器每个会话启动一次 `agent_command`：提示词经 stdin 传入，参数中的 `{prompt_file}` 替换为提示词文件路径；输出写入该会话目录的 `agent-output.log`。命令须在项目根目录非交互运行、结束后退出。模型、权限和沙箱参数都写在这个 argv 里，由用户确认；不自动加入跳过审批或沙箱的参数。示例：

```json
"agent_command": ["codex", "exec", "-s", "workspace-write", "--color", "never", "-"]
"agent_command": ["claude", "-p", "--permission-mode", "acceptEdits", "--allowedTools", "Bash(.venv/bin/python:*)", "Bash(git:*)"]
```

`agent_command` 为 `null` 时驱动器拒绝启动。

## 巡检 agent

`review_command` 与 `agent_command` 规则相同，但应只授予只读权限，例如：

```json
"review_command": ["codex", "exec", "-s", "read-only", "--color", "never", "-"]
"review_command": ["claude", "-p", "--allowedTools", "Read", "Grep", "Glob", "Bash(git log:*)", "Bash(git show:*)", "Bash(git diff:*)"]
```

每张卡完成提交后，驱动器用 `docs/PROMPT-REVIEW.md` 启动一个新的巡检 agent 审查该卡和固定提交，限时 `review_seconds`。输出存入 `evidence/reviews/<巡检ID>/output.log`，结论存入同目录 `review.json`。报告最后一行须为 `REVIEW-VERDICT: CLEAR` 或 `REVIEW-VERDICT: BLOCKING`。
只有 CLEAR 才推进下一张卡。BLOCKING、缺少结论、非零退出或超时都会让驱动器以 `review_blocked` 暂停（退出码 13）。重启时：上次是 BLOCKING 且提交未变则继续拒绝；提交已变（已修复）或上次巡检中断则先重新巡检；最近完成的卡若没有巡检记录（例如驱动器在完成提交后崩溃），也先补做巡检。用户明确接受某个 BLOCKING 结论时执行 `scripts/drive.py accept-review <巡检ID>`。底层脚本仍兼容 `review_command=null`，但本技能的完整开发流程必须配置独立只读巡检。`scripts/drive.py --review-only` 复用同一驱动锁和巡检证据，只审查已具备有效完成证据的卡，不启动开发 CLI 或增加会话；要求干净工作树且没有运行中的重命令。已有完成卡的修复若不满足受验完成提交规则，先处理基线/重新验收，不能用巡检绕过。可执行文件须是原生程序、Python 脚本或显式解释器 argv；Windows 上的 npm `.cmd` shim 由运行时解析为 Node 加 JS 入口，参数不经 shell/batch 求值，其他 `.cmd`、`.bat` 拒绝执行。
模型的最终消息只作交接参考；完成由验收收据和完成提交判定。

## 定时监管代理

外层代理按 `docs/PROMPT-SUPERVISE.md` 在实际项目中启动、监控、核验收尾和接续开发；独立巡检 agent 只读，不承担这些写操作。定时提示词保存真实项目路径、venv、授权原文/来源、范围和停止条件，每次从现有脚本和证据重读卡号、HEAD、PID，不固定历史快照。

`drive.py status` 只返回持久化状态，须用 PID＋创建时间核对实际进程、真实锁占用及收据。存活的开发/验收/verify/巡检只监控；结束后主动执行已授权交接。仅 Git 受阻时等旧写入者退出，核对范围、源码/冻结摘要与同基线单卡验收和完整 verify，再按原 done/提交流程收尾，单独巡检 CLEAR 后续卡。原脚本拒绝证据时查根因，不另造状态或放宽质量门。

监管复用宿主调度工具，在 Codex 桌面默认当前聊天 heartbeat；默认每 5 分钟，遵守用户指定频率。记录真实任务 ID，失败不伪报；CLI 不提供桌面定时任务管理。暂停/停止和范围变更遵守最新人类指令，授权缺失、预算耗尽或无法在授权内处理的质量门必须报告；禁止自己接受 BLOCKING、重置失败/额度或改冻结内容。没有实质变化保持安静。

## 项目、性能与真实集群约束

规划时将用户给出的项目路径、性能目标与运行限制提炼为 ARCHITECTURE 中有来源的执行契约，映射到相关任务卡和验收；文档审查与开发巡检均核对。真实信息不用于示例分析或测试夹具。
绑定唯一项目根目录、工作目录、venv、原脚本、驱动锁和 evidence，接续沿用原状态与证据；共享重锁沿用原全局路径。性能保留指标、单位、阈值、负载/规模、环境、测量方法、适用阶段及合格断言，按当前批准的 SPEC、TASKS、ADR 或用户指示区分硬门槛、目标和本机校准，不擅自升降强度。
所有真实集群服务成员必须由 Docker 部署；宿主仅客户端、报告和监管，不提供原生服务 fallback。每容器设置硬内存上限，其合计与宿主采样 RSS 合并核算批准总预算。部署、限额、用量及成功/失败/取消时自产资源清理均须有证据；缺失时不能声称验收或资源监管通过。

## 环境与控制

控制脚本支持 Linux、macOS、原生 Windows 10+。系统用户总线被 agent 沙箱拒绝时不能自行扩大权限，需要用户选择合适的受控运行环境。初始化和模板测试不访问在线模型。

| 平台 | 内存保护 | CPU / 并行保护 | 文件锁 |
| --- | --- | --- | --- |
| Linux | systemd/cgroup 硬上限；swap 禁用 | cgroup CPU quota＋固定构建 jobs | flock |
| macOS | 每约100ms采样进程树 RSS，超限停止自己的进程树 | 固定构建 jobs、测试线程1；没有 CPU quota | flock |
| Windows | 每约100ms采样进程树 RSS，超限停止自己的进程树 | 固定构建 jobs、测试线程1；没有 CPU quota | msvcrt 字节锁 |

macOS/Windows 的采样保护允许瞬时超限，不提供硬内存隔离；只计入本进程树，由系统服务托管的子进程（如 macOS WKWebView 的 XPC WebContent 进程）可能不在树内而被少计，涉及 WebView/模拟器的验收应另行记录系统级内存；收据标记 `rss-sampled` 和 `build-jobs`。Linux 用户 scope 不可用时失败，不自动换成采样或无上限执行。所有平台使用 Python 超时、PID＋创建时间核对与进程树清理；外部服务仍须在项目测试中负责清理。

Windows 重任务在 CreateProcessW 时原子加入 Job Object，不允许子进程 breakaway；根进程结束后清理全部 Job 成员，执行器异常死亡时由 kill-on-close 终止整个 Job。所需 API 不可用或无法建立 Job 时明确失败，不启动无监管 payload。POSIX 重任务按独立 session 清理，根进程快速退出也不能隐藏同 session 的子进程；macOS 执行器被 SIGKILL 时不能保证主动清理，恢复前须核对旧收据、进程和锁。重任务不支持 daemonize/另建 session 逃离监管；需要外部服务时在项目测试中显式负责其完整生命周期。

`scripts/drive.py status` 查询状态；`scripts/drive.py stop` 停止驱动器和开发会话。独立运行的重命令可能继续完成，下一次启动会等待旧运行结束；超时和中断不会重置累计会话数。
默认每会话3小时、每卡2会话，显式 `--session-hours`、`--max-sessions` 可由用户调整。`--once` 只启动一个会话。

`scripts/run.py A1 -- <准确argv>` 留下命令、输出、真实退出码、测试统计、源码快照摘要和资源配置；失败日志不可删除。测试报告支持 unittest、cargo、pytest 和 `junit:<相对路径>`；test 类型必须有非零通过数，JUnit 文件必须来自本次运行。unittest 的预期失败单独统计，既不算真实失败也不算通过；只有预期失败的测试集不能通过验收，unexpected success 仍视为失败。
验收是 TASKS.md 的 JSON 命令块；不要同时维护另一份机器任务清单。卡片可列 `cases`（冻结的 JUnit testcase 名）与 `skippable`，控制器要求每个用例出现且通过，未声明可跳过的用例 skipped 即失败。
选卡：按文件顺序取第一张依赖已完成、未被阻塞的卡。标题含卡号的 BLOCKED 只冻结该卡及依赖它的卡，标题无已知卡号时全部暂停；被阻塞的当前卡须先提交 WIP 才能转向，恢复后范围只计算它自己活动期间的改动。`- 阶段门：` 卡需 PROGRESS 中同卡 `- 结果：AUTHORIZED` 与 `- 授权：` 用户原话才可开始。`verify.py all` 只证明配置中列出的检查，不隐含其他集成或性能验收。
verify 配置的 ID 必须以 `verify-` 开头；卡片验收使用 A1、A2 等 ID。每次完整 verify 都先保存运行中收据，最终写真实退出码；完成检查同时采用本卡当前基线的最近单卡验收与最近完整尝试，并核验支撑的日志/报告，失败、中断或证据缺失不能沿用旧成功。完成提交后工作树有改动，或后续提交的源码未经验证时，驱动器明确失败并保留文件。历史已完成卡以其受验提交为基准，不要求与后续卡的源码摘要相同。
默认全局锁位于系统用户固定缓存目录下的 `agent-project-workflow/heavy.lock`：Linux 为 `~/.cache`，macOS 为 `~/Library/Caches`，Windows 为 `~/AppData/Local`，忽略 TMPDIR/TEMP。自定义 `heavy_lock` 必须为绝对路径，相关项目共用同一路径，实际存储符合机器规则。
验收 argv 中 `{python}` 使用启动工作流的解释器。路径使用项目内 `/` 形式，日志统一 UTF-8；源码摘要使用 Git clean 后的 blob 和有效文件模式，兼容 CRLF 和 `core.filemode=false`。
文档审查和独立审查报告提交在 `docs/reviews/`，与 PROGRESS 一样不计入源码摘要和卡片范围。`task.py status` 输出 `protected` 与逐条 `adr` 摘要供审查绑定。`evidence/` 不进入 Git，但恢复依赖它：备份整个项目运行证据，避免丢失收据、失败历史及会话额度。摘要和完成检查用于防止误用证据，不构成对恶意篡改本机文件的隔离边界。

`.github/workflows/script-portability.yml` 在 Linux/macOS/Windows 和 Python 3.11/3.13 上运行同一套脚本验证，并上传 JSON 报告。控制夹具隔离业务源码和真实 BLOCKED 历史；模拟 agent 不访问模型。Linux runner 没有用户 systemd 总线时，仅跳过具名的 cgroup 实机检查并写明原因。CI 只验证工作流基础设施，不代替业务验收；新增平台未经实机/CI 执行不能标为通过。

## 用户批准契约修订后的恢复

仅有 RESOLVED 不会自动接受冻结文件的变化。用户明确批准后，停止驱动器、等待旧重命令结束；完成授权范围内的修订并追加 ADR。运行 `scripts/task.py status` 获取当前 `protected` 摘要，在本卡 BLOCKED 后追加记录：

```markdown
## T0.1 用户决定 <UTC时间>
- 结果：RESOLVED
- 决定：<真实用户决定及批准的修订范围>
- 批准摘要：<status 输出的 protected SHA256>
```

再执行 `scripts/task.py rebaseline T0.1 --protected <同一SHA256>`。脚本核对卡号、决定、摘要、驱动器及重命令状态，保留原 Git 范围基线、累计会话数和失败记录。仅放行本次批准的冻结文件；旧收据全部失效，须恢复同一卡并重跑全部验收及 verify。命令不会增加额度；需要调整额度时另按用户明确决定处理。不通过删除 evidence 或手改状态文件恢复。
