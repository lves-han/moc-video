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


## ADR-018 2026-10-27T09:40:23.396203+00:00 — 已采纳：仅T0.2追加一次回归装载修复与长播诊断会话

来源：当前聊天明确询问“T0.2新增Node回归的失败已定位为模块加载器与断言函数同名冲突；ADR-017的第5会话已用完。是否仅为T0.2追加一次修复会话（5→6），修复该冲突并继续长播诊断，仍由原脚本实现、我负责复验，权限和资源限制保持？”；用户针对该问题明确回答“批准 T0.2 追加一次”。
决定：仅本卡累计开发上限5→6，原开发脚本最小修复CommonJS装载器与require断言函数重名问题，保持新增诊断回归及真实长播全部门，并继续根据真实现场定位播放停滞。外层按ADR-010复验；既有5会话/8失败/generation1、所有原收据及WIP保留。
执行：本次独立只读文档复审CLEAR后，紧邻启动核对earliest/current/next=T0.2、sessions5、全部旧写入者/重命令退出，唯一一次原scripts/drive.py --once --max-sessions 6。默认workflow2、原控制代码/权限/资源/时长保持，不加session-hours，不重置计数或复用旧例外。protected=5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481保持，不调用rebaseline。
复验与诊断：原准确A1仍包括真实MP4/HLS各1800秒、20生命周期及帧/UI/音频/安全/布局/清理；成功后完整verify、原done/Git/固定提交独立CLEAR。为减少缺少现场的再次交接，外层可通过原run调用已有NativeSession/Fixture/command执行真实MP4补充诊断，使用当前已构建诊断版、原资源监管和既有断言/清理，保存失败results/samples；此诊断单独记真实结果，准确A1和失败回归仍保持原门。外层不改产品或回归实现。
继承：ADR-015严格顺序、ADR-013项目锁及4096MiB RSS采样/jobs2/线程1、gpt-6.1-sol workspace-write/read-only均保持。后续卡等待本卡完整完成，新的开发需求依原流程报告真实决定。


## ADR-019 2026-10-27T10:02:29.327021+00:00 — 已采纳：T0.2由当前agent持续完成，不设任务额度上限

来源：当前聊天用户原话“你来负责T0.2任务的完成，不做额度限制”。此指令仅针对T0.2，覆盖此前本卡逐次追加开发会话的限制，继续执行严格卡序。
执行：由当前外层agent接手本卡实现、诊断、实际验证、修复和收尾，持续处理至原全部质量门完成；本卡后续开发会话次数及累计开发工时不设上限，不再逐次请求本卡额度。原开发CLI第6会话已WIP退出，WIP与全部历史保留；原sessions=6/失败9/generation1保持，不手写或重置计数。本次由当前agent直接在本卡原范围内实施，不靠扩大CLI沙箱或修改workflow默认会话设置落实。其他卡保持原分工及默认预算。
保持：原memory_mib4096 macOS RSS采样、jobs2、测试线程1、项目专属重任务锁、开发/独立巡检权限及单条构建/验收/verify/巡检命令超时；不取消资源监管。原受保护定义与控制代码保持，protected=5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481，无rebaseline。用户批准的不限任务额度不扩展单条命令、访问、阶段门或其他任务范围。
质量：当前agent修改后先执行相关构建与行为验证，失败在本卡持续修复复验，不以静态交接代替。所有重任务沿原run.py，准确A1保持真实MP4/HLS各1800秒、20窗口、安全/布局/DPI/资源清理及冻结cases；之后直接完整verify.py all、原done/Git/固定提交独立read-only CLEAR，工作树干净才推进。原全部失败/媒体基准/诊断保留，当前问题按实际证据核对；后卡等待。
复审与监管：本次分工/预算新决定须独立只读文档复审，之后同ID监管仅维护当前未完成事项，动态清理过期提示。未来若重新使用原开发CLI，先按原控制器支持及真实当前授权核对执行路线，保持累计历史；当前直接实施无需修改原控制器或伪造新CLI会话。


## ADR-020 2026-10-28T02:07:57.957015+00:00 — 已采纳：开发功能与产品长期验收分期

来源：当前聊天用户原话“长时间的验收放到产品测试阶段，开发阶段只专注于功能实现”。此真实指令取代早期要求T0.2开发完成前运行两格式各1800秒及20窗口的阶段安排；用户此前仅GUI长测稍后安排的等待，由本次分期决定接续。
决定：开发阶段专注功能实现，并实际执行相关构建/类型/行为正负例和必要短时集成，自验证通过再交接；原完整verify、完成提交与独立巡检继续。复用已有open/controls/stop及帧/音频/UI/安全/布局/缩放/资源释放读回验证T0.2，沿原操作超时。其他开发卡既有短时功能循环、边界反例与即时资源清理继续。
产品阶段承接：T0.2的MP4/HLS各1800秒和20窗口耐久归T0.28/A1；T0.25的30分钟真实网关播放归T0.26/A2（A1核对采样/协议准备）。原时长、规模、指标、真实媒体/产品链路、资源与清理要求保持，按届时当前受验源码/构建实跑。开发功能证据与产品长期验收分别记录，最终交付具有完整产品测试证据；SPEC和原始义务归档保留。严格卡序、原资源/权限/单次超时及T0.2当前agent不限累计额度保持。
合法迁移路线：原workflow.rebaseline仅支持当前单卡且拒绝其他已开始未完成卡。现T0.2/T0.4/T0.13共有旧protected=5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481，其中后两卡paused；新分期需要更新冻结TASKS/ARCH并使旧证据失效。已准备最小原入口批次迁移提案，当前用户本次已批准分期，控制器扩展及三卡迁移尚待单独明确决定；本ADR记录待批准路线，未将其写成用户已批准控制器。原冻结文件、原状态/current/失败/计数保持。早已批准ADR016的host.rs范围拟同次落实，文件范围无需再次确认。
审查：独立只读formal_start_doc_review对候选方案给出round16提案CLEAR；候选随后移除自拟开发30秒固定等待，以原功能状态读回复用，最新round17补充提案CLEAR。报告docs/reviews/doc-review/round-16.md与round-17.md原样保存，round17 SHA对应最终候选。两者仅确认提案可审批，实际新控制器/基线/functional与产品测试均待实施验证；实施后须固定实际内容独立复审。
候选位置：evidence/drive/acceptance-stage-change/proposal.md及documents.patch，最终文件摘要见sha256.json。控制器拟仅显式批准集合的原入口迁移，持续持有driver/heavy锁、完整先验校验、保留原计数/失败/区段/current/paused、按同批准事件幂等续写并分别增加一次generation；保持后卡功能阻塞及暂停。候选隔离副本先经现行原run验证，获准应用后原入口迁移再原run复测；禁止手写状态或伪造后卡完成。


## ADR-021 2026-10-28T03:02:50.375835+00:00 — 已采纳：原入口最小三卡基线迁移

来源：当前聊天明确询问“是否批准仅扩展原rebaseline入口完成这次迁移，保留全部历史、计数、卡序与暂停状态，再继续T0.2功能开发”，用户明确答“批准最小迁移并继续”。本次批准覆盖round17分期提案及原rebaseline最小批次扩展，迁移集合仅当前T0.2与paused T0.4/T0.13。
实施：原task.py rebaseline增加显式--include-paused参数，默认单卡行为保持；全程持有原driver/project-heavy锁，校验完整在途集合、当前/暂停身份、真实RESOLVED及准确摘要/迁移集合、append-only、scope、驱动/CLI及执行器/payload身份和未完成收据。全部校验先于逐文件原子保存；沿原rebaselines事件记录cards/previous/new/approval，各卡generation仅加一次，中断按同批准事件幂等续写。计数/失败/区段/base/resume_base/current/paused保持，旧验收失效。暂停卡scope仅其活动segments与未提交文件，修正误计暂停后其他卡提交的问题。
候选自验证：现行原run migration-candidate-tests20261028T025600726273 exit0，104测试、103通过/1平台skip；补finished收据仍存活payload守卫与反例后focused20261028T025951500653 exit0，9项迁移行为通过。候选在evidence隔离测试夹具验证后才应用到原文件，未用旁路执行器或手写实际状态。实际迁移后原run复测、固定内容独立文档复审，再T0.2功能实施/实际验收。
本次批准摘要：a13052cd3dfcc96cb36c056b2f41fcf31b88dcb3b4ce0b5347ffb4e804781db1。同时应用ADR020分期TASKS/ARCH与已获ADR016的T0.4 host.rs范围；SPEC、AGENTS、workflow配置、资源/权限/单条时限保持。T0.13顺序BLOCKED与隔离WIP保持，T0.4解除已批准范围问题后仍paused按序等待，当前卡继续T0.2。


## ADR-022 2026-10-28T05:34:12.866941+00:00 — 已采纳：仅T0.4追加一次功能失败修复会话

来源：当前聊天询问“T0.4已通过编译，17项原生功能测试有11项通过、6项失败，主要涉及返回数据的协议校验和超限错误分类。原两次开发会话已用完。是否仅为T0.4追加一次修复会话（2→3），仍由原脚本修复、我负责复验，权限和资源限制保持？”；用户明确回复“批准 T0.4 追加一次”。
决定：仅本卡累计开发上限2→3，一次第3开发会话用于定位/修复原A1的6项功能失败并做相关自验证；原gpt-6.1-sol workspace-write CLI实施，外层沿ADR010使用原run/verify验收与Git收尾。现2次会话/2条失败/generation1及WIP27b6204、原准确A1报告与全部日志保留。
实施：本次独立只读文档复审CLEAR后，紧邻启动确认最早未完成/current/next均T0.4、原sessions2，旧驱动/CLI/执行器及重命令均退出、两锁空闲，仅一次原scripts/drive.py --once --max-sessions 3。该参数仅本卡本次，启动后原计数3即已消费；workflow默认2及其他卡预算保持，不加session-hours、不重置计数、不复用其他卡例外。
验证：5项Host结果被工作协议拒绝和1项超限分类失败以原Rust日志为依据，具体根因由原开发CLI核实，保留原schema/协议、原反例、17行为及全部9需求门。修改后实际相关构建/行为自验证通过再正常交接；遇既知sandbox无法实跑时沿原分工准确交接未验证项，避免重复权限探测/扩大权限。外层稳定源码重新locked构建、准确A1、直接完整verify、原done/Git及固定提交独立CLEAR，前卡完整闭环后才依序下一卡。
保持：原4096MiB macOS RSS采样/jobs2/线程1、项目专属重锁、全部单条/会话时限、开发/只读权限及阶段/范围/顺序。protected=a13052cd3dfcc96cb36c056b2f41fcf31b88dcb3b4ce0b5347ffb4e804781db1、generation1保持，未修改冻结定义/控制器，不调用rebaseline。当前原source=7a4517bffdd377357236fa36d34bff1ce27b7c46005144b8b7fbe89d2476c446，此为运行前定位，后续每次从原状态核对。


## ADR-023 2026-10-28T06:44:25.818813+00:00 — 已采纳：T0.5仅补host.rs范围并联动暂停基线

来源：当前聊天询问“是否仅将host.rs补入T0.5，并通过现有原入口同步迁移T0.13的暂停基线，保留全部历史、暂停状态及T0.5原两次开发预算；本卡尚未消耗开发会话”，用户明确回复“批准 T0.5 补入 host.rs”。
决定：仅在T0.5 TASKS范围行增加src-tauri/src/host.rs，以将Stremio同步/内容适配接入原Host及其Job/提交锁/配置世代/取消语义。原义务/checks/cases/依赖、默认2次开发预算及权限/资源/单条时限保持。范围预检已完成且原sessions0/失败0，尚无开发CLI或新产品代码。
基线：应用独立提案round21核对的单行候选；批准新protected=01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d，使用已存在原task.py rebaseline T0.5 --protected 新摘要 --include-paused T0.13迁移当前及唯一paused未完成卡。保留原计数/失败/base/resume_base/segments/current/paused与T0.13顺序BLOCKED及隔离WIP；仅分别更新基线事件/generation，不新增控制器变更或恢复后卡开发，旧证据按原规则失效。已完成卡的历史受验提交/质量门保持。
执行：原入口迁移后核对状态保持/准确摘要/范围/原task check，对固定实际文档与迁移做独立只读复审CLEAR，再恢复同ID监管并普通drive --once/default2启动T0.5第1开发会话。沿ADR010原脚本开发与外层验收/Git、ADR020功能/产品分期及严格卡序。


