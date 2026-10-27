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

## ADR-011 2026-10-27T02:31:53.226839+00:00 — 已采纳：仅T0.1追加一次版本漂移修复会话

来源：当前聊天用户原话“同意，仅将 T0.1 的开发会话上限从 2 次增至 3 次，让原脚本修复版本漂移问题，你负责复验，现有权限和资源限制保持不变。”
决定：只将T0.1累计开发会话上限从2次增至3次，用于原开发脚本修复已复现的Rustup安装自更新/版本记录漂移问题，外层按ADR-010负责准确复验及原质量门。既有2次会话、两条失败和所有收据保留，不重置。
最小实现：保持workflow.json max_sessions=2及全部受保护文件原值；确认原next_card仍为T0.1且sessions=2后，唯一一次用原scripts/drive.py --once --max-sessions 3启动第3会话。原脚本已支持该非交互参数；此为当前人类明确授权的仅本卡例外。不得将该参数用于其他卡或借此重复追加会话；T0.1结束后其他卡仍原drive.py --once、默认max_sessions=2。独立--review-only不增加开发会话。
限制：gpt-6.1-sol、开发workspace-write/巡检read-only、单次10800秒、原命令超时、4096MiB RSS采样、jobs2、测试线程1和原全局heavy锁均保持；不加session-hours，不修改原控制脚本、全局配置、需求/验收/cases，不代写产品实现，不改弱断言或伪造结果。
基线与接续：保护摘要保持bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a、generation=0，未修改冻结定义，不调用rebaseline。追加本卡真实RESOLVED及原handoff、独立文档复审CLEAR后恢复同一监管任务并进行一次有来源的预算例外。修复后源码稳定再由外层通过原run.py执行新准备/准确A1，全部通过后完整verify及原完成提交/独立CLEAR。


## ADR-012 2026-10-27T05:56:24.422836+00:00 — 已采纳：仅T0.2追加一次构建产物校验修复会话

来源：当前聊天用户原话“同意，仅将 T0.2 的开发会话上限从 2 次增至 3 次，让原脚本修复构建产物校验误判，你负责复验，现有权限和资源限制保持不变。”
决定：只将T0.2累计开发会话上限从2次增至3次，用于原脚本修复已复现的构建产物校验误判及对应回归，外层按ADR-010负责复验。现有2次会话、4条失败、暂停区段及全部原收据保持；ADR-011的T0.1例外已使用完毕，本决定独立生效。
实施：workflow.json默认max_sessions=2及原控制脚本保持。先完成当前独立在途T0.3的正常质量门；原next_card优先当前活跃卡。仅当原current/next确为T0.2、sessions=2、全部旧写入者/重命令退出、此次独立只读文档复审CLEAR后，唯一一次使用原scripts/drive.py --once --max-sessions 3。此局部参数仅用于本卡第3会话，其余新卡普通--once/默认2；保留原计数与失败，不加session-hours，不手工切current或伪造阻塞。
范围：修正tests/acceptance/t01.py对真实打包JS的control/next路径及自动化标记判断，补接受真实产物/拒绝缺失与生产泄漏的回归；保持生产与测试构建隔离、严格断言、原准确A1和GUI长测。外层从稳定源码重新build、准确A1、完整verify、done/Git与固定提交独立CLEAR。编译子命令和15个诊断用例沿原记录保留，实际GUI长测待执行。
约束：gpt-6.1-sol、开发workspace-write/巡检read-only、单次10800秒、原命令超时、4096MiB RSS采样、jobs2、测试线程1、原全局heavy锁及既有桌面范围/移动阶段门保持；外层负责验收/Git，产品与验收实现交原开发工作流。
基线：protected=bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a，generation=0，受保护定义保持，沿原计数恢复，未调用rebaseline。第三会话退出后按原质量门复验；后续需要开发时依原流程记录新决定。


