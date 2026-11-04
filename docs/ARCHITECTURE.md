# 映象视频架构与契约

状态：文档规划基线。业务实现验收均为NOT RUN。需求权威为逐字节归档的SPEC 1.5.0及replica-spec的补充机器契约。原规格中的application/前缀映射到本项目根，docs/progress.md映射到docs/PROGRESS.md（ADR-001）。

## 现有能力与最小修改路径

已阅读现有App.vue、UiProvider、theme、i18n、HistoryView、SourcesView、SettingsView及测试入口。保留Vue 3.5.44、Naive UI 2.45.3、TypeScript 5.9.3、Vite 8.3.5与锁文件，版本事实来自package.json/package-lock.json。复用FilmCard、Icon、样式、双语词典、主题及播放事件。demo.ts与会话内history.ts在相应接线卡替换入口。当前无Rust宿主、Android引擎和真实来源调用。

最小方案沿SPEC §3/§4：一个前端项目、一个Rust app crate；api.ts唯一invoke入口，SourceDispatch按enum分发，matching.rs匹配两条身份；仅Cloudstream启用Android原运行时。新增模块按各卡实际需要创建。细节与未知版本由T00/M00根据固定源码和实际构建确认。

## 必须保持的契约 [C]

### C1 项目与原脚本绑定（用户路径确认；moc-init运行约束）

唯一根目录`/Users/eason/jp/application`；工作目录、Git、原scripts、`.venv/bin/python`、`evidence/`和`evidence/drive/driver.lock`均绑定该根（已核对scripts/workflow.py的实际锁路径）。初始化宿主Python为`/Users/eason/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3`，3.12.14；此后控制脚本使用`/Users/eason/jp/application/.venv/bin/python`。迁移至另一系统时重新核实根/venv/进程与授权，使用该系统实际venv解释器。

按ADR-013及当前聊天用户授权，本项目heavy_lock使用绝对路径`/Users/eason/jp/application/evidence/drive/heavy.lock`，仅串行本项目的重任务，独立于其他项目的共享锁排队。其他项目原共享锁保持。迁移系统或项目路径时重新绑定此绝对路径并按原冻结流程审查。卡号映射只在TASKS内记录；沿用原task/run/verify/drive状态、收据与完成提交判定。实际锁文件路径由T00读取原runtime核对。首次开发前确认无存活驱动/CLI/重命令/巡检，核对PID＋创建时间。

### C2 需求基线与阶段（SPEC §1/§14/§16；ADR-002）

完整保留U01..U09、G0-Core/G0-CS/G1..G5、后续MG0..MG3。原29张桌面卡全部验收后交付桌面；后续6张移动卡独立授权。SPEC列出的搜索UI、收藏、账户/支付/广告/P2P/离线等维持原范围决定。原README中的开发AI启动文字作为资料。开发前须当前文档独立CLEAR、两条独立CLI、明确预算/权限与用户开发指令；建立真实heartbeat后才启动drive --once。每卡结束独立巡检；T25完成停止桌面监管，报告后续M列表。

### C3 线格式与角色（SPEC §7/§10.2/§18.1；DATA_STRUCTURES §2/§9/§13）

bridge.openapi是Android HTTP DTO/路由权威；app.schema是17个invoke、事件、Settings和状态权威，snake_case统一。invoke(command,{request_id,input})；结果data/error互斥。Rust业务包络engine_epoch为host_epoch，客体epoch单独保存；ready必须UUID及health ready。固定命令和主WebView权限，拒绝任意进程/文件写入入口。空值/单位/UTF-8预算/有限数值按字典§6及schema。项目内protocol/已导入两份实际机器契约和原样例；T05据此生成/校验DTO。T18按C16既定设计补历史IPC/正负例后写宿主，保持既有17个命令兼容。偏离受审定义时按冻结修订与独立重审流程处理。

### C4 原始插件（SPEC §2/§6；字典§5/§8）