## ADR-024 2026-10-28T09:38:37.977722+00:00 — 已采纳：仅T0.5追加一次枚举修复会话

来源：当前聊天询问“原两次开发会话已用完，是否仅追加一次（2→3），由原脚本修复分组dub_status枚举不符并自验证、外层复验、权限资源保持”，用户明确回复“批准 T0.5 追加一次”。同一回复对真实扩展DNS调整条件明确答“已调整，可重新核对”。两项分别记录为开发授权与环境事实，网络就绪以实际重新核对为准。
决定：仅T0.5累计开发上限2→3，一次第3会话用于现videos()的dub_status非法值映射修复与相关回归；原gpt-6.1-sol workspace-write CLI实施，外层沿ADR010验收/Git。原19行为、全部协议/schema/字节预算/13需求及强制真实en/zh门保持，保留原两次会话、三条失败与WIPf1c591d，禁止将ContentStatus合法unknown等其他语义一并替换。
执行：此次独立文档复审CLEAR后，紧邻核对earliest/current/next=T0.5、sessions2、旧进程/重命令退出与原两锁空闲，仅一次原drive.py --once --max-sessions 3；原state达到3即本次许可已使用，其他卡默认workflow max_sessions2保持，不加session-hours、不重置计数或复用其他卡例外。
网络复核：用户确认调整后，原受控n01-dns-recheck收据evidence/runs/20261028T093539289489-n01-dns-recheck/receipt.json exit0，系统及resolv.conf A均为真实公网104.26.8.49/104.26.9.49/172.67.74.162，AAAA无记录。此仅为解析条件通过；实际生产HTTP的TLS/peer、部署版本及同ID中英文响应仍由修复后原A1证明，原公网安全策略保持，外层未改系统/代理配置。
约束：原4096MiB RSS采样/jobs2/线程1、项目锁、全部会话/命令时限、开发/独立只读权限、严格卡序及功能/产品阶段保持。当前protected=01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d、generation1与源码81c9bab9d5fa26bd0824f34774688b6afa54ebdc5cb8cbcddab749278aa69476保持，未改冻结定义，不调用rebaseline。修复后原locked构建/准确A1/直接完整verify/done/Git/固定提交独立CLEAR；后卡等待。


## ADR-025 2026-10-28T10:07:20.734522+00:00 — 已采纳：仅T0.5追加一次安全脱敏修复会话

来源：当前聊天询问“T0.5的19项原生测试和真实中英文响应已通过；准确A1剩余失败已定位为测试日志把图片凭据脱敏成REDACTED，导致43字符格式校验失败。ADR024的第3会话已用完，是否仅追加一次（3→4），由原脚本修复安全脱敏并自验证、外层负责完整复验，权限和资源限制保持？”；用户针对该问题明确回复“批准 T0.5 追加一次”。
决定：仅T0.5累计开发上限3→4，唯一一次第4会话最小修复测试观测安全脱敏与独立schema校验的兼容，并执行相关自验证。原gpt-6.1-sol workspace-write CLI实施，外层按ADR010原run/verify复验及Git收尾。原19项行为、协议/schema、字节预算、真实en/zh门、安全校验与全部13需求保持；保留原3会话/4失败/generation1/WIP61cb8b5与全部原日志/报告，禁止输出真实图片凭据或通过合法占位掩盖非法生产值。
执行：本次独立只读文档复审CLEAR后，紧邻核对earliest/current/next=T0.5、sessions3、旧驱动/CLI/执行器/payload退出与原两锁空闲，仅一次原scripts/drive.py --once --max-sessions 4。原state到4即许可已消费；其他卡与workflow默认2保持，不加session-hours、不重置计数或复用旧例外。
已确认事实：原locked构建20261028T095356724301 exit0；原准确A1 20261028T095411721445 exit1，19原生通过，独立校验3/162观测的脱敏capability不合43字符pattern。call()先按原Contracts检查真实响应再脱敏；原两项受控只读诊断确认问题并保留原准确A1失败。真实TMDB3.1.7四份en/zh响应hash及tmdb:27205的Inception/盗梦空间已诊断核对，修复后仍完整执行真实门。
质量与保持：修复后实际执行相关构建/行为正负例/脱敏报告路径验证，通过后正常交接；已知sandbox限制按既有代验路径准确交接，原执行器及最小权限保持。源码稳定后外层原locked构建、准确A1、直接完整verify、原done/Git及固定提交独立CLEAR；后卡严格等待。4096MiB RSS采样/jobs2/线程1、项目锁、全部单条及会话时限、权限/范围/阶段保持。当前protected=01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d、source=47922d7c4417d34e0701c7d70a693cb802647f576894c9e41f66269d9486174b保持，无冻结定义或控制器修改，不调用rebaseline。


## ADR-026 2026-10-29T01:11:21.611160+00:00 — 已采纳：仅T0.5追加一次规范引用Clippy修复会话

来源：当前聊天针对“T0.5准确功能验收已全部通过，完整verify最后停在stremio.rs:1056的Clippy告警，需要调整临时字符串比较，并保留引用规范性校验；第4会话已用完，是否仅追加一次（4→5），由原脚本最小修复并自验证、外层完整复验，权限和资源限制保持”，用户明确回复“批准 T0.5 追加一次”。
决定：仅本卡累计开发上限4→5，唯一第5会话用于原范围内规范引用字符串比较的最小修复及相关自验证。原gpt-6.1-sol workspace-write CLI实施，外层按ADR010原run/verify复验和Git收尾。保持canonical序列化文本精确相等与非规范引用拒绝；禁止用仅Value语义相等替代或抑制告警。原协议/schema、19项行为及安全脱敏正负例、真实en/zh门、字节预算、全部13需求保持。
执行：本次固定实际文档独立只读复审CLEAR后，紧邻核对earliest/current/next=T0.5、原sessions4、旧驱动/CLI/执行器/payload身份退出且两锁空闲，仅一次原drive.py --once --max-sessions 5。计数达到5即本次许可已消费；其他卡和workflow默认2保持，不加session-hours，不重置计数/失败/历史或复用已消费许可。
证据：原准确A1 20261028T101847528569 exit0，19native/162schema观测/真实TMDB3.1.7 en-zh四响应/36JUnit通过；原完整verify20261028T101915217237最新尝试exit1，前5门通过，第6native fmt/check后Clippy cmp_owned失败，full native test未运行。保留原sessions4/失败5/gen1、WIP9c6f6ac及全部报告/失败证据，历史通过沿原证据保留，修改后重新全门。
保持与验收：开发先执行相关格式/构建/行为合法与非规范负例和Clippy验证，通过后正常交接；真实已知sandbox限制沿既有受控代验分工准确WIP，不重复权限探测或扩大权限。稳定退出后外层重新locked构建、准确A1、直接完整verify、原done/Git及固定提交独立CLEAR，后卡按严格顺序等待。4096MiB RSS采样/jobs2/线程1、项目锁、原权限与会话/命令时限/阶段/范围保持。当前protected=01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d、source=fce64ea699d9b01c623c052f455dfd93ca468d86fbf14f69fbe244db597b86c8保持，无冻结内容/控制器修改或rebaseline。


## ADR-027 2026-10-29T02:28:42.013419+00:00 — 已采纳：最小执行器异常清理修复及原双卡迁移

来源：当前聊天询问“仅修复platform_runtime.py、workflow.py及对应scripts/tests，补故障回归，并通过现有入口同步迁移T0.6与暂停T0.13基线后重新验收；原预算、权限、资源、暂停状态和全部历史保持”，用户明确回复“批准最小修复及迁移”。
决定：仅上述原控制文件和对应生命周期回归允许最小修订，不改权限/资源/时限/默认会话、迁移接口或业务要求。payload创建后尽早登记真实身份；首轮和清理枚举异常时仍回收所创建进程，清理边界未确认保留未完成状态并阻止新重任务；确认收尾后封存日志，不把枚举失败视为空进程或伪造历史摘要。补首枚举与清理枚举同时EPERM、身份登记/直属进程与子进程清理、cleanup_failed/恢复/重入及日志稳定性正负回归，复用既有执行器和状态。
实施路线：先隔离候选由现行原run受监管验证，再应用已批准文件，追加实际新protected的真实RESOLVED及迁移暂停卡T0.13，经原task.py rebaseline T0.6 --protected NEW --include-paused T0.13迁移，不扩展原入口、不手写状态。T0.6完成勾选已因原真实BLOCKING撤回，原done/成功A1/verify/受验提交/报告全部保留为历史，未完成集合正好T0.6/T0.13。原T06 sessions1/failure1/gen0及T13paused/s1/gen2保留，generation按原入口各增加一次；其他已完成卡保留各自历史基线及真实CLEAR。
复验：实际修订/迁移经原受控回归和固定独立文档复审CLEAR后，当前T06重新准确A1/完整verify/原done/Git/固定独立复巡。原功能通过事实保留，原P1 BLOCKING及旧日志/收据不改写，原未登记payload历史退出/RSS如实未知。严格顺序T07等待，普通预算不追加，4096MiB RSS采样/jobs2/线程1和原最小权限保持。当前批准处旧protected=01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d；实际新摘要将在候选验证应用后记录，当前无伪造新摘要或已迁移结论。


## ADR-028 2026-10-29T06:09:55.791867+00:00 — 已采纳：仅T0.9追加一次测试夹具与兼容观测修复会话

来源：当前聊天明确询问“T0.9已用完原定两次开发会话，是否仅为本卡追加一次2→3，由原CLI最小修复gzip/Range测试夹具、图片续期兼容断言和测试日志脱敏，再重新验收；现有功能、安全门、权限、资源、时限及全部历史保持，其他卡仍默认两次”，用户明确回复“批准 T0.9 追加一次”。
决定：仅T0.9累计开发会话上限2→3，唯一第三会话由原gpt-6.1-sol/workspace-write CLI修测试夹具和观测，保持所有产品/协议边界。有效gzip正例保持原解压字节，并补或保留损坏CRC/截断负例与失败诊断；Range相同分支作条件合并，保留200/206/416/If-Range/HEAD。重复详情在完整稳定内容/图片身份/path/cap相等之外，按SPEC明确验证新DTO引用时有效30分钟租约续期，不删除旧200集/分组/顺序/schema断言。现有测试观测对新增images.url查询cap及失败Debug安全脱敏，保留真实响应先验schema、合法非秘密占位和原始响应不变。不得抑制lint或改弱16原生行为/8观测/13需求、安全与资源门。
实施：独立只读文档round27对固定实际版本CLEAR后，紧邻核对current/严格最早/next=T0.9、原sessions2、4失败/gen0与旧身份退出/两锁空闲，唯一一次原scripts/drive.py --once --max-sessions 3。计数到3即本许可已使用；原workflow默认2/其他卡额度保持，不重置计数、追加时长、提升权限或改控制器。原WIPe760cc9/7709c5d、准确失败A1/Clippy/T07回归和全部收据/历史保持。
复验：原开发CLI实际相关自验证并读回结果；已知sysctl/sccache/Git限制沿ADR010稳定边界外层代验，不重复探测或绕过原执行器。源码稳定后外层原run locked构建/all-targets Clippy、相关新旧回归及准确T09 A1；A1成功直接完整verify.py all、原done/Git、固定提交独立CLEAR及干净树齐备才完成。T10严格等待，T13paused/session1/gen3保持。4096MiB RSS采样/jobs2/测试线程1、全部时限/指标/负载/原清理保持。protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b、source=305a1333ff06844af64028e2dae881038af457e27faebf6822c758b55dd21615；本次仅预算与范围内测试修复授权，冻结定义保持，不调用rebaseline。


## ADR-029 2026-10-29T07:49:17.390263+00:00 — 已采纳：仅T0.9追加一次测试模块接线与映射夹具修复会话