## ADR-013 2026-10-27T06:39:36.732563+00:00 — 已采纳：本项目独立重任务锁

来源：当前聊天用户原话“授权变更，本项目不受共享锁影响”。该真实指令取代此前本项目必须跨项目共用heavy锁的要求；其余授权与限制继续。
决定：复用原workflow.json的heavy_lock配置，设为绝对路径/Users/eason/jp/application/evidence/drive/heavy.lock；原run/verify/drive仍使用同一锁完成本项目内重任务串行。其他项目继续使用各自原锁，本项目独立排队。原driver.lock保持；原控制脚本/开发CLI权限不变，无额外转发执行器或第二套状态。
最小修订：仅修改workflow.json的heavy_lock值及ARCHITECTURE C1对应一句，追加本ADR与当前卡真实BLOCKED/RESOLVED；通过原task.py rebaseline更新T0.2授权基线。此前原共享锁运行/等待及中断收据、历史T0.1/T0.3完成提交与巡检保持。docs/runtime.lock.json中的global_heavy_lock为T0.1当时真实观测记录，保留原历史值；当前路径以原配置及原runtime.lock_path实测为准。新运行明确记录项目独立锁，当前源码/基线下重新build/回归/准确A1/完整verify，不复用旧收据完成。
资源与权限：memory_mib4096的macOS RSS采样、jobs2、测试线程1、所有原命令/会话时长与计数保持。T0.2第三次一次授权已使用、sessions3/原失败及暂停区段保持；其他新卡默认max_sessions2。项目间重任务可并行，占用可能叠加；本项目仍受原单任务资源监管。
冻结与审查：旧protected=bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a；批准的新protected=5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481。原脚本rebaseline保留失败/计数、增加generation并使本卡旧验收失效；变更须独立只读文档round-11 CLEAR后恢复原同ID监管，沿ADR-010外层执行验收/Git、原脚本实现产品。未授权移动阶段保持原阶段门。


## ADR-014 2026-10-27T07:46:37.100030+00:00 — 已采纳：仅T0.2追加一次窗口与JUnit修复会话

来源：当前聊天用户原话“同意，仅将 T0.2 的开发会话上限从 3 次增至 4 次，让原脚本修复窗口尺寸和 JUnit 失败报告生成问题，你负责复验，现有权限和资源限制保持不变。”
决定：仅本卡累计开发上限3→4，唯一一次通过原scripts/drive.py --once --max-sessions 4启动第4会话，沿ADR-010原脚本实现/外层复验。修复真实native窗口与WebView尺寸差异及失败汇总JUnit生成KeyError，补保留失败信息/尺寸语义回归，原准确A1/真实长测/完整verify和完成独立CLEAR保持。
约束：仅原current/next=T0.2、sessions3、所有旧写入者/重命令退出、此次独立文档复审CLEAR后执行该一次局部参数；workflow默认2、原控制脚本、gpt-6.1-sol workspace-write/read-only、4096MiB RSS采样/jobs2/线程1、全部时长/失败规则保持，不追加session-hours或复用其他卡例外。原3会话、6失败、generation1和所有证据保持。
基线：protected=5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481，此决定与追加文档保持受保护定义；不调用rebaseline。第4会话WIP结束后外层从稳定源码重新原build/回归/准确A1（MP4/HLS各30分钟与20生命周期）/完整verify/原done/Git/固定提交独立CLEAR；继续需要开发时依原规则报告新决定。

## ADR-015 2026-10-27T07:46:37.100030+00:00 — 已采纳：严格按卡序推进