索引commit d411c9597e16f1e12f51f563ef75cd8d00ace002、索引sha256 a514c083b372a2016a2a87ba0425d559caa8432372176ec78cdce346350643ef，7包逐字节符合清单。全部真实MainAPI注册实例含SuperVod全集均枚举并测试声明能力。Cloudstream候选commit a7ac454af8fbd64574c96847a936905855911720待T02实测，成功才成为生产锁。原Context/Application、PathClassLoader、Cookie、extractor、WebView、getVideoInterceptor保留；opaque字符串按原字节语义回传。

### C5 身份、跨源匹配（SPEC §7.2/§18.5/§18.6；字典§5/§11）

统一H使用tag+NUL与UTF-8 uint32长度前缀、SHA256小写hex；所有原公式直接复用归档向量。Stremio为产品元数据，Cloudstream主播放、MacCMS可选；独立来源偏好。content_id=H(metadata,namespace,type,canonical_id)，config_id/addon_key准确回源；已验证IMDb/TMDB才归一，自定义命名空间按addon_key隔离。语言保持稳定身份。

match_playback每次一个选中provider，最多2次查询×10候选，最多20项；全部正常且零候选为not_found；失败/超时为ApiError。仅同一已验证外部ID、类型且唯一季集可自动确认，其余needs_confirmation。人工selection重新核验已登记内容/组/集/修订。无search时按原能力显示手动home/details入口。match_id最多256、空闲5分钟，绑定host/config/video/provider引用；resolve重新核验。季0、小数集、别名/重拍、乱序、旧修订与过期均有负例。Stremio媒体resolve拒绝；下一集按metadata video顺序重新匹配。

### C6 HTTP与网络（SPEC §12/§12.1/§18.2/§18.3）

MacCMS匿名JSON、首个$拆条目而保留URL余串，组/集原顺序与歧义失败；类型探测有界。Stremio按完整manifest快照发布，首次失败unpublished且能力false，刷新失败保留旧快照；资源过滤/required extras/skip/同名栏目/配置路径query保持，详情按原扩展与顺序回退，warnings最多64。stream/inline媒体字段按元数据角色忽略。

配置最多32、每Stremio配置1..16 manifests，UUID唯一，禁用/删除/重加的generation墓碑与sync_sequence防止旧提交。公共HTTP入口覆盖API/manifest/图像/媒体/字幕/HLS/key/探测：全部DNS A/AAAA与实际peer公网单播，IPv4映射IPv6还原，地址绑定连接，保持Host/TLS/SNI，逐跳最多5次且拒绝HTTPS降级，跨Origin清除敏感头，禁用环境代理。受控本机fixture只在测试构建固定允许列表中开放。

### C7 请求预算与取消（SPEC §6.2/§7.3/§11.2）

下表原样保留操作预算，排队、重试、重定向与全部子请求共享单调deadline；guest接收剩余毫秒。聚合提前1000ms取消收尾，前端传输余量2000ms后主动取消，公共元数据4槽，provider sequential串行。连接上限10000ms并裁剪剩余，流无字节空闲30000ms。UUID重复/自身取消BAD_REQUEST，取消独立控制路径，父子级联，finally清理，返回前核对取消与世代。

| 操作 | 总预算ms | 单子请求上限ms | 聚合并发上限 |
|---|---:|---:|---:|
| 默认元数据/详情/搜索/HTTP单栏请求 | 30000 | 30000 | 1 |
| Cloudstream首页第1页 | 60000 | 15000 | 1，保持provider顺序 |
| Cloudstream单栏请求（含首屏重试） | 30000 | 15000 | 1 |
| MacCMS首页第1页 | 45000 | 30000 | 4 |
| Stremio首页第1页 | 45000 | 30000 | 4 |
| HTTP来源同步（含全部manifest） | 45000 | 15000 | 4 |
| resolve（MacCMS/Cloudstream） | 45000 | 45000 | 1，Cloudstream保持原回调行为 |
| match_playback | 45000 | 15000 | 4 |
| health / get_resource_status | 3000 | 3000 | 1 |
| runtime_prepare | 120000 | 120000 | 1 |

### C8 Session所有权与资源代理（SPEC §5.4/§9；字典§10）