来源：当前聊天询问仅本卡追加一次3→4，原CLI修共用测试模块接线（host.rs仅cfg(test)声明）和N01 images映射误用数组夹具，再完整复验；其余权限、资源、安全门、预算和历史保持。用户明确回复“批准 T0.9 再追加一次”，随后指示“继续”。
决定：仅T0.9累计上限3→4，唯一第四会话完成共用observation_redaction测试模块在host.rs的cfg(test)入口一次声明及两fixture引用；按原ImageAccessMap以resource_id构造/遍历N01脱敏夹具，保留真实响应先schema、原响应不变、全部稳定字段及URL正负例。保持helper行为/原16原生8观测13需求/负载阈值与安全门，不抑制lint、复制实现或修改生产DTO/鉴权；host.rs原已在本卡范围，仅测试编译接线改变。
实施：固定实际文档独立round28 CLEAR后，紧邻current/next/严格最早=T0.9、原s3/f10/g0、旧全部身份退出/两锁空闲/clean/source/protected核对，唯一一次原drive --once --max-sessions 4。实际计数4即消费许可，其他卡default2及原权限/资源/全部时限保持，不重置历史、不延长时长。T13paused/s1/gen3保持，T10等待。
复验：沿ADR010原CLI实施与真实相关自验证；已知sandbox sysctl/sccache/Git限制已由原125确认，不重复探测或绕过执行器，准确受限WIP在稳定退出边界交外层原run构建/Clippy/共享脱敏和N01等相关回归/准确A1；A1成功后直接完整verify、原done/Git/固定独立CLEAR及干净树全部成立才完成。旧A1通过、失败完整verify/旧回归及WIP、10失败（含两外层误判）均保持历史，新源重新验收。当前source=ad2b5de36d1616ff00d2c4423366038dbbbc9ae976904c8713f28d88220d3e1e、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，冻结定义保持，此单卡预算批准不调用rebaseline。


## ADR-030 2026-10-29T09:57:10.277026+00:00 — 已采纳：仅T0.10追加一次真实隐式IV素材与校验修复会话

来源：当前聊天明确询问“独立巡检确认T0.10标称隐式IV素材实际含显式IV=0，真实隐式IV尚未验证，原两次会话已用完；是否仅追加一次2→3，由原CLI修正逐片序列号加密素材、前置断言和独立校验，再完整复验，完整24帧、音频、安全门、资源、权限及全部历史保持，T11等待”，用户明确回复“批准 T0.10 追加一次”。
决定：仅本卡累计开发上限2→3，唯一第三会话仅修tests内真实隐式IV素材生成及前置/独立oracle校验。按每个分片的非零媒体序列号大端128位IV独立AES128CBC加密，原清单不含IV属性；不只删除显式属性却复用固定IV密文。原本地HLS与原网关均须完整24视频帧逐帧hash等于原源、非零音频；显式/真实隐式/fMP4三模式、同cap/源上下文/全部子资源请求/序列及原8行为17观测3需求和全部安全/负载门保持，补素材前提及篡改反证，不改生产DTO/鉴权或降低断言。可复用已在用ffmpeg/openssl，不增加全局配置或控制器。
实施：固定实际文档独立round29 CLEAR后，紧邻严格最早/current/next=T0.10、原s2/f2/g0、旧身份退出/两锁空闲/clean/source/protected核对，唯一一次原drive --once --max-sessions 3。实际计数3即消费，仅本卡一次；其他卡default2、原权限/4096MiB RSS采样/jobs2/线程1/全部时限保持，T13paused/s1/gen3和所有历史不变。
复验：原CLI真实相关自验证读回，已确认sandbox sysctl/sccache/Git限制不重复探测或绕过原执行器；真实受限按ADR010稳定WIP退出边界交外层原run构建/Clippy/相关回归/准确A1，成功直接完整verify、原done/Git/新固定独立CLEAR/干净树齐备才完整完成，T11严格等待。原成功A1/完整verify/受验提交/独立CLEAR均保持历史，缺口修复后新源重新全验，不accept-review或覆盖历史。当前source=84a54c2242a250278993421cdcf8c4c4523d0340d516391304ded9aa33e7f2f1、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，冻结定义保持，仅本卡预算和范围内测试修复授权，不调用rebaseline。


## ADR-031 2026-11-03T01:11:55.184302+00:00 — 已采纳：仅T0.11追加一次ASS基础文本边界修复会话

来源：当前聊天询问“原两次开发会话已用完，是否仅追加一次2→3，由原CLI最小修复ASS注释处理和连续换行截断，补原生、独立校验及真实WKWebView文本/时间轴回归，再完整复验；原六轨、安全门、权限、资源、时限与全部历史保持，T12等待”，用户明确回复“批准 T0.11 追加一次”。
决定：仅本卡累计开发上限2→3，唯一第三会话最小修subtitle.rs的ASS覆盖块/注释处理及安全cue序列化，相关tests原生/独立oracle/真实打包WK完整文本和起止时间正负例。按ASS格式忽略覆盖块内注释，保留非注释正文/合法字面转义；连续换行不得变成WebVTT cue分界、丢后半文本或折叠为单换行。实际WebView读回完整文本与原时间范围，不能只比前缀、只测parser或把旧单换行六轨替代新边界。原8原生/13观测/两集六轨/开关切换seek/旧轨与Session清理/Resolution回收后加载/原源HTTP上下文/安全/大小/30s及负载资源门保持，新增边界计数准确记录，ASS高级特效原边界不变。
实施：固定实际文档独立round30 CLEAR后，紧邻严格最早/current/next=T0.11、原s2/f3/g0、旧身份退出/两锁空闲/clean/source/protected核对，唯一一次原drive --once --max-sessions 3。实际计数3即消费，仅本卡一次；其他卡default2，原workspace-write开发/read-only巡检、4096MiB RSS采样/jobs2/线程1/全部时限、T13paused/s1/gen3及全部历史保持。
复验：原CLI真实相关自验证读回，已确认sysctl/sccache/Git限制不重复探测或绕过原执行器；真实受限稳定WIP退出按ADR010交外层原run构建/类型/行为/真实WK准确A1，成功直接完整verify/原done/新受验提交/新固定独立CLEAR/干净树，齐备才完整完成，T12严格等待。真实GUI锁定时准确等待解锁，不放宽30s或跳过真实WK。原done/A1/verify/受验提交/CLEAR及缺口副本全部保持历史，不accept-review或改写收据。当前source=2941c9b105e342212b3fed693e5538491daca43ea34c43ed2ac2f83d883a57b0、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，冻结定义不变，本次仅单卡预算及原范围内修复授权，不调用rebaseline。


## ADR-032 2026-11-03T02:48:07.517238+00:00 — 已采纳：仅T0.12追加一次图片过期恢复定位与最小修复会话

来源：当前聊天明确询问“T0.12已用完两次开发会话，50次真实切集通过，图片过期恢复仍失败且原因待定位；是否仅追加一次2→3，由原CLI补真实图片请求与错误诊断、最小修复并完整复验，原权限、50次规模、安全门、资源、时限及历史保持，后卡等待”，用户明确回复“批准 T0.12 追加一次”。
决定：仅本卡累计开发上限2→3，唯一第三会话补原生img load/error、资源URL与身份、真实失效HTTP请求、Host状态查询计数和阶段诊断，区分浏览器缓存复用、DOM错误传递及生产身份回调。根因当前未确认，不因超时直接认定生产恢复函数错误。沿原4096-entry LRU确实触发已过期图片请求，验证正式页面原生img错误→一次只读状态查询→仅所属DTO一次刷新与重新decode；仅修实际确认原因及必要范围内生产回调，增加相关正负例，保持原40+非零行为计数与全部后续错误恢复门，不以mock/直接调用error回调替代真实原生图片链路。
实施：固定实际文档独立round31 CLEAR后，紧邻严格最早/current/next=T0.12、原s2/f4/g0、旧全部身份结束/两锁实际空闲/clean/source/protected核对，唯一一次原drive --once --max-sessions 3。实际计数3即消费许可，其他卡default2；原workspace-write开发/read-only巡检、4096MiB macOS RSS采样/jobs2/线程1/全部时限、T13paused/s1/gen3与所有历史保持，不重置或复用许可。
复验：原完整50次实际切集及每轮红蓝帧/440与880非零PCM/Session与URL绑定/无旧帧旧音频/单video轨道HLS监听/HTTP与会话资源清理、CPU/RSS/无单调资源增长门保持；原PlayerEvent/250ms/旧epoch与异步play/EOS/unknown duration/autoplay、同集换线Resolution、captured旧handles回收、video/img/track每加载一次3s只读status/request host revision epoch target、expired一次ResumeSnapshot/401重连/502上游/codec/查询失败旧查询及30s与命令时限均保持。首第二源通过50切集但仍失败的原A1、原97前端/8oracle及首所有失败/WIP/125未封存副本继续历史保存，新源全部重新验收。
分工：原CLI真实相关自验证读回；已确认sysctl/sccache/Git限制不重复探测或绕过原执行器，真实受限稳定WIP退出沿ADR010交外层原run相关构建/类型/行为/真实WK准确A1，成功直接完整verify/原done/受验提交/新固定独立CLEAR/干净树齐备才完整完成。GUI锁定只读等待解锁，严格后卡等待。当前source=1178265c43c21db0a639bb4dde16b6d12c278ed5b6c690528d6585f45e95cd3b、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，原卡范围src/player.ts、PlaybackView.vue、App.vue、api.ts、state.ts及tests内最小修复，不改范围外Rust/模型/依赖/冻结定义和全局配置；本次仅单卡预算与原范围内修复授权，不调用rebaseline。


## ADR-033 2026-11-03T03:59:43.666952+00:00 — 已采纳：仅T0.12再追加一次RSS资源持有定位与最小修复会话

来源：当前聊天明确询问“T12已用完第三次会话，新一轮50次切集画面音频通过，但十个窗口内存趋势门失败且原因待定位，图片及后续恢复未执行；是否仅再追加一次3→4，由原CLI补资源持有诊断、最小修复并完整复验，原50次规模、无持续资源增长要求、安全门、权限、资源及时限保持”，用户明确回复“批准 T0.12 再追加一次”。
决定：仅T0.12累计开发上限3→4，唯一第四会话补每个创建身份进程RSS、媒体/DOM/轨道/监听、Session/HTTP以及测试观测持有量的最小诊断，依据真实数据区分浏览器缓存暖机、实际未回收资源及验收自身持有；仅修证据确认原因，增加相关正负例，再完整复验。当前十个五切窗口RSS中位数严格递增为已确认失败，根因尚未确定，未据此宣称泄漏；原4096MiB预算未超。
强度：保留原完整50次实际切集与51份CPU/RSS样本、原十个五切窗口无单调资源增长门、每轮当前红蓝帧/440与880非零PCM/Session URL绑定/无旧音频旧帧/单video HLS轨道监听及HTTP会话回收；不删门、改阈值或单位/抽掉样本/人为warmup或强制GC/反复同源筛选通过来规避失败。指标、门或测法实质变化须另取真实决定；原权限、4096MiB RSS采样/jobs2/线程1、30秒和命令时限、全部安全门与历史保持。
实施：固定实际文档独立round32 CLEAR后，紧邻严格最早/current/next=T0.12、原s3/f6/g0、旧全部创建身份退出/两锁实际空闲/clean/source/protected核对，唯一一次原drive --once --max-sessions 4；实际计数4即消费本次许可，未授权第五会话，其他卡default2，不重置历史或复用许可。T13paused/s1/gen3及严格后卡等待保持。
复验：新源完整准确A1仍须真实50切集与资源门、nativeHLS/controls/expire、第三源新增4096 LRU图片原生410/trusted error→生产一次3秒只读status→所属DTO一次刷新/trusted load/decode及阶段/身份/计数、errors/autoplay/unknown/EOS/layout/正常stop-dispose/Host finish全部通过。原PlayerEvent/250ms/旧epoch异步play及EOS、同集换线Resolution/captured旧handle回收、video/img/track每加载一次status核request host revision epoch target、expired一次ResumeSnapshot/401/502/codec/查询失败旧查询门保持，不直接invoke error回调或mock替代真实原生链路，不改生产缓存语义。
分工：原gpt-6.1-sol/workspace-write CLI实施与真实相关自验证读回；已知sysctl/sccache/Git限制不重复探测或绕过原执行器，真实受限准确WIP稳定退出按ADR010交外层原run构建/类型/行为/准确A1，成功直接完整verify/原done/受验提交/新固定gpt-6.1-sol只读独立CLEAR/clean齐备才完整完成。GUI锁定只读等待解锁不改系统。当前source=0bf54dfbc4159459f1d83f3eaa44c9e7f133b759102739118fa8f0952edc4fbe、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b；仅原T12范围src/player.ts、PlaybackView.vue、App.vue、api.ts、state.ts与tests内最小诊断修复，不改范围外Rust/模型/依赖/冻结定义/控制脚本或全局配置，不调用rebaseline。原所有A1失败、50切集通过但后门未执行、RSS样本/原125未封存副本/WIP及累计6失败保持。


