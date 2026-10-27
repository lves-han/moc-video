# 映象视频 决策记录

仅追加：ADR编号、UTC日期、状态、问题、备选方案、最小决定、验证及影响。用户决定注明依据，不伪造授权。

## ADR-001 2026-10-26 — 已采纳：原目录复用与规格归档

来源：用户本聊天“根据这个文件夹里的内容初始化项目 application里已有前端的骨架，可以直接拿来用”，随后明确选择“使用 /Users/eason/jp/application，在原目录接入工作流”。
决定：该目录为唯一项目根；原初始化脚本在相邻空目录生成模板后接入；保留原前端和README，工作流说明放docs/WORKFLOW.md；原规格包逐字节复制至项目内replica-spec/，DEVELOPMENT_SPEC逐字节保存为docs/SPEC.md。现有node_modules/dist保持忽略。
路径适配：规格中application/前缀相对本项目根省略；docs/progress.md统一到docs/PROGRESS.md。规格内17章引用按归档目录读取。控制器编号映射详见TASKS，不修改原控制器。
影响：原输入可溯源且项目可独立携带。当前只执行初始化与规划；CLI选择、开发及监管授权待后续人类指令。

## ADR-002 2026-10-26 — 已采纳：分阶段交付及完整验收

来源：SPEC 1.2、14、16、18.6及原AI_TASKS。
决定：保留29张桌面和6张后续移动卡。HTTP路径先行；最终桌面需完整Cloudstream原包/全部实例/三平台及G0-Core/G0-CS/G1..G5。桌面T25结束后停止监管；M系列等待后续明确授权。当前前端的会话内fixture作为复用基础，真实数据和持久化在各卡单独验收。
所有版本与公网能力沿原规格标记“候选/待实测”；本轮未重新选择产品依赖版本。

## ADR-003 2026-10-26 — 已采纳：独立审查后的最小规划修订

来源：独立round-1报告B1..B4、SPEC11.4/18.6及实际verify失败日志。
决定：T18增加N02依赖，实际卡正文移到其后，编号保持；给需要安装依赖/接入口的具体卡补对应清单锁文件/入口范围；在protocol/初始化原机器契约及样例，完整verify持续校验工作协议、原有17命令/DTO及正负例/语义兼容；归档保持原字节。契约报告改用evidence根目录，消除首次运行父目录缺失。
规划细化：ARCHITECTURE C16固定四个历史IPC及单调写序号/删除屏障，作为T18既定实施内容。该决定由规划作者根据已给规格形成，未将其记为人类额外授权。定义变化继续遵守原冻结修订和独立审查。
验证：结构、真实verify、工作协议的反例实验及第二轮独立审查见PROGRESS。

## ADR-004 2026-10-26 — 已采纳：历史快照接纳与提交资格

来源：round-2独立报告R2-B1/R2-B2、SPEC9.1/11.4/18.6及实际机器字段定义。
决定：C16复用match_id的不透明字符串1..160格式与PlaybackViewSession.session_id原格式。save_history带session_id，复用Session已有记录保有确认身份；捕获/宿主接纳与数据库提交分开，已接纳快照在匹配回收、旧会话关闭/切页后继续提交；旧事件新快照及错误归属拒绝，写序号与删除屏障持续有效，失败最近快照可同序号同载荷重试。T18验收补5分钟GC、延迟最终快照、非UUID及屏障反证。
影响：保持原17命令和原Session网络所有权，明确SQLite历史已要求的合法最终进度保存；未启动业务实现。

## ADR-005 2026-10-26 — 已采纳：macOS first与验收可证性最小修订

来源：用户本聊天“所有跨平台问题都只限定在macos里, macos first,对跨平台不做严格要求，其余的问题按照最小化原则帮我进行修复”，针对当次TASKS审核意见。
问题：五环境硬门使T0.2在T0.1后必然停住且原控制器BLOCKED全局停止；command验收只看退出码；M系列缺控制器级停止；T24独立结论无证据路径；T0.12/T0.21范围缺文件；N02缺DASH补查；T21缺播放器同一媒体集合条件。
决定：必需平台仅本机macOS14+ arm64；macOS x86_64、Windows、Ubuntu X11/Wayland的运行/安装/性能/签名降为非阻断NOT RUN记录项，代码保持cfg可移植，后续另行授权。原始义务原文保留，由TASKS平台范围段统一覆盖。T0.26删除A3..A6四个非macOS套件。全部验收改为kind=test+同次JUnit报告，testcase名按需求ID前缀并逐ID覆盖。T0.29完成后监管追加T1.1 BLOCKED授权门。T24独立报告固定到evidence/independent-review/T24/。T0.12补App.vue、T0.21补Cargo清单，N02/A1补DASH，T21/A1补同一受控媒体集合。缺Apple签名凭据时产出未签名包并列发行待办。
保持不变：docs/SPEC.md与replica-spec归档逐字节保持；卡号、依赖与原卡全文不变；T18对N02的依赖与T22单项7200秒超时保留，后者在开发前按实际时长复核。
验证：task.py check、validate.py与完整verify见PROGRESS；修订后旧CLEAR失效，需按PROMPT-DOC-REVIEW独立复审。