256位随机控制Bearer和独立cap，bootstrap通过adb shell stdin，清除Intent extras，内存保管并脱敏。网关仅loopback，资源GET/HEAD限定登记ID；固定Origin CORS/必要OPTIONS与CSP。Rust与Android共用Range200/206/416、If-Range、压缩和重定向fixtures；插件原interceptor先执行，重写清单后重算头。

Session独立强持有LinkRecord、Cookie/interceptor与字幕/HLS上下文；Resolution释放/5分钟到期后已建Session继续。active_requests钉住、空闲5分钟释放；同集换线保留Resolution，切集/源/离页按捕获旧ID依次stop、close、release；旧清理幂等且保持新句柄。媒体初始64KiB流缓冲；清单≤8MiB，请求≤1MiB、响应≤8MiB、opaque UTF-8≤524288字节。图像/字幕大小及像素上限需T09按实际解码器明确并固化测试，保持RESOURCE_LIMIT。图片同epoch去重、30分钟滑动续租、默认4096 LRU。

HLS主/子、key、map、音轨/字幕、相对地址、签名、byterange、重定向基址、序列/不连续与未实现tag保留，全部派生资源同会话cap。基线实际出现清晰DASH时纳入BaseURL/List/Base/SIDX及受约束Template；字幕VTT/SRT/ASS基础文本、时间轴、鉴权及回收；DRM明确MEDIA_UNSUPPORTED。

### C9 资源诊断与恢复（SPEC §10.5）

get_resource_status只读登记状态、3秒预算，ready/expired/failed；最近请求序号控制错误归并，同资源成功清旧错，其他资源成功保持原失败。未知ID同expired；查询不访问上游及不续租。前端一次加载尝试最多一次查询，验证request/epoch/revision/target；图片所属DTO最多刷新一次，播放过期resolve/open/ResumeSnapshot恢复一次。业务错误按error.code；保留原MediaError及查询失败诊断。

### C10 状态与界面（SPEC §8/§10.3/§11.1）

单AppState使用现有Vue页面与Naive唯一UiProvider。host重启全部失效，guest重启仅Cloudstream。首页按全量/单栏重试/分页范围合并；index保留，空/错及部分失败分开；请求旧结果/旧EOS无副作用。前端原演示数据在真实接线验收时由invoke替换。

默认1200×800、最小960×640、360/390/768 CSS宽度、高DPI；首页4..7卡/行、海报约150..180与2:3方向，缺图/长文/200集可读。内嵌video/hls.js实际事件驱动、250ms合并位置、闲置3秒隐藏控制、焦点恢复、autoplay提示、字幕/音量百分比/seek/倍速/全屏/键盘，EOS按选项与当前epoch下一集。界面及匹配确认按原卡全部错误/取消用例验收。

### C11 设置、双语与外观（SPEC §8.1/§11.3/§18.3.1）

Settings v2原子JSON与v1迁移，单前端写队列+同宿主提交锁；未知提交暂停、取消/收尾后同锁重读，失败保持确认值。保存成功先发布设置/注册表及世代后释放锁；配置探测锁外。退出排空队列，损坏保留诊断副本。

默认en与lime，en/zh-CN文案与Naive locale、文档lang/title/无障碍名称同步；中文变体→zh-CN，其余→en。Stremio扩展语言方式在N01实证，英文基础ID与有效同ID中文覆盖，缺失/失败逐字段回退并记录原因；原预算共享、分语言缓存、语言/页面世代抵御中英中乱序，播放会话保留。系统明暗useOsTheme，五色系×明暗分别验证，首帧恢复色系，旧ui_theme兼容读取并忽略显示效果，变更保留路由/播放/筛选，存储失败反馈。

### C12 历史与数据完整性（SPEC §11.4）

Rust应用数据目录yingxiang-video.db，PRAGMA user_version独立迁移，事务失败保留数据，参数化SQL。watch_history完整列及索引按SPEC逐项落实；长期仅保存MetadataIdentity/video/季集/source与展示快照，临时URL/cap/Session/Cookie/令牌留在原生命周期。