## ADR-034 2026-11-03T04:24:11.523101+00:00 — 已采纳：仅T0.12再追加一次逐模式错误分类定位与最小修复会话

来源：当前聊天明确询问“T12本轮50次切集、原内存趋势门和真实图片恢复链路均通过，随后错误分类验收失败，具体模式和实际类别尚需诊断；第四会话已用完，是否仅再追加一次4→5，由原CLI补逐模式证据、最小修复并完整复验，原规模、资源门、安全要求、权限及时限保持”，用户明确回复“批准 T0.12 再追加一次（4→5）”。
决定：仅本卡累计开发上限4→5，唯一第五会话补逐mode真实输入、媒体kind/path前提、当前Session/epoch、MediaError、状态查询request/response与最终error.code的逐阶段诊断，区分生产分类、异步旧状态或夹具媒体路径前提，再最小修证据确认原因并增加正负例。原错误报文未含具体mode与实际code，根因仍未确认，不据报错直接认定生产从浏览器message推HTTP。保持协议/C9的401重连、502上游、codec明确类别及业务error.code，不改生产鉴权/错误类别或仅改期望值迎合夹具，不从browser message猜HTTP码；媒体模式/请求目标前提须准确，mock不能替代原生video/img/track链路。
实施：固定实际文档独立round33 CLEAR后紧邻严格最早/current/next=T0.12、原s4/f7/g0、旧全部创建身份结束/两锁实得/clean/source/protected核对，唯一原drive --once --max-sessions 5；实际sessions5即消费本许可，未授权第六，其他卡default2，不重置历史或复用他卡许可，T13paused/s1/gen3及后卡严格等待保持。
强度与复验：原完整50真实切集/51份aggregate CPU RSS/十个五切窗口无单调资源增长门/每轮当前红蓝帧440与880非零PCM/Session URL绑定/无旧帧音频/单video HLS track监听 HTTP Session回收保持，不删弱门/改阈值单位/抽样本/人为warmup强制GC/同源筛选通过，指标门测法实质变化另需真实决定。nativeHLS/controls/expire、4096LRU真实410/trusted error→一次3秒只读status→所属DTO一次refresh/trusted load decode及URL DTO/请求Host/ref revision/epoch target/30s计数、errors含track/401重连与查询失败旧查询、autoplay/unknown/EOS/layout/正常stop-dispose/Host finish全须新源实测。PlayerEvent/250ms/旧epoch异步play EOS/同集换线Resolution/captured旧handle/expired一次ResumeSnapshot及所有原安全资源时限保持；原4096MiB macOS RSS采样/jobs2/线程1不变。
分工与历史：原gpt-6.1-sol/workspace-write CLI实施及真实相关自验读回，已知sysctl/sccache/Git限制不重复探测或绕过原执行器，真实受限准确WIP稳定退出沿ADR010交外层原run构建/类型/行为/准确A1，成功直接完整verify/原done/受验提交/新固定gpt-6.1-sol只读独立CLEAR/clean才完整完成，GUI锁定只读等解锁不改系统。当前source=587d111a9d64880a9515869fc931986e198b946dd1c8d4d3c0f4916b64e78468、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b；仅原T12前端及tests范围最小诊断修复，不改范围外Rust/模型/依赖/冻结定义/控制脚本/全局配置或生产缓存语义，不rebaseline。第四源本轮50/资源/图片诊断过但A1失败及未运行后门准确保存，不追认历史RSS根因；原42份SHA副本、旧41/18/35/39/125未封存副本/全部WIP与累计7失败保持，新源完整重新验收。


## ADR-035 2026-11-03T05:37:10.923201+00:00 — 已采纳：仅T0.12再追加一次EOS夹具动作修正与完整复验会话

来源：当前聊天明确询问“T12本轮错误分类与重连已通过，结束事件验收失败；视频已在播放，EOS测试仍点击播放暂停按钮会转为暂停，第五次会话已用完，是否仅再追加一次5→6，由原CLI最小修正这一测试动作、补回归并完整复验，生产EOS语义、真实结束事件和全部原门保持”，用户明确回复“批准 T0.12 再追加一次（5→6）”。
决定：仅本卡累计开发上限5→6，唯一第六会话仅tests内最小修EOS夹具已播放/已暂停两前提动作顺序，加正负例与失败时正式video/controller状态诊断。第五源EOS前真实观测playing/paused=false，夹具无条件gesture点击toggle与该前提冲突；EOS后正式视频状态未保存，修复后仍须实测确认，不宣称生产EOS缺陷已确定。不得修改生产EOS语义、直接派发synthetic ended、跳过结束事件或放宽原时限；必须核真实原生DOM ended与当前epoch/一次事件映射。
实施：固定实际文档独立round34 CLEAR后，紧邻严格最早/current/next=T0.12、原s5/f8/g0、旧全部创建身份结束/两锁实得/clean/source/protected核对，唯一原drive --once --max-sessions 6；实际sessions6即消费许可，未授权第七，其他卡default2，不重置历史或复用他卡，T13paused/s1/gen3与严格后卡等待保持。
强度：新源完整准确A1仍须50次真实切集/51份aggregate CPU RSS/十个五切窗口无单调资源增长门/原单位阈值，每轮当前红蓝帧440与880非零PCM/Session URL/无旧帧音频/单video HLS track监听 HTTP Session回收、nativeHLS controls expire/4096LRU图片原410 trustederror一次3sstatus所属DTO一次refresh trustedloaddecode及身份次数30s、完整errors/track/401重连/502上游/codec/查询失败旧查询、autoplay拒绝与unknown duration UI、EOS/layout/正常stop-dispose/Host finish全部门。保持PlayerEvent/250ms/旧epoch异步play EOS/同集换线Resolution/captured旧handles/expired一次ResumeSnapshot及全部安全权限资源时限；不删弱门/改阈值单位/抽样本/人为warmup强制GC或同源筛选通过，实质指标门测法变化另需真实决定。
分工与历史：原gpt-6.1-sol/workspace-write CLI仅实现被授权测试修复及真实相关自验读回；已知sysctl/sccache/Git限制不重复探测绕过，真实受限准确WIP稳定退出按ADR010交外层原run类型/相关正负例/真实WK准确A1，成功直接完整verify/原done/受验提交/新固定gpt-6.1-sol只读独立CLEAR/clean齐备才完整完成。GUI锁定只读等真实解锁不改系统，原4096MiB macOS RSS采样/jobs2/线程1/30秒及命令期限保持。当前source=10e446036a8615ed005b6f7c6a54f46c6591dac472b44d340a70f8005f828f38、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b；不改生产源码、鉴权类别缓存、范围外Rust/模型/依赖/冻结定义/控制脚本或全局配置，不rebaseline。第五源已有原生分类/图片/50资源等观测与独立校验过仍属失败A1，EOS后门/fullverify未过准确保持；原43份SHA、旧42/41/18/35/39/125未封存副本/全部WIP与累计8失败保留，新源全量重新验收。


## ADR-036 2026-11-03T05:55:59.835267+00:00 — 已采纳：仅T0.12再追加一次WebView资源持有诊断与最小修复会话

来源：当前聊天明确询问“T12本轮画面音频通过，原内存趋势门再次失败，尚未运行到EOS；增长集中WebView渲染与图形进程，页面轨道监听会话HTTP数量稳定但原因待定位，第六额度用完，是否仅再追加一次6→7，继续原范围资源诊断、最小修复并完整复验，原50次规模/RSS门/安全要求/权限时限保持”，用户明确回复“批准 T0.12 再追加一次（6→7）”。
决定：仅本卡累计开发上限6→7，唯一第七会话在原T12前端与tests范围内继续最小补renderer/GPU资源持有诊断，依据真实证据区分浏览器缓存/解码资源、实际未释放对象与验收自身持有，再最小修确认原因并增加正负例。已有每身份数据显示renderer增加44515328bytes/GPU增加20299776bytes、application下降5341184bytes且DOM71/track1/监听9/Session1/HTTP0/scratch24576bytes与1canvas等稳定；此不足确认缓存或泄漏，也不能代替原aggregate RSS门。
实施：固定实际文档独立round35 CLEAR后，紧邻严格最早/current/next=T0.12、原s6/f9/g0、旧全部创建身份结束/两锁实得/clean/source/protected核对，唯一原drive --once --max-sessions 7；实际sessions7即消费许可，未授权第八，其他卡default2，累计会话失败与全部历史不重置、不复用他卡许可，T13paused/s1/gen3及严格后卡等待保持。
强度：原完整50真实切集/51份aggregate CPU RSS样本/十个五切窗口无单调资源增长门/原单位阈值及每轮当前红蓝帧440与880非零PCM/Session URL/无旧帧音频/单video HLS track监听 HTTP Session回收保持；不删弱门、改阈值单位、抽样本、人为warmup或强制GC、同源反复筛选通过。指标门或测法实质变化须另取真实决定；原4096MiB macOS RSS采样/jobs2/线程1/30秒及命令期限、全部安全权限时限保持。
完整复验：第六源仅tests的EOS已播放直接seek/暂停先正式按钮resume再seek、原20秒总期限/真实trusted DOM ended当前epoch session一次及失败诊断保持，生产EOS语义不变；新源须完整准确A1覆盖原50/资源门、nativeHLS controls expire、4096LRU图片真实410/trustederror→一次3s只读status→所属DTO一次refresh/trustedload decode及URL DTO request Host ref revision epoch target/次数30s、完整errors track401502codec/query失败旧查询、autoplay unknown/EOS/layout/正常stop-dispose/Host finish。PlayerEvent250ms/旧epoch异步play EOS/同集换线Resolution/captured旧handles/expired一次ResumeSnapshot及全部原门保持，不synthetic ended或mock替代原生链路，不据旧源通过补新源。
分工与历史：原gpt-6.1-sol/workspace-write CLI实施与真实相关自验读回，已知sysctl/sccache/Git限制不重复探测绕过，真实受限准确WIP稳定退出沿ADR010交外层原run相关构建类型行为/真实WK准确A1，成功direct完整verify/原done/受验提交/新固定gpt-6.1-sol只读独立CLEAR/clean齐备才完整完成，GUI锁定只读等解锁不改系统。当前source=c2f88c950fce441fa4285441aed1c3fc745b49ad16948bada7c5f46d97325fb0、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b；仅原T12范围最小修，不改范围外Rust/模型/依赖/冻结定义/控制脚本/全局配置，不rebaseline。第六源真实50帧PCM通过但RSS失败、新EOS及后门未运行、finally-15/lingering=[]准确保存，原39份新SHA/旧43/42/41/18/35/39与125未封存副本/全部WIP累计9失败保持。


## ADR-037 2026-11-03T06:36:53.996798+00:00 — 已采纳：仅T0.12再追加一次查询完成后重复错误诊断保留修复会话