## ADR-006 2026-10-26 — 已采纳：macOS仅本机arm64的用户确认

来源：round-4独立报告R4-B1；用户本聊天原话“我确认：macOS 只要求本机 arm64，Intel（x86_64）列为 NOT RUN。按此追加 ADR-006，顺带修复 N1、N2，然后再启动独立复审。”
决定：确认ADR-005的平台范围，macOS必需目标仅本机macOS14+ arm64；macOS x86_64（含x86_64-apple-darwin/universal构建、Rosetta运行、Intel安装包、Intel性能与x86_64 AVD）列为NOT RUN非阻断记录项。正文平台范围无需改动。
同批最小修订：N1——T0.29阶段门记录与之后的RESOLVED由监管以`docs:`提交，保持完成树干净；N2——T24独立报告目录固定为`round-<N>`，按N数值取最大轮次。
影响：只补授权来源与两处执行细节；需求、卡号、依赖与其他验收不变。修订后需独立复审。

## ADR-007 2026-10-26 — 已采纳：控制器与提示词升级到最新moc-init

来源：用户本聊天原话“帮我修改掉application里的内容  让其与最新的skill匹配”。尚无已开始的卡（task.py status current=null），属于文档阶段修订。
决定：从最新moc-init模板同步scripts/workflow.py、scripts/tests/test_workflow.py、docs/PROMPT-PLAN/DOC-REVIEW/SUPERVISE/CARD/START、docs/TASK-TEMPLATE.md、docs/WORKFLOW.md，AGENTS.md按新模板更新工作流规则并保留“本项目初始化约定”。项目自有的check-project.py、check-contracts.py、SPEC与replica-spec不变。
适配：BLOCKED改为只冻结同卡及依赖卡；T1.1增加`- 阶段门：`由控制器拦截，取代ADR-005中T0.29由监管追加BLOCKED的做法，T0.29改为“移动阶段”说明。每项验收增加冻结`cases`，取本卡“需求”行全部ID，脚本为每个ID输出同名汇总testcase；ARCHITECTURE验收契约同步。T0.2/T0.21/T0.24/T0.26/T0.27/T0.29增加`- 环境：`记录ADR-005/006的平台范围。T24独立报告路径改为提交的`docs/reviews/T24/round-<N>.md`；文档审查报告round-1..5原样复制到`docs/reviews/doc-review/`，此后报告随文档提交。
预算：新控制器核对每卡验收与完整verify超时之和不超过会话额度。T0.26原为14400s+8640s，超过21600s。作者将verify-project超时7200s降为3600s（现实际运行数秒，Rust加入后含fmt/check/clippy/test），合计19440s；超时只会让超时显式失败，验收强度不变。用户可改为恢复7200s并提高max_sessions。
验证与影响：见PROGRESS；受审定义变化，需独立复审round-6。

## ADR-008 2026-10-26T08:22:44.841305+00:00 — 已采纳：按默认授权启动开发

来源：用户当前聊天原话“[$moc-init](/Users/eason/.codex/skills/moc-init/SKILL.md) 现在开始启动开发任务”；按docs/PROMPT-START.md默认授权表执行。
决定：唯一根目录/Users/eason/jp/application；项目解释器/Users/eason/jp/application/.venv/bin/python。agent_command采用["codex","exec","-s","workspace-write","--color","never","-"]；review_command采用["codex","exec","-s","read-only","--color","never","-"]，模型沿用CLI默认值。仅配置两条原为null的命令，控制脚本保持。
范围：TASKS全部29张桌面卡T0.1..T0.29，T1.1移动阶段门及其后续等待单独授权；必需平台沿ADR-005/006仅本机macOS arm64。预算原值：session_seconds=10800、idle_seconds=2700、max_sessions=2、command_seconds=7200、review_seconds=1800、memory_mib=4096、jobs=2、测试线程1、共享重任务锁沿原全局路径。
监管：当前聊天heartbeat每5分钟，取得当前版本独立文档CLEAR且调度工具成功后启动原drive.py --once。Git提交仅T编号/WIP T编号/docs前缀，卡内声明依赖与SDK/工具链及真实验收网络按默认授权代办。若开发CLI写.git受限，外层核验后代办完成提交。
停止：用户暂停/停止、阶段门、全部桌面完成、预算耗尽、需用户决定的阻塞、冻结修订、凭据/设备/付费或必需操作权限受阻时依PROMPT-START/SUPERVISE处理；保留失败证据及累计额度。
验证：命令自检、validate、task check、完整verify、配置变更后的独立文档复审及调度/真实CLI身份核对另记PROGRESS与evidence/drive/supervision.md。业务验收当前全部NOT RUN。ADR-007的AUTHORIZED机制同时取代ADR-006中N1的旧阶段门收尾方式。