按影片合并最近一集/今天昨天更早；约5秒为成功调度写入目标，暂停/seek/切集/离页/退出/后台提交捕获快照；世代与单调写序号防乱序/时钟回拨。删除/清空确认及写队列共用串行入口，旧快照作废，写错保留可重试快照并显示未保存。续播先定位原config/addon→当前详情/video→重新匹配播放源→新会话，loadedmetadata后恢复且裁剪新时长；EOS仅当前集完成，重播0。T18/T20 macOS arm64真实SQLite跨重启与失败证据。

### C13 Cloudstream生命周期与更新（SPEC §5/§6.1）

系统启动→安装T02 APK→固定BootstrapActivity→forward→鉴权health。RuntimeManager只操作自身AVD/serial/forward；空格/中文路径、取消、超时、崩溃、退出清理且保留用户其他设备。HTTP在Android缺失/崩溃时继续可用。INTERACTION_REQUIRED按真实fixture信号，用户明确打开交互后停止当前Cloudstream媒体、同AVD保留userdata重启为windowed，新guest令牌/epoch仅失效该分支，保持host及HTTP；同步Cookie，固定站点/Activity，用户完成后以新请求重试。

原包刷新暂存先校验，失败/无变更保持播放；提交前停止受影响包全部实例会话并撤销资源、阻止新业务，取消旧任务后发布revision；其他包/HTTP保持。失败回滚旧注册表但已停会话保持停，回滚失败包failed。清理命令在更新期间仍幂等可用。ZIP路径/解压/未知apiVersion及资源限制全部按负例验证。

### C14 平台、性能与资源（SPEC §5.1/§13；moc-init运行约束）

必需目标为本机macOS14+ arm64（ADR-005，macOS first）；构建与真实运行分别记录。原规格的macOS x86_64、Windows11 x86_64、Ubuntu24.04 X11/Wayland降为非阻断记录项：代码保持cfg可移植，报告写NOT RUN与复现命令，后续另行授权；移动机器在M00确认。公开最低配置在发行前实测；16GB内存/12GB磁盘、AVD 2核3072MB保持资源预算起点。缓存128MiB/256MiB保持初始观察预算。

工作流当前模板值memory_mib=4096、jobs=2、command_seconds=7200、session_seconds=10800、max_sessions=2；这是保留的控制器默认配置，开发启动前核实用户预算及每卡完整长命令覆盖。宿主macOS使用RSS采样，Linux原cgroup；线程1。AVD/JVM超出可用预算时先报告所需预算与完整命令，按授权调整配置并重审后运行。

真实集群测试服务成员Docker部署，逐容器硬内存上限；容器硬上限合计+宿主进程树采样RSS纳入批准总预算，运行时同时记录actual usage，成功/失败/取消仅清理带本次身份标签的自产容器/网络/卷。桌面应用与Android AVD依SPEC在目标系统运行；普通单进程fixture/真实集群部署类型在测试报告明确。当前未批准具体集群拓扑/容器预算；涉及该场景时须先明确额度并满足Docker契约，T21/M04对应验收拒绝漏项。

以下按ADR-044放宽SPEC §13：娱乐软件以无泄漏、无关键可靠性问题为门，响应类数值为记录目标，未达记差距不阻塞。T22在macOS arm64记录原始样本、测量起止、样本数/p95、CPU/GPU/OS/SDK镜像、依赖版本、分辨率/帧率/码率/RSS/AVD内存。参考机定义及本地桥接样本数在T00/T22测量方案实证后锁定；缺失时标为待验证，禁止把本机校准写成产品硬门槛。目标未达需明确差距与复现；关键可靠性问题清零后再进入发行。