来源：当前聊天明确询问“T12独立巡检发现并已复现生产缺陷，查询完成后同资源重复错误覆盖已确认分类并隐藏重试；第七额度用完，是否仅追加一次7→8，原CLI最小保留诊断与重试动作、补回归和真实页面验证再完整复验，一次查询/安全/资源及原验收门保持，T13等待”，用户明确回复“批准 T0.12 再追加一次（7→8）”。
决定：仅本卡累计开发上限7→8，唯一第八会话最小修原state.ts同加载尝试/资源/epoch/session身份下已完成诊断及动作保留；查询完成后的重复原生错误不得用原MediaError覆盖已确认的网关分类与重试入口。保持每加载尝试最多一次3秒只读status，不因重复错误增加查询；同资源成功清错、其他资源独立、旧epoch与新load身份/资源目标/Host/source_revision/request校验保持，覆盖查询完成前后重复、错误资源、旧身份与新加载等正负例，补真实WK相关回归。
事实与历史：固定5f651e8的原独立review真实ended/exit0/CLEAR同时确认C9/SPEC10.5 P2；原run反例首次query后UPSTREAM_FAILED/actionretry，再次同资源MediaError4变MEDIA_UNSUPPORTED/action空，根因receive先fail、diagnose queried guard返回，已复现；真实WK该重复时序尚NOT RUN。原done/A1/directverify/受验提交/CLEAR保持原样，完成勾选已撤并留真实BLOCKED/新反例SHA；不accept-review或以旧成功覆盖缺口，新源须新受验提交及新固定独立CLEAR。
实施：固定实际文档独立round36 CLEAR后，紧邻严格最早/current/next=T0.12、原s7/f10/g0、旧全部创建身份结束/两锁实得/clean/source/protected核对，唯一原drive --once --max-sessions 8；实际sessions8即消费许可，未授权第九，其他卡default2，不重置历史或复用他卡，T13paused/s1/gen3与严格后卡等待保持。
强度：原完整50真实切集/51aggregate CPU RSS/十个五切窗口无单调增长门/原单位阈值/每轮当前红蓝帧440与880非零PCM/Session URL/无旧音频帧/单video HLS track监听 HTTP Session回收及nativeHLS controls expire、4096LRU原生图片410/trustederror一次3sstatus所属DTO一次refresh/trustedloaddecode的身份次数30s、完整errors401502codec/track/控制401重连/查询失败旧查询、autoplay unknown/真实当前epoch Session一次trusted DOM ended/布局/正常stop-dispose/Host finish全门新源完整复验。PlayerEvent250ms/旧epoch异步play EOS/同集换线Resolution/captured旧handles/expired一次ResumeSnapshot与所有安全权限资源时限保持；不删弱门改阈值单位/抽样本/人为warmup强制GC/同源筛选通过，实质指标门测法变化另需真实决定。
分工：原gpt-6.1-sol/workspace-write CLI仅原T12范围实施最小生产修复及tests回归与真实相关自验读回；已知sysctl/sccache/Git限制不重复探测绕过，真实受限准确WIP稳定退出按ADR010交外层原run构建类型行为/准确A1，成功direct完整verify/原done/新受验提交/新固定gpt-6.1-sol只读独立CLEAR/clean才完整完成，GUI锁定只读等真实解锁不改系统。当前source=83f9e7c1d6184e73aba11485402a33fa6ce77cec07108a1c735cfeecd26b0f1c、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，不改生产鉴权/协议错误类别/缓存语义、范围外Rust模型依赖/冻结定义/控制脚本全局配置，不rebaseline。原4096MiB macOS RSS采样/jobs2/线程1/30秒与全部时限保持；原12份gap SHA/50份完成归档/可选缺周期samples事实/旧39/43/42/41/18/35/39/125未封存副本及全部WIP累计10失败历史保留。


## ADR-038 2026-11-03T08:18:22.351766+00:00 — 已采纳：仅T0.13追加一次容量中断后的最小补齐与复验会话

来源：当前聊天明确询问“T13第2会话因模型服务容量不足中断，134项前端行为及原生类型/Clippy通过，但仍有4处类型错误、Rust格式、缺失独立校验文件及已复现UI验收分支错误；原两次额度用完，是否仅追加一次2→3，沿原模型权限补齐中断工作并完整复验，原范围/要求/资源时限保持”，用户明确回复“批准 T0.13 追加一次（2→3）”。
决定：仅本卡累计开发上限2→3，唯一第三会话沿原gpt-6.1-sol/workspace-write CLI最小修App.vue两处genre键索引、一处Close词典键及CatalogView模板app.value类型错误，修lib.rs格式，补tests/acceptance/test_t13.py独立校验正负例，纠正validate_ui主套件因包含history-refresh-reset-and-preferences误入firstframe专用分支的KeyError，并核对容量中断后原实现与验收完整性。原134行为、已通过native类型/Clippy及所有当前失败保持历史，新源全部重新验证；不以扩大类型/取消断言或降低oracle数、矩阵与真实UI门迎合缺口。
事实与实施：原CLI因Selected model is at capacity两次中断exit1，无正常最终交接；原s2/f4/gen3/pausedfalse，旧第1会话/rebaseline/segments/WIPb5f979e和隔离ead5af1及f5cd88e保持。固定实际文档独立round37 CLEAR后，紧邻strict最早/current/next=T0.13、旧全部创建身份结束/两锁实得/clean/source/protected/原s2核对，唯一原drive --once --max-sessions 3；实际sessions3即消费许可，未授权第四，其他卡default2，不重置失败或复用别卡许可，不自行切模型/扩大权限。
验收强度：保留原14冻结cases、单AppState与Route/PlaybackContext/Resolution/Session交叉绑定/metadata playback分离、host重启全失效guest仅CS/旧epoch与取消/返回快切、首页全量单栏分页按ref index合并/失败栏目保留/图片裁剪/局部状态独立；默认en中文/Settings成功提交与失败确认值/五色系×明暗×双语390/系统动态首帧/旧ui_theme兼容/lang title a11y/路由筛选选集和活动会话保持、唯一UiProvider/useOsTheme、焦点快捷键/最小窗口高DPI/真实WK与必要短时媒体及相关正负例全保持。独立oracle至少4项、完整主UI交互/20矩阵/4DPI及五色首帧等当前非零断言保持，分支修正应有主报告和首帧报告正例及缺/错矩阵历史帧等反例，不只用截图存在或mock代原生；对比度等沿既有appearance判定及SPEC原可读要求，不新增无来源产品硬门槛。
边界与完成门：History本卡仅会话预览/刷新重置/按片时间分组续播重播删除清空确认，SQLite T18/T20；公共lib桌面cfg为本卡结构检查，移动业务另授权，真实CS条件DASH N02/产品耐久及其他平台边界保持。原T12播放器/C9重复诊断保留/单窗口prototype与真实WK安全回归保持，不覆盖旧卡报告收据充新证据。新源相关构建类型行为及准确T13 A1成功后direct完整verify/原done/受验提交/新固定独立CLEAR/clean齐备才完整完成；后卡严格等待。
分工与基线：原CLI实际相关自验读回；已知sysctl/sccache/Git限制不重探绕过，真实受限准确WIP稳定退出按ADR010交外层原run代验和Git。原scope src/、lib.rs/main.rs、tests/PROGRESS，不改范围外Rust host/gateway、依赖清单/冻结模型契约/控制脚本全局配置，不rebaseline。当前source=ca41e6a49dc3edb08e1a77421fa41cdf64ed596645b1070b38ba19611b0a94a9、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，generation3与原4096MiB macOS RSS采样/jobs2/thread1/全部安全权限期限保持；GUI锁定只读等解锁不改系统，当前先补编译/验收前提。原29份当前SHA、容量stderr/原125早登记payload与未封存副本/原recover归档/全部WIP失败和16份preflight旧WIP证据保持。

## ADR-039 2026-11-03T09:11:24.169312+00:00 — 已采纳：仅T0.13追加一次真实播放与界面失败的诊断修复会话

来源：当前聊天明确询问“T13三次额度已用完，50次切集/音画/内存门通过，但过期会话恢复、返回筛选、界面对比度、删除确认检查失败，测试程序写报告后退出超时；部分原因待诊断，90份证据封存。是否仅追加一次3→4，原模型权限定位最小修复/补回归再完整复验，原范围全部门/资源时限保持”，用户明确回复“仅批准 T0.13 追加一次（3→4）”。
决定：仅本卡累计开发上限3→4，唯一第四沿原gpt-6.1-sol/workspace-write CLI，在原T13范围补正式App/controller失败诊断，区分生产接线与夹具前提，定位并最小修复已确认原因：过期会话恢复、返回筛选、文字可读性、历史确认role/焦点前提及原生UI测试程序正常退出。补相关正负例与必要短时真实WK验证，再新源完整准确A1与direct完整verify；不预认根因、不为通过夹具改弱生产语义或断言。
事实：原第三s3/f5/gen3/pausedfalse；12oracle/134行为/类型/格式/native check Clippy过，准确A1 exit1/106required104pass2fail/JUnit107pass13fail；50完整帧PCM/51aggregate十窗RSS原门/HLScontrols过，仅历史不代新源。过期失败现有diagnostics为generic video而非正式App/controller；UI11case8pass3fail仅stack缺message/数值selector，确认截图可见模态但未保存实际role；程序写report后存活至930秒，finally remaining=[]/原exitnull，原因待确认。后续图片/errors/autoplay unknown EOS/layout/正常stopdispose/Hostfinish、五色startup及fullverify NOT RUN，失败不改写。
实施与预算：固定授权文档独立round38 CLEAR原文保存提交后，紧邻strict最早/current/next=T0.13、原s3f5g3/旧全部创建身份退出/两锁实得/clean/source/protected核对，唯一原drive --once --max-sessions 4；实际sessions4即消费，未授权第五，其他卡default2，不重置失败/segments/rebaseline或复用别卡许可，不自行切模型扩大权限。原90份SHA、缺1可选周期samples而实际51逐命令样本齐、旧29份/容量stderr/125未封存及所有WIP历史保持；不rebaseline。
验收强度：保留原14冻结cases、单AppState/Api/SettingsQueue/UiProvider/useOsTheme/i18n/History、Route/PlaybackContext/Resolution/Session交叉身份、metadata/playback分离、host全失效guest仅CS/epoch取消旧响应旧EOS/返回快切、全量单栏分页按ref index合并/失败栏目与图片裁剪/局部状态独立。默认en中文/五色×系统明暗×双语390/系统动态首帧/旧ui_theme忽略/lang title a11y/设置提交失败确认值/活动会话筛选选集路由保留、模态焦点快捷键/最小窗口高DPI及20矩阵4DPI五色首帧和独立oracle非零正负例保持。对比度沿既有appearance与当前真实测量4.5判定；补测量selector/样本/消息诊断，不降低门或伪造矩阵。真实WK与必要短时媒体保持，不以mock/截图存在/静态枚举代原生。
播放与资源：保留完整50切集/51aggregate CPU RSS十个五切窗口原趋势门与单位阈值、每轮1080p当前红蓝帧440/880非零PCM/Session URL/单video轨道监听/旧410HTTP0及全部清理门；不抽样/人为warmup强制GC/同源筛选/改阈值测法。一次3s只读status身份及一次过期恢复/第二拒绝/手动retry、图片410原生一次所属DTO恢复、完整401502codec/track与完成后重复诊断/C9重连、autoplay unknown真实trusted EOS/布局/正常stopdispose Hostfinish均须新源真实通过。补失败正式route/context/session/resolution/epoch/资源身份次数和原生video/controller状态，保留原异常及清理状态；正常退出修复须证明实际退出和负例不吞错，不synthetic ended、跳门或放宽930s/其他期限。
范围与完成：原scope src/、src-tauri/src/lib.rs/main.rs、tests/PROGRESS，不改范围外Rust host/gateway/依赖清单/冻结模型契约/控制脚本全局配置。原T12生产与prototype/真实WK/CSP Origin单窗口保持。History仅会话预览刷新重置/按片分组续播重播删除清空确认，SQLite T18/T20；lib/cfg仅结构检查、移动另授权，真实CS条件DASH N02/产品耐久其他平台边界保持。原CLI实际相关自验读回，已知sysctl/sccache/Git限制不重探绕过，真实受限准确WIP稳定退出按ADR010外层原run代验Git。新源准确A1冻结{python}字面成功direct完整verify/原done/受验提交/新固定独立CLEAR/clean全部成立才完整完成，后卡等待。
当前基线：source=a2120f9d02c862fa5b02114c8cb2596e1d56d435d4e4a88241738004325b6c4b、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，generation3和原4096MiB macOS RSS采样/jobs2/thread1/session10800/idle2700/command7200/review1800与全部权限时限安全强度保持。GUI实际锁定只读等真实解锁不改系统。第四后需新实现须真实留证报告最小决定并暂停，无第五许可；纯验收Git继续。

## ADR-040 2026-11-03T10:29:51.027440+00:00 — 已采纳：仅T0.13追加一次历史完成与重播诊断修复会话

