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