| 项目 | 目标与测量条件 |
|---|---|
| 壳首次可见 | 参考机启动后1.5秒内呈现窗口与状态 |
| UI动作反馈 | 常见点击100ms内显示状态变化 |
| 本地桥接纯处理 | 无外网fixture的元数据请求p95低于100ms |
| 引擎冷启动 | 已安装环境参考机目标60秒内健康，记录5次样本 |
| 播放 | 自有1080p30/H264+AAC样本连续10分钟，音画正常、可seek |
| 本地代理吞吐 | 20Mbps受控流，连续3分钟且保持流式内存上界 |
| 连续切集 | 50次切换后无旧音频、旧帧、悬挂会话或内存泄漏（owned RSS五切窗口中位数首末增长≤64MiB且均≤512MiB） |
| macOS UI | 最小窗口、125%/150%/200%DPI（或macOS等效缩放）与常见全屏状态可用 |

### C15 移动复用与发行（SPEC §10.4/§14/§16）

桌面lib.rs移动公共入口/cfg隔离；移动后续复用DTO/状态/页面/Rust HTTP、匹配及SQLite。Android同机双APK同签名、显式signature Service、原Context；iOS配对受信HTTPS远程原DEX引擎，TLS/撤销/多客户端隔离/断线恢复。360/390/768、44px触控目标、safe area/playsinline/系统返回、旋转、音频中断、前后台暂停及进程回收真机验收。最低OS/工具链M00实测，MG0..3逐项证据。

macOS arm64干净安装、初始化、升级/卸载、签名公证、SDK获取、GPL/原插件许可与源码对应、离线打包资源均按T23/T24；WebView2/WebKitGTK/GStreamer等其他平台项列NOT RUN。缺Apple签名凭据时产出未签名包并列为发行待办；真实源失败在同原包参考环境复核。

### C16 历史IPC的最小实现契约（SPEC §11.4/§18.6的规划细化）

此处固定T18的既定交付字段，当前实际protocol/保留原17命令；T18追加以下四个命令及schema/正负例，再实现Rust/SQLite，合计21命令。Android HTTP桥接保持原样。所有命令沿用invoke(command,{request_id,input})与结果data/error互斥、host_epoch UUID及既有ApiError。

| 命令 | input | 成功data |
| --- | --- | --- |
| list_history | {} | {entries: WatchHistoryRecord[], write_sequence: 非负安全整数}，按updated_at DESC/content_id排序；write_sequence为当前宿主已提交序号 |
| save_history | {record: WatchHistoryRecord, session_id: 非空字符串（复用PlaybackViewSession.session_id）, match_id: 长度1..160不透明字符串, playback_epoch: 非负安全整数, write_sequence: 正安全整数} | 已成功提交的WatchHistoryRecord |
| remove_history | {content_id: 64位小写hex, write_sequence: 正安全整数} | Ack {ok:true}，目标已不存在时同样成功 |
| clear_history | {write_sequence: 正安全整数} | Ack {ok:true}，已空时同样成功 |

输入对象关闭额外字段。match_id原样复用PlaybackContext.match_id格式，允许合法非UUID值；session_id复用现有PlaybackViewSession字段的准确格式，不另限UUID或长度。前端唯一历史写队列给快照及删除/清空动作分配递增write_sequence，序号在捕获/入队时固定，重启通过list_history读取当前host序号后继续。

Session在创建时独立保有已确认的MetadataIdentity/video_id/播放source与match_id绑定。捕获阶段，前端先按实际PlayerEvent与当前playback_epoch核对身份，再携带捕获的session_id/match_id调用save_history；Rust按该活动Session持有的已确认身份核对记录及播放世代并接纳不可变快照。MatchRecord空闲回收与活动Session独立：超过5分钟或候选/匹配记录回收后，活动Session仍可接纳合法周期写入。切集、切源、离页和退出先发起捕获快照保存，等待该请求提交结果或明确失败后再关闭捕获的旧Session；视图切换及停止画面/音频可立即发生。

宿主接纳后，快照随既有历史写队列独立保有该确认身份至提交结束；提交时不再以当前页面世代、MatchRecord是否存活或旧Session是否已关闭否定已接纳快照。旧Session关闭后新发起且未曾接纳的保存请求被拒绝；已接纳但写失败的最近快照保留供同write_sequence和完全相同载荷明确重试，已被更高序号成功写入或删除/清空屏障覆盖时返回CANCELLED。不同载荷复用同序号返回BAD_REQUEST。旧事件产生的新快照、错误session/match/影片/video/来源及伪造归属均拒绝，保持原数据。