来源：当前聊天明确询问“T13本轮原生退出、返回筛选及完整20组界面对比度通过，历史页片尾后仍继续观看而非重播，原因需区分结束事件/保存时序/验收前提；第四额度用完，104份证据封存。是否仅追加一次4→5，沿原模型权限最小修复/补回归并完整复验，全部原门资源时限保持”，用户明确回复“仅批准 T0.13 追加一次（4→5）”。
决定：仅本卡累计开发上限4→5，唯一第五沿原gpt-6.1-sol/workspace-write CLI，在原T13范围补真实失败瞬间的可信原生ended/current epoch/session/controller正式状态、HistoryEntry及保存提交顺序诊断，确认生产历史完成/晚到非完成写/页面载入或夹具异步前提的真实原因，最小修历史完成→重播及相关正负例和必要真实WK。保留显式新重播的正常重新记录语义，验证旧epoch/session/旧写晚到保持无副作用；不预认根因，不以进度百分比推定完成、合成ended、跳过重播/删除/焦点确认或降低原断言获得通过。
当前事实：原s4/f6/gen3/pausedfalse，第四源13oracle/136行为/类型/fmt/native check Clippy过，UI诊断预检exit1/3项2pass1fail，主UI11case10pass1fail；自然main成功0/故意失败1探针及清理过，20组合五类contrast均满足原4.5。唯一主UI失败在browser.js247 Completed record has no replay action，原生video.ended后历史截图Episode3 00:29/00:30仍Continue watching；当时controller和HistoryEntry快照欠缺，原多条保存/updated_at替换仅候选原因。后续重播/单删清空焦点未到、五startup及第四源完整准确A1/fullverify/done/完成巡检均NOT RUN，全局T13-all仍第三源失败，不借旧结果代新源。
实施与预算：固定授权文档独立round39 CLEAR原始报告保存提交后，紧邻strict最早/current/next=T0.13、原s4f6g3/旧全部创建身份结束/两锁实得/clean source protected核对，唯一原drive --once --max-sessions 5；实际sessions5即消费，未授权第六，其他卡default2，不重置原失败/rebaseline/segments或复用别卡许可，不自行切模型扩大权限。原104份实际SHA、第四/第三/第二成功失败、90/29份与125未封存/容量stderr/旧WIP隔离preflight历史全部保持，不rebaseline；未确定的旧RSS根因不追认。
验收强度：保留原14冻结cases、单AppState/Api/SettingsQueue/UiProvider/useOsTheme/i18n/History、Route/PlaybackContext/Resolution/Session交叉负例、metadata/playback来源分离、host全失效guest仅CS/旧epoch取消旧响应旧EOS/返回快切、首页全量单栏分页ref index合并/failed_sections对应/图片裁剪/局部状态独立。默认en中文/Settings提交成功及失败确认值/五色系统明暗双语390/首帧及运行系统动态/旧ui_theme忽略/lang title a11y/活动会话与路由筛选选集保持、模态焦点快捷键上下文/最小窗口高DPI、真实20矩阵4DPI五色首帧/独立非零正负例全部保持，对比度沿既有4.5，不以截图存在/静态枚举或mock代真实页面。
播放与阶段：保留第四源新Resolution清旧stream句柄及同Resolution显式换线、正式过期失败诊断与一次3s只读status身份次数、一次恢复/第二次拒绝/手动retry；新源完整50切集/51aggregate CPU RSS十窗口原门与单位阈值/每轮1080p当前红蓝帧440880非零PCM/Session URL绑定/单video轨道监听/旧410HTTP0及全部回收，图片4096LRU410原生一次DTO恢复/完整errors完成后重复诊断401502codec track与控制401重连/auto unknown真实trusted EOS/布局正常stopdispose Hostfinish全部完整重跑。不抽样/warmup强制GC/同源筛选或改阈值测法。保留原生退出成功0和故意失败1探针/真实main返回/布尔报告/原930s/身份XPC清理，UI诊断预检不替代完整A1。History仍仅会话预览/按片分组续播重播删除清空确认及刷新重置，SQLite T18/T20；公共lib/cfg仅结构检查、移动另授权，真实CS条件DASH N02/产品耐久其他平台边界保持。
范围与完成门：原scope src/、src-tauri/src/lib.rs/main.rs、tests/PROGRESS，不改范围外Rust host/gateway/模型依赖/冻结契约或控制全局配置。保留T12播放器/C9已完成诊断保留/捕获旧handle清理、prototype/正式PlaybackView/真实WK及Origin CSP单窗口。原CLI实际相关自验读回；已知sysctl/sccache/Git限制不重探绕过，真实受限准确WIP稳定退出按ADR010外层原run代验Git。新源准确A1冻结{python}字面成功direct完整verify/原done/受验提交/新固定独立CLEAR/clean全齐才完整完成，后卡等待。第五后需新实现真实留证报告最小决定并暂停，无第六许可，纯验收Git继续。
当前基线：source=c739dea75e1ca6afeafd912b8cdd832454fbe403cbe651f15d2d7b5b803bb72b、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，generation3/原4096MiB macOS RSS采样/jobs2/thread1/session10800/idle2700/command7200/review1800和全部权限时限安全强度保持；GUI锁定只读等真实解锁，不改系统设置。

## ADR-041 2026-11-04T03:40:59.431021+00:00 — 已采纳：仅T0.13追加一次真实结束前提夹具修正会话

来源：当前聊天明确询问“T13新证据确认测试看到ended属性就返回历史页，此时可信结束事件尚未记录、控制器仍播放，历史保存为未完成；第五用完，107份证据封存。是否仅追加一次5→6，由原CLI仅最小修测试结束前提/补正负例完整复验，保留真实结束事件/原时限/全部门与生产EOS语义”，用户明确回复“仅批准 T0.13 追加一次（5→6）”。
决定：仅本卡累计开发上限5→6，唯一第六沿原gpt-6.1-sol/workspace-write CLI，仅tests内最小修历史EOS夹具前提与相关正负例/必要诊断，保留原当前video/epoch/session的真实trusted ended、正式控制器完成及对应HistoryEntry完成提交后再执行原导航/Play again/从0重播/删除清空焦点断言。原结束等待12秒预算保持共享，不串接新的12秒延长结束期限，其他原时限保持。不得用仅ended属性、合成事件、state-only、旧video/旧epoch/session或进度阈值满足前提；不得补造completed或更改生产EOS/历史语义迎合夹具。本次不授权生产源码修改。
真实事实：第五源仅诊断，生产未改；原s5/f8/gen3/pausedfalse、13oracle/136行为/类型格式/native check Clippy过，UI诊断预检exit1/3项2pass1fail，主UI11case10pass1fail。完整55条order1..55未截断，native-ended/delivered-ended为0；order50 video.endedtrue/currentTime30但正式epoch120/session仍playing/position29800，order51/52紧接导航仅写completedfalse，order54读回同false，order55 Continue watching。未观测到completedtrue写被覆盖；delivered仅事件后微任务观察，非controller回调标记。当前失败观测确认验收前提过早，生产真实ended处理没有新确认缺陷，修正效果仍须新源实际验证。
实施与预算：固定授权文档独立round40 CLEAR原文保存提交后，紧邻strict最早/current/next=T0.13、原s5f8g3/所有旧创建身份退出/两锁实得/clean source protected核对，唯一原drive --once --max-sessions 6；实际sessions6即消费，未授权第七，其他卡default2，不重置失败/rebaseline/segments或复用别卡许可，不切模型扩大权限。第五CLI受限125早登记payload/raw_exit tests未知/无报告事实与10份未封存副本保持，外层原recover归档及原失败保留。第六不得再次探测已知sysctl/sccache/Git限制或绕过原执行器。
验收与范围：此次仅tests/PROGRESS内最小夹具修复，原冻结14cases、原13独立oracle及行为正负例要求保持，新增回归覆盖真实当前结束/完成提交正例和错身份/旧epoch/session/合成state-only/仅ended属性等拒绝反例。保留完整20矩阵五色系统明暗双语390/4DPI/五首帧/4.5对比度/模态焦点/重播删除清空确认，成功0及故意失败1自然退出探针/真实main日志与身份清理，原930s和全部期限不放宽；UI诊断预检不代准确A1，不以截图存在/静态枚举/mock代原生。
完整复验：新源准确A1须完整50切集/51aggregate CPU RSS十窗口原门及单位阈值、每轮1080p当前红蓝帧440880非零PCM/Session URL/单video轨道监听/旧410HTTP0/全部资源回收、nativeHLS controls同Resolution换线/一次3s只读status身份查询/一次过期恢复第二拒绝与manualretry/4096LRU图片410原生一次所属DTO恢复/完整errors完成后重复诊断401502codec track与控制401重连/auto unknown真实trusted EOS/布局正常stopdispose Hostfinish全部执行，不抽样/warmup强制GC/同源筛选或改阈值测法。保留第四新Resolution清旧句柄与原C9修复；旧第三50/51等结果不能代新源。
完成门与阶段：原CLI实际相关自验读回，已知限制真实受限时准确WIP稳定退出按ADR010外层原run代验Git；新源准确A1冻结{python}字面成功direct完整verify/原done/受验提交/新固定独立CLEAR/clean齐备才完整完成、后卡等待。单AppState/Api/SettingsQueue/UiProvider/useOsTheme/i18n/会话History、所有路由来源epoch与首页合并契约、双语首帧偏好/错误反馈/活动会话保持、prototype/正式PlaybackView/真实WK/Origin CSP单窗口与旧handle清理保持。History会话预览/SQLite T18/T20、公共lib/cfg仅结构检查/移动业务另授权、真实CS条件DASH N02/产品耐久其他平台边界保持。原范围外Rust host/gateway/模型依赖/冻结契约/控制全局配置不改，不rebaseline。
当前基线与历史：source=d63da9cc42fc9e831636797d0bb2eea1d102864d6aadec574d5aa72136ef7358、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，generation3/4096MiB macOS RSS采样/jobs2/thread1/session10800/idle2700/command7200/review1800及全部安全权限时限保持，GUI锁定只读等真实解锁不改系统。107份当前SHA及causal-readback/受限10/第四104/第三90/第二29/125未封存/容量stderr/WIP隔离和16preflight全部保持。第六后若需实现，真实留证报告最小决定并暂停，无第七许可；纯验收Git继续。

## ADR-042 2026-11-04T05:52:05.944776+00:00 — 已采纳：仅T0.13追加一次原前端资源持有定位与最小修复会话

来源：当前聊天明确询问“T13历史重播/删除焦点/界面矩阵/五色首帧全过，唯一失败为50切集后的原内存趋势门；增长主要在WebView渲染进程，总预算未超，缓存或实际持有原因未定，167份证据封存。选择仅本卡6→7沿原前端范围用现有诊断定位最小修复、原门不变，或先只读复审测法阶段”，用户明确选择“仅批准 T0.13 追加一次（6→7），沿原前端范围用已有诊断定位最小修复，原门不变”。
决定：仅本卡累计开发上限6→7，唯一第七沿原gpt-6.1-sol/workspace-write CLI，在原T13前端src/、tests/及PROGRESS范围优先读取当前50/51逐身份RSS/生命周期及8份renderer/GPU vmmap原证据，定位确认真实相关资源或验收自身持有原因后最小修复及必要局部回归。未确认前不预认缓存/泄漏，不为通过而修改生产缓存或制造资源下降；本次不授权改RSS门、测法或适用阶段，不授权Rust/lib/main/host/gateway、依赖或控制配置修改。
当前事实：原s6/f9/gen3/pausedfalse；第六tests修正的真实trusted ended/正式完成/HistoryEntry提交693ms<12s、重播0/删除清空焦点、主UI11case/20矩阵4DPI五startup和自然退出正负探针均过；最新准确A1 exit1/128required127pass1fail/JUnit137pass5fail。完整50帧PCM/Session URL/单video轨道监听/旧410HTTP0过且51aggregate齐，但十个五切窗口RSS中位数297041920→331481088 bytes严格增长原门失败，owned首289914880末340115456；renderer+51937280/application-4980736/GPU+2408448/network+835584 bytes，treepeak2050703360未超4096MiB。对象/Session/HTTP计数稳定及vmmap footprint下降/波动均不代RSS或证明根因。原后续播放安全门未跑、完整verify/done/完成巡检NOT RUN，不借旧结果代新源。
实施与预算：固定授权文档独立round41 CLEAR原文保存提交后，紧邻strict最早/current/next=T0.13、原s6f9g3/所有旧创建身份结束/两锁实得/clean source protected核对，唯一原drive --once --max-sessions 7；实际sessions7即消费，未授权第八，其他卡default2，原失败/rebaseline/segments/WIP全部历史保持，不切模型扩大权限或复用别卡许可，不rebaseline。167份当前SHA/8原memory-map、旧107/受限10/104/90/29、125未封存/容量stderr/WIP隔离16preflight历史保持，不追认旧RSS根因。
原内存门与资源：保留完整50次切集、51份aggregate CPU/RSS样本原时点及十个五切窗口原无单调增长判定/原单位阈值、全部真实创建身份与WebView XPC纳入预算，每轮1080p当前红蓝帧440880非零PCM/Session URL/单video轨道监听/旧410HTTP0/所有资源回收。不得排除renderer等成员或以detail/footprint/DOM计数替代aggregate，不抽样删弱门/人为warmup/强制GC/同源筛选/改变负载或测法阈值来通过。真实资源修复须有因果证据、正负例和当前源实测；若实际需要门/测法/阶段或范围决定，准确留证另取真实授权。
必要测试限制：执行2026-11-04最新真人“只做必要测试”。先用已存诊断，新增/重跑测试需说明具体改动或未解失败、要区分的原因及现有证据不足，选现成最小入口并在原supervision记理由。直接相关局部检查失败则先定位，已知sysctl/sccache/Git限制不再探测/不绕过原执行器；避免全npm/类型/生产构建及Rust检查的惯例批量前置，避免完整UI预检再重复A1，不另造测试框架。稳定源码仅一次准确A1，成功direct完整verify；原冻结必需门/规模不削弱，复用仍有效结果，周期监控与文档记录不跑业务/128项控制全套测试。
验收强度：保留原14冻结cases及单AppState/Api/SettingsQueue/UiProvider/useOsTheme/i18n/History复用，Route/PlaybackContext/Resolution/Session交叉身份、metadata/playback分离、host全失效guest仅CS/旧epoch取消响应EOS/返回快切、首页全量单栏分页ref index合并/failed_sections对应/图片裁剪/局部状态独立。默认en中文/偏好成功提交及失败确认值/五色系统明暗双语390与首帧动态/旧ui_theme忽略/lang title a11y/活动会话路由筛选选集保持、4DPI/4.5/焦点快捷键/五startup和自然成功0故意失败1退出探针保持。当前可信EOS与HistoryEntry完成/重播从0/删除清空确认及焦点和原共享12s不回退，不以截图/静态枚举/mock代真实页面。
播放与阶段：新源准确A1须完整nativeHLS controls同Resolution换线/第四新Resolution清旧stream句柄、一次3s只读status身份查询/一次过期恢复第二拒绝manualretry、4096LRU图片410原生一次所属DTO恢复、完整errors完成后重复诊断401502codec track与控制401重连、autoplay unknown真实trusted EOS/布局正常stopdispose Hostfinish，原C9修复和捕获旧handle回收/prototype正式PlaybackView/真实WK/Origin CSP单窗口不回退。History会话预览/SQLite T18/T20、公共lib仅结构范围和移动业务另授权/真实CS条件DASH N02/产品耐久其他平台边界保持。
分工与完成门：原CLI实际相关自验读回，已知限制真实受限准确WIP稳定退出按ADR010外层原run代验Git/只读巡检，不重复开发侧权限失败。新源准确A1冻结{python}字面成功direct完整verify/原done/受验提交/新固定独立CLEAR/clean全齐才完整完成，后卡等待。第七后若需新实现，真实留证报告最小决定并暂停，无第八许可，纯验收Git继续。原范围外Rust/模型依赖/冻结契约/控制全局配置不改。
当前基线：source=198ec737482ec9a703d79bbd9c0429196af25d450de478e236a77b68dc580d72、protected=97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b，generation3/4096MiB macOS RSS采样/jobs2/thread1/session10800/idle2700/command7200/review1800及全部权限时限安全强度保持；GUI锁定只读等真实解锁，不改系统锁屏设置。