## ADR-009 2026-10-26T10:22:51.723468+00:00 — 已采纳：新版CLI与启动前测试隔离

来源：当前聊天用户“再次尝试一下 模型使用`gpt-6.1-sol`”“帮我升级brew里的codex”，以及最新“现在正式开发任务”。继续原开发授权与PROMPT-START默认预算/范围；Homebrew0.164.0已由实际请求验证模型可用。
决定：两条既有Codex exec命令显式加入-m gpt-6.1-sol；开发workspace-write、巡检read-only，沿用PATH已验证的/opt/homebrew/bin/codex。解释器仍/Users/eason/jp/application/.venv/bin/python。
启动前最小验收修订：scripts/tests/test_workflow.py的setUp在复制真实workflow.json后显式review_command=None；要求巡检的测试继续用enable_review配置本地替身。原4项失败由夹具继承真实巡检命令造成，证据evidence/drive/start-validate.log。此一行只修正自测隔离，按PROMPT-START第3步“只涉及配置、措辞或验收可执行性的问题，由外层最小修订后再复审”执行；用户明确指令是正式开始开发，未将此记录为用户逐字批准补丁。原脚本运行逻辑、预算、门槛、任务、需求及失败历史均保持。当前current=null，尚无业务卡冻结基线。此前PROGRESS中的该修复待授权记录由此启动前修订说明接续。
验证：开发写读删除/Git权限和巡检只读自检、原validate、完整verify及独立文档复审结果见后续PROGRESS；独立审阅须核对该修订的范围及来源。取得CLEAR及真实heartbeat后仅drive.py --once启动，每卡完成后独立巡检；29张桌面卡结束或遇停止条件暂停，移动阶段保持单独授权。

## ADR-010 2026-10-27T01:23:11.172247+00:00 — 已采纳：最小权限下开发与巡检验收分工

来源：当前聊天用户原话“你作为agent负责巡检验收，最小化授权  script里的脚本负责开发实现”，回复外层此前提出的“CLI保持workspace-write，外层通过原run.py代跑构建和验收”方案。此为真实分工决定，继续既有桌面开发授权。
决定：原scripts/drive.py --once及其agent_command负责本卡代码、验收脚本和所需输入/依赖清单的开发实现；开发CLI维持gpt-6.1-sol/workspace-write，独立巡检命令维持read-only。外层agent负责核对范围/身份/源码稳定性，通过原run.py执行构建、依赖获取、准确单卡验收，通过原verify.py all执行完整门，并按既有授权Git收尾、记录完成、调用原--review-only取得独立巡检结论及接续。
协作：已知CLI对全局heavy.lock、macOS进程枚举和.git写入的权限限制由执行分工处理。开发会话完成可验证实现及准确命令清单后，以WIP交接并结束；这些已明确移交外层的操作不再作为重复权限探测或新的用户决定阻塞。外层仅在开发/重命令退出、源码稳定后验收；发现实现问题交回原开发工作流修复，外层不代写产品代码。不得以CLI退出0或WIP替代完成证据。
质量门与最小授权：保持原单卡准确A1及JUnit cases、完整verify、task.py done、受验完成提交和固定提交独立CLEAR，既有测试断言、资源限制、共享锁及清理规则均保持。原运行控制脚本、PROMPT-CARD、AGENTS、SPEC、ARCHITECTURE、TASKS、workflow.json和全局配置均不修改；通过原handoff传递本条当前用户指示。无跳过审批/沙箱参数、无新控制状态或后台转发系统。
T0.1处理：解除当前执行分工待决定的BLOCKED；原失败收据、WIP10fbe00、外层probe成功及已消耗1次会话均保留，仍max_sessions=2。保护摘要保持bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a；未发生受保护定义修订，不调用rebaseline。影响执行定义的新增ADR须经独立只读文档复审后，再恢复同卡剩余会话及原监管任务。