同一宿主串行入口只允许序号严格大于最近已提交序号的变更提交；旧序号返回CANCELLED并保持数据库，写失败保留原已提交序号。删除/清空成功同时发布写序号屏障，晚到的捕获旧快照及已接纳旧写仍被拒绝；新捕获动作可使用更高序号。跨进程重启从已提交SQLite记录恢复并使用新host_epoch，旧Session和未提交资格全部失效。

日期只用于排序，时间回拨保持较新写序号有效。窗口退出停止接受新动作并排空队列；未知提交结果取消收尾后list_history重读序号/记录再恢复队列。迁移、写失败、无效记录、权限/磁盘错误沿原ApiError返回脱敏原因，失败保留列表和最近未保存快照。运行期URL/cap/Session/Resolution/Cookie等字段按WatchHistoryRecord严格schema拒绝。删除/清空前的UI确认、保存时约5秒目标、重新匹配续播保持C12。

本契约是本轮规划技术决定，来源为原规格的历史功能/单调写序号和同一串行入口要求。实施该定义属于T18范围；改动本契约、既有命令或原始输入要求时遵守原冻结修订流程。

## 验收与证据契约

按当前用户决定与ADR-020实施开发/产品测试分期：开发卡实际验证功能行为、构建、相关回归和必要短时集成，长时间连续播放及T0.2的窗口耐久验收集中在T0.26/T0.28产品测试；其他开发卡现有的短时功能循环、错误/边界反例及即时资源清理继续按卡执行。T0.2功能验证复用现有open/controls/stop真实短播，两格式各600秒和5次窗口开关由T0.28/A1承接；T0.25的10分钟网关播放由T0.26/A2承接（时长与次数以ADR-044为准）。C14的指标、时长、规模、原始样本及资源清理按ADR-044执行；开发阶段报告明确尚待产品测试的项目，最终交付具备当前源码/构建的完整产品测试证据。每次修改后的真实相关自验证、完整verify和独立巡检继续执行。

每卡完整来源工作/通过/验证/停止条件原样纳入TASKS；覆盖表给准确验收ID。未来`tests/acceptance/<原卡号小写>.py`是该卡真实行为验收脚本，随当前卡实现，目前均NOT RUN。它调用真实前端/Rust/Android/设备入口及必要fixture，断言每个列出用例；入口不存在、用例缺失、未执行、非零失败、必需平台缺失、证据陈旧均失败。各卡scope允许相应测试。跨平台与长测可分会话累计，但每一验收完整执行并落原run.py收据。

脚本`--report evidence/acceptance/<原卡号>-<suite>.json`输出UTF-8 JSON，至少包含card、suite、utc_started/finished、tested_source_digest、platform/arch/OS/WebView/dependency版本、commands(argv/exit/log)、cases(id/输入/预期/实得/结果/证据)、required/ran/passed/failed/skipped计数、资源部署/采样/清理与NOT RUN原因。case集合以卡内“完整原卡义务”和覆盖表为要求，脚本需逐项关联；只读导入旧报告/空用例/预填passed时退出非零。同一次运行还以`--junit`写JUnit XML，控制器据此核对本次生成、非零通过与零失败；每项验收的`cases`冻结为本卡“需求”行的全部ID：脚本为每个ID输出一条同名汇总testcase（如`R27`），其下具体用例命名为`<ID>/<用例>`（如`R27/range-416`）；汇总testcase只在其全部必需子用例通过时通过，控制器要求每个汇总testcase出现且通过（ADR-007）。子用例skipped仅用于ADR-005非macOS平台项和卡内明确允许的条件项（如基线未出现DASH），并写明原因；macOS arm64必需子用例skipped时其汇总testcase失败。真实平台报告须匹配该平台真实执行环境及本次构建摘要；外部机器按原规格可由人类运行准确命令后带版本回填。长测保留时间序列、样本和清理日志。报告生成及断言由同一次当前代码运行完成；全量source摘要、完成资格与会话额度仍由原控制器判定。