## ADR-043 2026-11-04T06:18:24+00:00 — 已采纳：T0.13第八次开发在当前Claude对话内进行

来源：用户中止外层sessions后要求“从T13的任务卡开始接着做”，在收到“第七后无第八许可、原门不变、本对话或原CLI二选一”的说明后明确回复“批准，在这里做”。
决定：仅T0.13追加一次开发，由当前Claude Code对话（claude-opus-5-5）直接执行，取代ADR-042“无第八许可”及“不切模型”对这一次的限制。未经drive.py启动，task-state的sessions保持实际值7，不手写状态、不rebaseline，失败历史全部保留。
范围与门：范围沿T0.13原卡。原14 cases、50次切集、51份aggregate样本、十个五切窗口判定、单位阈值和全部安全资源门不变；不做同源筛选重跑、人为warmup或强制GC。诊断单列，不代替A1。若只有修改内存门或测法才能通过，停止并再次询问用户。
完成门：新源一次准确A1成功后直接完整verify、原task.py done、受验完成提交、drive.py --review-only独立巡检CLEAR。

## ADR-044 2026-11-04T06:32:55+00:00 — 已采纳：按娱乐软件定位放宽性能与耐久验收

来源：用户原话“验收标准过于严格了 基本上都维持在300MB附近，没有出现内存泄露即可，这是一个娱乐软件，不是工业软件，无需过于严格的验收标准。另外先检查其他任务卡，无需要太过于严格的验收标准，有的话帮我改了”。
决定：取代SPEC §13及ADR-020中相应的强度，SPEC.md须与原始规格逐字节一致，故保留原文，以本ADR、ARCHITECTURE C14和TASKS为准。
- 内存：T12/T13共用判定及T22改为owned RSS五切窗口中位数首末增长≤64MiB且均≤512MiB，取代“十窗不得严格增长”。数值依据：同源实测增长约16至37MiB，首轮A1约33MiB。约1.3MiB/次以上的泄漏会被拒绝，更慢的泄漏不在此门覆盖内。
- T22(T0.26)：冷启动20次改5次；1.5秒窗口/100ms UI/桥接p95/冷启动60秒改为记录目标，未达记差距不阻塞；网关1080p30由30分钟改10分钟；20Mbps由10分钟改3分钟。
- T24(T0.28)：MP4/HLS各1800秒改各600秒；窗口开关20次改5次。
保留：安全、鉴权、凭据隔离、数据完整性、历史写入、功能、原包兼容、关键可靠性清零、独立验收及已完成卡的历史。M系列未进入阶段，本次未改。
基线：仅用户本次决定下修订TASKS、ARCHITECTURE，经原task.py rebaseline T0.13更新授权基线；T13原14 cases及其余门不变，旧A1失效，新源须准确A1、完整verify、done、完成提交和独立巡检。

## ADR-045 2026-11-05T02:08:22+00:00 — 已采纳：后续任务卡在当前对话内按序开发

来源：用户原话“按照任务顺序继续做，该项目交给你实现”。此前T0.14至T0.16亦由用户逐张指示在当前Claude对话内开发。
决定：T0.14起各卡沿严格顺序在当前Claude Code对话内开发，不经drive.py --once启动，task-state的sessions按实际保留；每卡仍须范围内修改、准确A1、完整verify、task.py done、受验完成提交和drive.py --review-only独立巡检。巡检确认的产品缺陷直接修复；证据类细项由实施者判断并如实记录。不accept-review、不伪造授权；M系列阶段门、设备凭据与真实环境前提仍须用户提供。

## ADR-046 2026-11-05T02:26:38+00:00 — 已采纳：先交付桌面端，移动端作为稳定上线后的扩展

来源：用户原话“调整顺序，先做桌面端，移动端作为稳定上线后的产品扩展”。
决定：M00至M05保持最后且受原阶段门约束，桌面版稳定上线前不启动。T0.19至T0.22（T02/T03/T04/N02）虽名含Android，属于桌面版运行原始Cloudstream插件的引擎，其是否调整顺序待用户另行决定，本ADR未改变。

## ADR-047 2026-11-05T02:46:28+00:00 — 已采纳：放弃Android兼容层方向，改用Rust重写播放源并交接Codex

来源：用户原话“能否将这几个android代码重写成rust 这样就不用做安卓兼容层了”，以及“先暂停，做交接文档，后续交给codex，我想使用rust重写。现在整理交接文档和交接提示词”。
决定：暂停T0.19及Android引擎路线，后续由Codex以Rust重写播放源。本ADR记录方向，尚未修订SPEC、ARCHITECTURE与TASKS，U02、U05等要求的具体取代方式、许可、内容版权与首版范围待用户确认后由新ADR固定。交接说明见docs/HANDOFF-RUST-REWRITE.md。


## ADR-048 2026-11-05T02:57:04+00:00 — 已采纳：个人使用的独立Rust JSON来源、任务与基线迁移

来源：当前聊天用户原话“许可选择依据站点公开行为独立实现，暂时仅供个人使用，首版先做可用的 SuperVod JSON 来源，其余按你的建议。批准修订文档和迁移基线，独立审查通过后按任务顺序开始开发，先核实 AES 用途。”时间以当前聊天日期2026-11-05计；本记录时间为文档操作UTC时间。

决定：
- 许可选择依据站点公开行为独立实现。固定插件源码仓库未见许可证，照源码翻译存在衍生作品风险；本次未取得源码翻译许可。已读源码仅作范围和风险识别，产品实现和fixture依据公开HTTP行为，记录独立实现依据。个人使用是当前范围，内容版权授权和公开发行许可没有被本决定授予；公开发行/新增站点另请用户决定。
- 首版仅可用SuperVod普通JSON来源。优质、量子、红牛、无尽、光速、极速、豪华已取得含播放字段JSON，仍待完整协议/媒体验证；淘片简略JSON待详情核实；卧龙非JSON暂排除；其余六包站点、XML及特殊播放器路径暂缓。HTTP200与协议/实际播放分别记录，逐个通过后才晋级支持。
- AES范围核实：固定提交25cdd2119be03f62ae6a5a2cf46b16f9484224a2的SuperVod全部15个kt文件中，aesDecrypt仅Utils.kt定义，无调用；BaseVodProvider直接输出登记媒体，VodExtractor encrypt1/2为转义/Base64；Haiwaikan/JiangNan注册注释。普通JSON首版无需新增插件AES解密，HLS标准AES-128仍保留现有网关和播放器验证。静态核实与在线行为分列，不声称完整播放已通过。
- U02以核准JSON来源的搜索/详情/选集/解析取代给定索引和全部provider；U05以独立Rust来源实际行为取代7包字节一致ART/DEX执行；U08改为Stremio元数据与Rust MacCMS播放。SPEC与replica-spec原文保持；TASKS覆盖表逐项注明R02/03/04/05/06/07/09/10/11/12/13/14/15/16/18/36/44/45/47/49/50/53/54/63等受影响义务。归档Android/原包义务在未完成卡由本ADR取代，已完成卡正文、失败历史、ADR/PROGRESS/BACKLOG既有内容保持。
- T0.19复用maccms.rs/source_dispatch.rs/resolution.rs/gateway.rs实现JSON；T0.20多源身份/协议完整性；T0.21原生请求上下文/取消/隔离；T0.22宿主来源管理整合。沿用maccms kind和配置，现有机制足够时不新增provider抽象。T0.18保持随后设置/SQLite；T0.23由原包更新改配置事务；T0.24至T0.29改核准Rust来源的集成、网关、性能、个人使用安装包与交付。
- M00至M05改为两平台本地Rust来源，移除Android双APK/Service和iOS配对远程Android引擎义务；真机/生命周期/网络/数据与签名验证保留。仍须桌面稳定上线后另行明确移动授权与设备/签名前提，本次开发授权止于桌面。
- 公开响应脱敏fixture为主要行为回归、真实公开JSON与媒体短播为补充；每个支持来源逐个核实，至少一个真实完整闭环；所有在线失败/NOT RUN如实记录。验证码/Cloudflare/访问控制即停止该来源并报告，证书校验保持，日志和报告无凭据或capability/签名媒体URL。安全、鉴权、数据完整性与功能门保持。
- 性能完全沿ADR-044：5次冷启动、响应目标只记录差距、10分钟网关播放、20Mbps3分钟、50次切集RSS五切窗口中位数首末增长≤64MiB且均≤512MiB、最终MP4/HLS各600秒及5次窗口开关；删除AVD测量对象，替换为Rust宿主，macOS arm64必需、其他平台NOT RUN沿ADR-005/006。
- 用户明确授权本轮文档修订和T0.19原流程rebaseline；原protected摘要034b3b6a7baaa956956d0e29fe6a3b3c859ff2671fdb85df8d5ee310ebc7d129供核对，RESOLVED的批准摘要由实际task.py status新protected填写。迁移保留sessions/失败历史，旧验收证据失效。固定新文档基线独立只读审查CLEAR后，按任务顺序开发；每卡准确A1、完整verify、task done、完成提交与原review-only独立巡检，不自行accept-review。预算和workflow.json控制器不变；ADR-045当前对话执行方式继续适用。


## ADR-049 2026-11-05T03:41:56.643841+00:00 — 已采纳：仅使用指定公共DNS并保留内网防护