来源：当前聊天用户原话“前一任务未完成或阻塞，禁止开启下一个任务”。该指令取代此前跨BLOCKED推进独立分支的授权，立即生效。
决定：外层按TASKS原文件顺序检查最早未完成卡；该卡须当前受验源码准确单卡验收、完整verify、受验完成提交、固定提交独立CLEAR及干净工作树全部成立后，才启动后一张。前卡BLOCKED时仅处理该卡授权内诊断/交接或等待真实决定，暂停开启后卡。原next_card仍有独立分支语义，外层新增启动前顺序核对；保持原控制代码、锁、权限、资源与会话历史，不手写状态。
落实：原drive stop停止已在途T0.13并核对所有身份退出；WIP完整保留在提交b5f979e，外层仅做可逆Git隔离提交ead5af1，将该WIP产品/测试差异的逆补丁应用回当前源码，PROGRESS及原历史保留。当前源码摘要与原b5f979e父提交一致，T0.13实现未验收，按人类真实顺序约束BLOCKED等待T0.1..T0.12完成；原第1会话保留。将来按序到达T0.13时由原工作流在本卡范围恢复保留WIP并验证，不视为已完成。
接续：当前先落实已批准T0.2第4会话；T0.4范围授权已获得，排在T0.2完成后处理。严格顺序和批准记录只追加ADR/PROGRESS/handoff及监管提示词，不自行修改冻结定义或调整其他卡预算。

## ADR-016 2026-10-27T07:46:37.100030+00:00 — 已采纳：T0.4仅补host.rs的范围批准记录

来源：当前聊天用户对“是否仅将 src-tauri/src/host.rs 补入 T0.4 范围，保留其余要求及剩余1次开发会话”明确回复“批准补入 host.rs”。该具体范围决定已获得，后续无需重复确认。
决定：T0.4允许的最小修订仅为补入src-tauri/src/host.rs，保持原验收/cases/义务/预算与会话1/2。按ADR-015先完成T0.2；此处记录真实批准，TASKS范围本次尚未修改。实际修改前按原rebaseline检查当前卡/其他已开始未完成卡及原独立文档复审流程处理；若原控制器对其他暂停未完成卡的限制阻断，保留本批准并报告准确路线约束，禁止手写状态/基线或假完成。


## ADR-017 2026-10-27T09:05:17.718377+00:00 — 已采纳：仅T0.2追加一次长播诊断修复会话

来源：当前聊天外层明确询问“T0.2的窗口和JUnit修复已通过复验，但真实MP4长播约128秒后出现停滞；ADR-014的第4会话已用完。是否仅为T0.2追加一次诊断修复会话（4→5），仍由原脚本实现、我负责复验，权限和资源限制保持？”；用户通过该问题明确回复“批准 T0.2 追加一次”。
决定：仅T0.2累计开发会话上限4→5，原脚本定位真实MP4长播Playback or UI stalled的根因并作最小修复/回归，外层按ADR-010复验。保留失败瞬间可观察证据、原始23样本及有效失败JSON/JUnit；不得凭正常前序样本推定通过或放宽长播/帧/UI/音频断言。原窗口/inset与JUnit修复通过事实沿原证据保留。
实施：独立只读文档round-13 CLEAR后，紧邻启动确认原earliest/current/next=T0.2、sessions4、旧写入者/重命令退出，唯一一次原scripts/drive.py --once --max-sessions 5。原default max_sessions2/控制代码/权限/资源/单次时长保持，无session-hours或计数重置；原4会话、7失败、generation1、全部原日志/报告/区段保持。此例外仅本卡一次第5会话，其他卡无额度变化。
原质量门：源码稳定后原run.py重新build、回归、准确A1（真实MP4/HLS各1800秒、20窗口、原安全/布局/资源清理断言），成功后直接完整verify.py all，再原done/Git/固定提交独立CLEAR。继续需要开发时依原流程报告真实决定。
继承：ADR-015严格顺序，前卡未完整完成或阻塞时不启动后卡；ADR-013项目专属锁、4096MiB RSS采样/jobs2/线程1、原超时与gpt-6.1-sol workspace-write/read-only保持；T0.4已批准范围及原多在途基线路线约束继续等待本卡完成后处理。protected=5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481，受保护定义保持，此预算批准不调用rebaseline。