T0.28产品独立验收的长测前置实跑与最终A1按明确证据链衔接：原run受控实跑完整时长和规模，原日志打印原始证据manifest SHA并由收据log_hash绑定，固定受验构建及源码经独立只读审查记录原始文件摘要后，最终A1在同source/protected/generation下完整重算全部原始样本与清理断言，同时核对前置收据/日志/manifest和独立报告。报告可先于A1且绑定固定受验构建提交，后续仅docs提交保持绑定。缺失、改写、旧基线或只引用passed摘要均失败。

T01测试专用最小loopback服务只入tests/fixtures，随机端口/cap/CORS/Range及超时清理；正式构建检查其排除。T06..T11以HTTP和共用合成fixture验收，原包真实条目转N02，T20/T21最终复测。所有原包运行、真实来源、平台运行、安装、性能逐类别单独统计。

workflow.verify分别校验原归档契约与实际protocol/契约（scripts/check-contracts.py），覆盖全部本地/跨文件引用、原有DTO/17命令兼容、原正负例和语义/生命周期不变量。四个历史命令为既定追加集合，T18的history.rs存在时必须全部发布并为每个Args/Result补正负例。归档原始定义及回归向量保持不变，实际protocol/追加契约；此入口始终执行，缺文件即失败。另覆盖现有npm行为测试、类型/构建、导入摘要，以及Rust存在后的fmt/check/clippy/test。verify-workflow仅控制流程测试。Android每卡构建/instrumentation、macOS长媒体/故障/安装按具名卡执行，保留固定源码/制品/期望结果。当前业务长验收NOT RUN；Rust未创建时check-project明确记录NOT RUN，T01/T05创建后自动执行原生检查。

## 输入摘要绑定

以下全部位于项目内replica-spec/，SOURCE_MANIFEST逐项同值；check-project复核文件摘要和SPEC字节。独立审查额外绑定原task.py status的protected及已采纳ADR摘要。修改归档/受审定义或影响要求的配置时重审。

| 输入文件 | SHA-256 |
| --- | --- |
| `AI_TASKS.md` | `89acb3b77e37366e6df6b7645faac06c345620a337d84a1c7d46eee847827c61` |
| `DATA_STRUCTURES.md` | `90d13c6a3f9554c868063bea40990bf0bb50af976f61cf92bfcb3520f96d0b74` |
| `DEVELOPMENT_SPEC.md` | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| `README.md` | `b027aa63702d9d3a5c2eb5353a55715231e2a7df58a2ee69ff69da41040228ae` |
| `app.schema.json` | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` |
| `bridge.openapi.json` | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` |
| `examples/contract-cases.json` | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` |
| `references/SOURCES.md` | `b090a93ed5194128c89834a38ffc450712ce7588c07048f5f1d7da72fa6b75a5` |
| `references/android-aosp-images.json` | `17876f1253308f01e1daf96809bc5d5a3b90f3c02f52ae0849c9108262b7a889` |
| `references/android-google-images.json` | `515bd488a54fc93e92dee725dc24306ef1ba68e996d3a97a2c8c5ad74a73db39` |
| `references/app-data-evidence.json` | `451f0e156dcc0d8371893d1d0ac437fd37c129af15948c5c51c3105a46848792` |
| `references/app-model-rea-evidence.json` | `89557245477b6c2210b20217ebc83c99d784e95ddce9a277b6e14c49ad98afcf` |
| `references/plugin-artifact-inventory.json` | `290f670e3f86719f76a7239a98dc21c2d0d112463e1543ceeecd0162023e516b` |
| `references/plugins.snapshot.json` | `a514c083b372a2016a2a87ba0425d559caa8432372176ec78cdce346350643ef` |
| `references/validation-report.json` | `9e4ea6981e7f910fe4cc21be50ab02cefbd80a025f6ed75f98352ac0d202105e` |
| `validate_contracts.py` | `9f6bc407045563a60118c72a1baf6ea6c1632d8d2852793f8fb73876a6bc39e9` |