来源：用户在本聊天要求“指定1.1.1.1  8.8.8.8这两个公共DNS，不使用本机dns”。
决定：按C6统一固定公共域名解析到1.1.1.1:53、8.8.8.8:53，复用现有hickory依赖及异步取消/总预算。无系统DNS/hosts/搜索域或失败回退；不新增UI或运行时配置。两服务器失败明确报错；SSRF公网地址全集检查、连接绑定/peer、重定向、TLS与凭据隔离保持。测试注入仅测试编译。普通UDP DNS有被网络劫持可能，响应仍按公网规则检查，禁止通过放行Fake-IP解决；DoH/显式代理另行评估。按用户本次明确需求修订C6/T019验收，原rebaseline后独立文档CLEAR再实现，旧失败保留。

## ADR-050 2026-11-05T05:40:53+00:00 — 已采纳：来源支持按多视频样本判定

来源：用户原话“不对，各种源支持的视频可能不一样，不能仅凭一个视频就判断不支持”。
决定：取代ADR-048与T0.19中“遇访问控制停止该候选”的粒度。每个来源抽样多个视频，单个视频或媒体域名失败只记入该样本；至少一个样本通过JSON协议与媒体验证即晋级支持，并记录样本通过数与失败域名；全部样本未通过记为未验证，不写成不支持。API被访问控制拒绝才停止该来源；媒体域名被拒只跳过该域名。不规避：不伪造请求头、不换接口、不重试同一被拒URL，验证码与Cloudflare页面照常停止并报告，安全门不变。打包窗口短播仍至少一个真实闭环。

## ADR-051 2026-11-05T05:43:28+00:00 — 已采纳：程序保证来源兼容，播放与内容属来源状态

来源：用户原话“你要做的是做兼容性，都兼容这几个视频源，至于是否能播放，有没有视频则是视频源的原因，不能判断为程序原因”。
决定：取代ADR-048及ADR-050中以媒体可用性决定支持的部分。T0.19对优质、量子、红牛、无尽、光速、极速、豪华、淘片、卧龙记录两层：兼容性指程序能解析并映射其公开响应，失败属程序缺陷，必须修复；来源可用性指接口可达、响应形态、视频与媒体可播，失败属来源状态，如实记录，不算程序失败。可访问的JSON被适配器拒绝判为程序缺陷并使A1失败。至少一个真实打包短播闭环继续要求，用于证明程序链路。其他六个插件站点、访问控制不规避与安全门按ADR-048、ADR-050不变；是否扩大到其他站点未由本决定改变。

## ADR-052 2026-11-05T08:40:00+00:00 — 已采纳：同UUID且同端点重加仍为同一来源

来源：T0.20巡检20261105T082027165112指出“删除重加旧引用失效”与固定身份公式冲突；用户选择“视为同一来源”。
决定：修订T0.20验收。source_id与source_revision只由配置UUID和端点决定，同UUID且同端点删除重加后新旧引用逐字节相同，属同一来源，旧引用继续有效，在途请求仍取消。改端点、禁用、删除、以新UUID重加（来源界面的做法）的旧引用保持失效。H向量、设置结构和其余门不变。


## ADR-053 2026-11-06T02:30:34.575732+00:00 — 已采纳：交接文档纳入T0.23重验范围

来源：当前聊天用户明确批准将交接文档加入T0.23范围并按原流程rebaseline，完成重新验收和独立巡检后继续T0.24。
决定：仅将docs/HANDOFF-CODEX-T0.24.md加入T0.23范围，原验收、产品源码和工作流控制器保持；按原task.py rebaseline迁移当前卡基线，保留失败历史和额度，重新准确A1、完整verify、done、完成提交与review-only独立巡检，CLEAR后进入T0.24。用户另已确认ElfHosted简体中文TMDB主扩展及Hugging Face AIOLists中文TMDB备份；两者普通无认证请求已验证，产品端到端仍归T0.24。


## ADR-054 2026-11-06T05:42:54.712873+00:00 — 已采纳：用户可管理的媒体磁盘缓存

来源：用户先要求加入媒体缓存、设置最大容量和清理，随后在当前聊天明确选择“批准，加入缓存并统一验收”，批准相关设置/IPC契约、T0.24范围和验收修订及原流程rebaseline。
决定：按ARCHITECTURE C9/C11增加网关鉴权后的有界磁盘缓存，默认256MiB、可调、0关闭；完整HLS媒体分片/初始化片段及MP4完整或原样Range响应，保持流式背压；LRU/原子发布/启动恢复/容量预留/清理世代屏障，待释放占用可见。缓存按HTTP可缓存性及新鲜期处理，清单/密钥/凭据/cap不落盘，身份和请求上下文以哈希隔离，不跨会话复活授权。新Settings v2可选字段和两项固定IPC在原主WebView权限内，旧21项保持，SPEC及replica-spec原文保留；仅修订相关ARCHITECTURE、T0.24范围（加入protocol及原契约检查脚本）和验收。原完整A1 argv/安全/资源门不变，缓存与播放图标统一重验至独立CLEAR。失败/额度历史保持，当前protected摘要经原status记录后rebaseline，文档独立只读审查CLEAR后实现。仅个人使用，不增加离线下载/导出/分享功能。


## ADR-055 2026-11-06T09:22:47.157870+00:00 — 已采纳：Video.js 10.3.0官方播放器与控制栏

来源：用户要求使用最新版Video.js组件替代自搭播放器，并在当前聊天明确批准修订播放器冻结契约、T0.24范围和验收及原rebaseline，指定Video.js 10.3.0官方播放器与控制栏，保留缓存、切集、字幕与历史续播，完成原验收和独立CLEAR。
决定：按C10迁移，Vue采用官方@videojs/html及必要官方媒体适配/默认皮肤，精确版本10.3.0及锁文件；原自搭主控制栏和直接Hls生命周期由官方实现取代，业务仅薄适配。当前官网稳定版及npm latest已核实10.3.0，官方Vue指南https://videojs.org/docs/framework/html/guides/vue，版本https://videojs.org/changelog/10.3.0。不启用CDN/分析/外部源，不放宽安全和资源预算。仅修改当前T0.24相关架构/范围/验收（加入package.json/package-lock.json/vite.config.ts），SPEC和历史卡正文保留。原rebaseline保留额度失败历史，独立文档CLEAR后实现；WIP、准确A1、完整verify、done、完成提交及review-only CLEAR全部保持。


## ADR-056 2026-11-06T12:07:18.041614+00:00 — 已采纳：一次额外受控恢复入口

来源：用户当前聊天批准修订工作流恢复入口，增加一次受控恢复机会，保留全部失败历史和原验收阈值，不自行修改状态文件；先原rebaseline及独立文档审查，继续T0.24至完整验收与独立CLEAR。
决定：新增task.py resume-hard <当前卡> --protected <摘要>，仅在当前卡已STUCK-HARD、现有max_sessions内、唯一额外恢复尚未消费、最新rebaseline批准条目含精确受控恢复会话迁移标记时可用。持原驱动/重任务锁，核对无存活驱动/开发/重命令，校验冻结及追加记录；原脚本原子写入一次消费事件并仅将会话1增至2，失败与hard_sessions、预算/资源/验收阈值保持。当前用户只批准T0.24本次使用；作者不得手改状态。独立文档CLEAR后消费，再通过run.py验证入口正负例，原A1、完整verify、done/完成提交与review-only CLEAR不变；失败再次硬停止后等待用户决定。独立文档审查可在恢复前只读执行，控制器运行测试在恢复后进行。

ADR-056执行路径补充：本次恢复由当前外层在会话2内直接继续原run.py诊断/准确A1、verify、done及完成提交，再以drive.py --review-only独立巡检；不启动新的drive --once开发CLI。原driver的历史硬停止及max_sessions检查保持，继续拒绝另起开发会话。


## ADR-057 2026-11-09T01:29:38+00:00 — 已采纳：桌面端 Apple TV 化改造卡插入 T0.27 之后

来源：用户原话“给我一个改造方案，将映象视频改造成类似于apple tv的样子，只改造桌面端，包括app页面，使用体感”“推荐需要按照视频分类进行区分，如电影，电视剧，年份，类型”“还需要加入更多的分类，丰富筛选功能，所有的都按照apple tv来做”“把这些更改整理成任务卡，放到T0.27之后，原来的任务卡顺延”。方案文档：https://claude.ai/artifact/AVuEx8V1sPasLH1dc6ausK 。
决定：在 T0.27 后新增 7 张桌面卡：T0.28/TV1 外壳主题焦点、T0.29/TV2 目录协议与 TMDB 筛选、T0.30/TV3 首页与分类页、T0.31/TV4 MacCMS 分类与同源播放、T0.32/TV5 详情转场与待播、T0.33/TV6 沉浸播放器、T0.34/TV7 搜索资料库与来源设置。原 T0.28（T24）顺延为 T0.35，原 T0.29（T25）顺延为 T0.36，内容和验收强度不变；T0.35 依赖 T0.33、T0.34，T1.1 依赖改为 T0.36。
映射：已完成卡正文、历史 ADR、PROGRESS、审查报告及已完成卡的验收脚本（如 tests/acceptance/t01.py）保留原文，其中的 T0.28/A1 指顺延后的 T0.35/A1，T0.29 指 T0.36，原卡号 T24、T25 及其证据路径不变。
契约：ARCHITECTURE 新增 C17，在宽度不小于 960px 的桌面布局上修订 C10、C11 和 ADR-055 的对应条款：横向内容排、固定深色、Video.js 官方元素组合的自定义皮肤。新增 IPC list_watchlist、add_watchlist、remove_watchlist、search_catalog，合计 27 项；主窗口新增 core:window:allow-start-dragging；SQLite 升至版本 2。CSP、CORS、TLS、媒体网关策略保持。伦理片、福利、擦边短剧默认不显示。
状态：本 ADR 只修订文档。TASKS.md 和 ARCHITECTURE.md 属冻结文件，继续开发前须由用户明确授权 rebaseline，并由独立只读审查按 PROMPT-DOC-REVIEW 给出新的 CLEAR；移动阶段门和其他卡的范围不变。

ADR-057补充 2026-11-09T01:43:38+00:00：用户要求“0.28-0.34的任务卡里需要详细规划好页面风格”。T0.28至T0.34各增加“页面风格规格”和对应验收用例，T0.28定义共用变量，开工校准后的最终值写入PROGRESS；T0.31、T0.32、T0.34的范围同步增加样式或组件目录。

ADR-057补充 2026-11-09T01:48:31.000000+00:00：用户选择“加入 4 个文件并重验”。T0.27 范围增加 src/data/defaultSources.json、src/data/sourcePresets.ts、src/main.ts、src/state.ts，以覆盖交付后已提交的默认来源功能（2f6bd94）；T0.27 验收命令和阈值不变，经 rebaseline 后完整重跑 A1、verify、done、完成提交和 review-only，失败历史与会话额度保留。升级配置缺 TMDB 的缺陷由 T0.29 修复。

ADR-057补充 2026-11-09T02:03:01+00:00：按独立审查 round-48 修订 T0.28 至 T0.34 与 C17：契约检查和 t05 进入范围且只放行列名字段；真实窗口验收宽度改为 960、1200、1512，宽屏断点改为 1440，1920 记为 NOT RUN（本机显示器逻辑宽度 1512）；筛选增加排序映射与地区可用性声明；首页和分类页增加按类型、按年份内容排；MediaRow 增加真实页面验收；权限集合和 CSP 增加精确断言；MacCMS 同源放宽补 navigate 和历史海报。


## ADR-058 2026-11-09T02:18:20+00:00 — 已采纳：干净系统生命周期验收移至 T0.36

来源：用户原话“0.27重验不走UTM虚拟机测试，简化一下0.27验收标准，真机或虚拟机测试在0.36做”。
决定：T0.27/A1 简化为当前源码安装包检查（构建、依赖许可、产物内容、ZIP/DMG 复验、签名状态），使用 `t23.py --suite package`，覆盖 U04、R33、R45、REQ-T23，不启动窗口、虚拟机或其他机器。干净系统安装、初始化、首播、升级、卸载及用户声音确认作为 T0.36/A2，使用 `t23.py --suite lifecycle`，覆盖 U04、R30、R50、REQ-T25-LIFECYCLE，可在干净真机或专用虚拟机上执行，原有的干净系统、默认安全设置、quarantine 与观察绑定门槛不变；T0.36 范围增加 packaging/、src-tauri/、src/ 以修复发现的缺陷。T0.35 的真实运行证据核对不含这部分。
影响：T0.27 完成不再证明安装生命周期，T0.27 至 T0.35 期间该项为 NOT RUN。ADR-057 的 T0.27 重验按本决定执行；失败历史、会话额度和其他验收强度不变。
