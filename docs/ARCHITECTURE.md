# 墨客视频架构与契约

当前版本已按用户要求移除旧版本兼容：使用 `app.moc.video` 和 `moc-video.db`，仅读取当前格式；下文旧版升级验收保留为历史记录。

## 当前验收与流程（ADR-062）

自 T0.34 起仅验证当前任务及受本次改动影响的行为；全量 verify、逐卡提交、独立巡检和冻结修订的手工基线/复审均按需要执行。后文与本条冲突的全量流程为历史要求；既有证据保留。T0.28–T0.34 的验收标准需扩大时仍须用户批准。

状态：T0.1至T0.17已完成，Rust来源路线按ADR-048修订，T0.19及后卡实现验收NOT RUN。需求原文保留逐字节归档的SPEC 1.5.0及replica-spec机器契约；当前批准差异由ADR-005、ADR-044、ADR-048及以下契约定义。原规格中的application/前缀映射到本项目根，docs/progress.md映射到docs/PROGRESS.md（ADR-001）。

## 现有能力与最小修改路径

已阅读现有App.vue、UiProvider、theme、i18n、HistoryView、SourcesView、SettingsView及测试入口。保留Vue 3.5.44、Naive UI 2.45.3、TypeScript 5.9.3、Vite 8.3.5与锁文件，版本事实来自package.json/package-lock.json。复用FilmCard、Icon、样式、双语词典、主题及播放事件。demo.ts与会话内history.ts在相应接线卡替换入口。Rust宿主、Stremio/MacCMS、匹配与网关已实现，实际证据见PROGRESS；Android补丁构建中断且未成功。

当前最小方案：一个前端项目、一个Rust app crate；api.ts唯一invoke入口，source_dispatch.rs复用现有maccms/stremio分发，matching.rs保留元数据与播放两条身份。SuperVod普通JSON来源作为现有MacCmsConfig预设，由用户显式添加，复用maccms.rs、resolution.rs及gateway.rs。新增provider接口/trait/注册框架须先证明现有实现无法满足当前实际行为；首版无需新增。原生来源依据站点公开响应独立实现；已读插件源码只用于识别范围和许可风险，不作为翻译实现依据。

## 必须保持的契约 [C]

### C1 项目与原脚本绑定（用户路径确认；moc-init运行约束）

唯一根目录`/Users/eason/jp/application`；工作目录、Git、原scripts、`.venv/bin/python`、`evidence/`和`evidence/drive/driver.lock`均绑定该根（已核对scripts/workflow.py的实际锁路径）。初始化宿主Python为`/Users/eason/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3`，3.12.14；此后控制脚本使用`/Users/eason/jp/application/.venv/bin/python`。迁移至另一系统时重新核实根/venv/进程与授权，使用该系统实际venv解释器。

按ADR-013及当前聊天用户授权，本项目heavy_lock使用绝对路径`/Users/eason/jp/application/evidence/drive/heavy.lock`，仅串行本项目的重任务，独立于其他项目的共享锁排队。其他项目原共享锁保持。迁移系统或项目路径时重新绑定此绝对路径并按原冻结流程审查。卡号映射只在TASKS内记录；沿用原task/run/verify/drive状态、收据与完成提交判定。实际锁文件路径由T00读取原runtime核对。首次开发前确认无存活驱动/CLI/重命令/巡检，核对PID＋创建时间。

### C2 需求基线与阶段（SPEC §1/§14/§16；ADR-002）

U01..U09按TASKS覆盖表与ADR-048修订：U02改为可用SuperVod JSON来源；U05原包兼容义务由独立Rust来源实际行为取代；U08为Stremio元数据与Rust播放。G0-CS取消，G2改为核准JSON来源身份/能力/媒体闭环；G0-Core/G1/G3/G4/G5及后续MG0..MG3按当前平台和来源范围验收。原29张桌面卡全部验收后交付桌面；后续6张移动卡独立授权。SPEC列出的搜索UI、收藏、账户/支付/广告/P2P/离线等维持原范围决定。原README中的开发AI启动文字作为资料。开发前须当前文档独立CLEAR、独立审查与开发执行分离、沿用workflow.json预算/权限和用户开发指令；建立真实heartbeat。ADR-045允许当前对话按顺序开发，sessions按真实入口保留，原验收与review-only巡检门持续适用。每卡结束独立巡检；T25完成停止桌面监管，报告后续M列表。

### C3 线格式与角色（SPEC §7/§10.2/§18.1；DATA_STRUCTURES §2/§9/§13）

归档bridge.openapi保留旧Android协议资料；工作protocol/中app.schema是17个invoke、事件、Settings和状态权威，复用HTTP DTO，snake_case统一。invoke(command,{request_id,input})；结果data/error互斥。Rust业务包络engine_epoch为host_epoch；ready必须UUID及health ready。固定命令和主WebView权限，拒绝任意进程/文件写入入口。空值/单位/UTF-8预算/有限数值按字典§6及schema。项目内protocol/已导入两份实际机器契约和原样例；T05据此生成/校验DTO。T18按C16既定设计补历史IPC/正负例后写宿主，保持既有17个命令兼容。T0.24按ADR-054新增C11的两项缓存IPC，既有21项继续兼容。偏离受审定义时按冻结修订与独立重审流程处理。

### C4 原生JSON来源与独立实现（ADR-048取代SPEC §2/§6原插件义务）

首版仅个人使用，范围是可用SuperVod JSON来源。2026-11-05普通GET核实的候选：优质、量子、红牛、无尽、光速、极速、豪华的`ac=detail&pg=1`返回JSON列表且有播放字段；淘片返回简略JSON列表尚需详情核实；卧龙返回非JSON暂排除。上述属于候选，逐个通过协议/完整选集/媒体验证后才发布为支持。其余六包站点、SuperVod XML和特殊播放器API暂缓。每个候选保留日期、公开端点、请求参数、响应类型/计数、支持/失败/NOT RUN状态及脱敏依据，媒体/签名URL与凭据留原生内存。HTTP200只记录可达性。

固定插件提交`25cdd2119be03f62ae6a5a2cf46b16f9484224a2`的SuperVod 15个Kotlin文件静态核实：`aesDecrypt`只在Utils.kt定义、无调用；BaseVodProvider输出已登记媒体地址，VodExtractor的encrypt=1/2是转义/Base64，Haiwaikan/JiangNan注册注释。此核实用于确定首版范围：普通JSON适配无需移植插件AES或播放器代码；HLS标准AES-128 key/IV仍沿C8完整验收。产品实现及fixture来自公开HTTP行为；原包、补丁及旧证据保留历史，运行和打包路径停止使用Android/DEX。

以maccms kind表示上述来源；预设只包含公开配置且由用户确认添加，最多32配置、UUID/HTTP H身份与原配置兼容。支持状态以当前实测为准，失败来源原配置和用户数据保留。需要其他来源、账户/访问密钥、解密播放器或访问控制处理时报告具体前提，按授权范围推进独立部分。

### C5 身份、跨源匹配（SPEC §7.2/§18.5/§18.6；字典§5/§11）

统一H使用tag+NUL与UTF-8 uint32长度前缀、SHA256小写hex；所有原公式直接复用归档向量。Stremio为产品元数据，核准SuperVod JSON与用户已有MacCMS为Rust播放来源；独立来源偏好。content_id=H(metadata,namespace,type,canonical_id)，config_id/addon_key准确回源；已验证IMDb/TMDB才归一，自定义命名空间按addon_key隔离。语言保持稳定身份。

match_playback每次一个选中provider，最多2次查询×10候选，最多20项；全部正常且零候选为not_found；失败/超时为ApiError。仅同一已验证外部ID、类型且唯一季集可自动确认，其余needs_confirmation。人工selection重新核验已登记内容/组/集/修订。无search时按原能力显示手动home/details入口。match_id最多256、空闲5分钟，绑定host/config/video/provider引用；resolve重新核验。季0、小数集、别名/重拍、乱序、旧修订与过期均有负例。Stremio媒体resolve拒绝；下一集按metadata video顺序重新匹配。

C5 补充（ADR-061）：跨源影片由用户明确确认候选内容及线路、当前会话实际播放推进后，可对该元数据影片与所确认来源内容建立受限偏好。后续每集重新获取当前来源内容和线路数据，仅当来源修订一致、内容仍为已确认的同一内容、目标季集及已确认线路均可由可审计标识唯一对应时，允许使用该偏好自动选定该集；可审计对应至少绑定用户确认的元数据影片身份、来源及修订、已确认来源内容；来源没有结构化季集字段时，只有能与目标季集互相校验且唯一的季集标签可作为证据，数组下标、单个旧 group_id 或仅标题相似均不足。即使来源 revision 和内容 ID 未变，也须用首次确认时的非敏感稳定内容证据交叉核验当前快照；字段冲突或无法证明同一内容时回确认。重复或缺失集号、跨季歧义、线路重排或同名导致无法唯一确认、内容标识变化时走 needs_confirmation。旧的 match/Resolution/Session/Stream 及 URL 均不得复用或持久化；来源修订变化、已确认内容失效或用户选择其他来源时偏好不得越权。没有受限偏好的首次跨源匹配仍执行原已验证外部ID、类型及唯一季集门。

### C6 HTTP与网络（SPEC §12/§12.1/§18.2/§18.3）

MacCMS匿名JSON、首个$拆条目而保留URL余串，组/集原顺序与歧义失败；类型探测有界。Stremio按完整manifest快照发布，首次失败unpublished且能力false，刷新失败保留旧快照；资源过滤/required extras/skip/同名栏目/配置路径query保持，详情按原扩展与顺序回退，warnings最多64。stream/inline媒体字段按元数据角色忽略。

配置最多32、每Stremio配置1..16 manifests，UUID唯一，禁用/删除/重加的generation墓碑与sync_sequence防止旧提交。共享HTTP入口覆盖API/manifest/图像/媒体/字幕/HLS/key/探测：允许公网、内网、回环、保留及代理Fake-IP地址，不校验DNS结果或实际peer的IP范围及一致性（ADR-068）。解析结果仍绑定连接，保持Host/TLS/SNI与证书校验；逐跳最多5次且拒绝HTTPS降级，跨Origin清除敏感头，禁用环境代理，支持系统TUN透明代理。

ADR-049 的固定DNS继续生效：共享HTTP域名只通过1.1.1.1:53和8.8.8.8:53解析A/AAAA，复用hickory异步解析器及其受操作总deadline约束的失败转移；不读取系统DNS、hosts、搜索域或环境DNS配置，无本机回退。所有API/图片/媒体/字幕/HLS子资源及每次重定向共用此入口。空结果或两服务器均失败时明确返回DNS错误。IP字面量直接连接。测试专用nameserver/fixture映射仅cfg(test)。明文DNS可能被TUN拦截，Fake-IP按原结果连接；ADR-068取代原公网地址限制与peer核验。桌面字体下载经独立cap的本地字体网关，使用同一原生HTTP/DNS入口并校验固定大小及SHA256（ADR-070）。

### C7 请求预算与取消（SPEC §6.2/§7.3/§11.2）

下表保留原生HTTP操作预算，排队、重试、重定向与全部子请求共享单调deadline。聚合提前1000ms取消收尾，前端传输余量2000ms后主动取消，公共元数据4槽。连接上限10000ms并裁剪剩余，流无字节空闲30000ms。UUID重复/自身取消BAD_REQUEST，取消独立控制路径，父子级联，finally清理，返回前核对取消与世代。

| 操作 | 总预算ms | 单子请求上限ms | 聚合并发上限 |
|---|---:|---:|---:|
| 默认元数据/详情/搜索/HTTP单栏请求 | 30000 | 30000 | 1 |
| MacCMS首页第1页 | 45000 | 30000 | 4 |
| Stremio首页第1页 | 45000 | 30000 | 4 |
| HTTP来源同步（含全部manifest） | 45000 | 15000 | 4 |
| resolve（MacCMS JSON） | 45000 | 45000 | 1 |
| match_playback | 45000 | 15000 | 4 |
| health / get_resource_status | 3000 | 3000 | 1 |
| runtime_prepare | 120000 | 120000 | 1 |

### C8 Session所有权与资源代理（SPEC §5.4/§9；字典§10）

资源cap由独立256位随机值鉴权，控制仅通过Tauri允许列表；保留原有控制凭据字段兼容性，真实凭据只在原生内存保管并脱敏。网关仅loopback，资源GET/HEAD限定登记ID；固定Origin CORS/必要OPTIONS与CSP。Rust执行Range200/206/416、If-Range、If-None-Match、压缩和重定向fixtures；按已验证来源上下文处理请求，重写清单后重算头。

Session独立强持有LinkRecord、Cookie/interceptor与字幕/HLS上下文；Resolution释放/5分钟到期后已建Session继续。active_requests钉住、空闲5分钟释放；同集换线保留Resolution，切集/源/离页按捕获旧ID依次stop、close、release；旧清理幂等且保持新句柄。媒体初始64KiB流缓冲；清单≤8MiB，请求≤1MiB、响应≤8MiB、opaque UTF-8≤524288字节。图像/字幕大小及像素上限需T09按实际解码器明确并固化测试，保持RESOURCE_LIMIT。图片同epoch去重、30分钟滑动续租、默认4096 LRU。

HLS主/子、key、map、音轨/字幕、相对地址、签名、byterange、重定向基址、序列/不连续与未实现tag保留，全部派生资源同会话cap。基线实际出现清晰DASH时纳入BaseURL/List/Base/SIDX及受约束Template；字幕VTT/SRT/ASS基础文本、时间轴、鉴权及回收；DRM明确MEDIA_UNSUPPORTED。

### C9 资源诊断与恢复（SPEC §10.5）

get_resource_status只读登记状态、3秒预算，ready/expired/failed；最近请求序号控制错误归并，同资源成功清旧错，其他资源成功保持原失败。未知ID同expired；查询不访问上游及不续租。前端一次加载尝试最多一次查询，验证request/epoch/revision/target；图片所属DTO最多刷新一次，播放过期resolve/open/ResumeSnapshot恢复一次。业务错误按error.code；保留原MediaError及查询失败诊断。

媒体磁盘缓存（用户新增需求，ADR-054）：缓存只在网关完成现有Session/cap/source_revision鉴权之后查找，命中不创建或延长任何授权。仅缓存完整接收的HLS媒体分片/初始化片段及MP4完整响应或原样Range响应；动态清单、密钥、图片和字幕继续原路径。Range以精确请求为键，不拼接不同字节段；HEAD、条件请求与显式no-cache请求沿原网络路径。缓存键包含来源修订、上游URL、原生请求头和Range等全部上游表示上下文的哈希；磁盘只保留哈希、受限表示头和媒体字节，不持久化URL、Cookie、Authorization或本地cap。上游no-store/no-cache及Vary:*不存储；max-age/Expires扣除Age与Date后决定新鲜期，缺少显式期限的可缓存媒体采用最多60秒启发式新鲜期；过期重新走原网关请求，不返回过期内容，签名上下文及TLS/SSRF/访问控制门保持。

缓存位于应用数据目录专用media-cache子目录，复用原有依赖。默认最大268435456字节（256MiB），0关闭并释放可回收内容；用户可在设置修改，旧设置缺字段按默认。按LRU淘汰，索引最多4096条；计入缓存文件头、完整正文和临时写入的逻辑字节，文件系统块/元数据开销由操作系统决定。每块写入先预留容量，超限/写错时停止该次缓存写入并回收临时文件，原播放流继续且缓存诊断可查询；完整响应原子发布，取消/断流不发布残片，启动删除本模块遗留临时与损坏条目并恢复有效条目。清理或降额先建立世代屏障，旧在途写入不得回填；已打开读取可以完成，待释放字节计入占用并显示pending_bytes，关闭后删除，未释放前不借用这部分容量。清理只操作自身目录内识别出的缓存条目，文件系统失败显式报告，设置页保留重试与实际占用，不误报清零。此处容量为缓存硬上限策略，原进程RSS/网络并发/背压预算保持。

### C10 状态与界面（SPEC §8/§10.3/§11.1）

单AppState使用现有Vue页面与Naive唯一UiProvider。host重启全部失效，来源revision/generation变更仅影响所属来源。首页按全量/单栏重试/分页范围合并；index保留，空/错及部分失败分开；请求旧结果/旧EOS无副作用。前端原演示数据在真实接线验收时由invoke替换。

默认1200×800、最小960×640、360/390/768 CSS宽度、高DPI；首页4..7卡/行、海报约150..180与2:3方向，缺图/长文/200集可读。按用户指定与ADR-055采用Video.js 10.3.0官方播放器、官方控制栏与皮肤，Vue沿官方@videojs/html自定义元素接入，精确版本写依赖及锁文件；移除自搭基础控制栏和直接管理hls.js实例的实现。仅保留对接网关Session、业务选集/下一集/线路、历史及原生全屏的薄适配。控件、时间轴、音量、字幕和倍速由官方组件提供，维持图标/悬浮说明、键盘和无障碍名称、中英文/系统明暗/390px；不同时运行第二套播放器或基础控制栏。实际媒体事件映射原PlayerEvent、250ms合并位置、旧epoch/旧Session事件丢弃、焦点恢复和autoplay提示保持，EOS按选项与当前epoch下一集。界面及匹配确认按原卡全部错误/取消用例验收。

Video.js迁移（ADR-055）承接旧T01/T12/T16播放器要求，历史完成卡正文与原始SPEC保持原文。媒体只使用现有Rust网关签发的media_url/字幕URL，缓存、鉴权、清单重写和HTTP策略保持；不启用CDN、分析或外部媒体服务，不放宽CSP/CORS/TLS及Tauri权限。MP4/HLS及字幕/续播/切线切集均通过官方媒体适配及store/原生事件与既有状态控制器桥接；停止、替换来源和卸载时释放官方实例/订阅/媒体网络与DOM轨道，不残留旧音频。单媒体所有权与50次切集回收仍通过真实行为验证，不以旧hls.js实例计数或手写控件选择器代替。桌面全屏接回既有原生窗口命令；保留停止/错误重试、当前视频EOS与自动下一集确认。

### C11 设置、双语与外观（SPEC §8.1/§11.3/§18.3.1）

Settings v2原子JSON与v1迁移，单前端写队列+同宿主提交锁；未知提交暂停、取消/收尾后同锁重读，失败保持确认值。保存成功先发布设置/注册表及世代后释放锁；配置探测锁外。退出排空队列，损坏保留诊断副本。

默认en与lime，en/zh-CN文案与Naive locale、文档lang/title/无障碍名称同步；中文变体→zh-CN，其余→en。Stremio扩展语言方式在N01实证，英文基础ID与有效同ID中文覆盖，缺失/失败逐字段回退并记录原因；原预算共享、分语言缓存、语言/页面世代抵御中英中乱序，播放会话保留。系统明暗useOsTheme，五色系×明暗分别验证，首帧恢复色系，旧ui_theme兼容读取并忽略显示效果，变更保留路由/播放/筛选，存储失败反馈。

Settings v2新增可选media_cache_max_bytes（整数0..9007199254740991，缺省268435456），通过原单队列和原子写盘持久化；旧字段、schema_version及既有21项IPC保持兼容。保存失败沿原确认值保持规则；保存成功后缓存策略采用新上限，已在途读取/写入的延迟释放与缓存I/O诊断通过状态明确呈现。设置页提供最大容量（MiB）、已用容量、待释放占用、保存和清理按钮，覆盖中文/英文及窄屏，错误保留实际状态并可重试。

新增两项固定主WebView IPC，合计23项（ADR-054），不接收路径或URL，沿原request_id/engine_epoch/data/error与请求预算/取消规则：get_media_cache input={}；clear_media_cache input={}；两者成功data均为MediaCacheStatus {max_bytes, used_bytes, pending_bytes, entry_count, last_error}，字节数/条目数为非负安全整数，last_error为ApiError|null。used_bytes含待释放内容；clear成功表示新查找不再命中被清条目，实际未释放字节由pending_bytes列明；删除失败走error且后续查询保留实际占用。缓存I/O失败不会阻断已有播放，last_error必须可在设置页展示。沿default_metadata的30秒预算，清理在实际文件变更前检查取消；一旦开始，完成屏障和状态发布，未知提交沿既有重读规则处理。

### C12 历史与数据完整性（SPEC §11.4）

Rust应用数据目录moc-video.db，PRAGMA user_version独立迁移，事务失败保留数据，参数化SQL。watch_history完整列及索引按SPEC逐项落实；长期仅保存MetadataIdentity/video/季集/source与展示快照，临时URL/cap/Session/Cookie/令牌留在原生命周期。

按影片合并最近一集/今天昨天更早；约5秒为成功调度写入目标，暂停/seek/切集/离页/退出/后台提交捕获快照；世代与单调写序号防乱序/时钟回拨。删除/清空确认及写队列共用串行入口，旧快照作废，写错保留可重试快照并显示未保存。续播先定位原config/addon→当前详情/video→重新匹配播放源→新会话，loadedmetadata后恢复且裁剪新时长；EOS仅当前集完成，重播0。T18/T20 macOS arm64真实SQLite跨重启与失败证据。

### C13 原生来源生命周期与配置事务（ADR-048取代SPEC §5/§6.1）

应用启动直接准备Rust宿主和loopback媒体网关，无SDK/AVD/ADB/APK前提。单来源探测/请求失败保持Stremio及其他来源可用。访问控制、验证码、Cloudflare或证书失败准确显示阶段与原因，停止该来源请求并报告；保留重试/换源入口，无规避、TLS关闭或隐藏WebView方案。

T0.19只接入实际支持的JSON；T0.20验证身份/完整性；T0.21验证请求上下文/取消/隔离；T0.22完成来源预设与UI整合。现有cloudstream枚举/请求及runtime字段保留线格式兼容，准备/交互请求返回明确未支持错误，来源状态准确；T0.22移除页面失效引擎操作并提供迁移说明，T0.18保留旧偏好/历史及诊断。旧值由用户显式选择新来源后更新，禁止静默关联、丢弃配置或伪装成功。

T0.23复用设置提交锁、config_generation与sync_sequence：网络探测在锁外；失败/无变更保持已确认配置和播放。提交受影响来源变更时取消该来源请求、停止会话及撤销cap，发布新配置/修订；其他来源保持。持久化/发布失败保留原确认状态；已经关闭的会话保持停止，重新解析后方可恢复。旧结果在锁内核对世代后丢弃，清理入口幂等。远程代码/原包下载、ZIP安装和索引自动更新入口退出首版产品路径，历史材料保留。

### C14 平台、性能与资源（SPEC §5.1/§13；moc-init运行约束）

必需目标为本机macOS14+ arm64（ADR-005，macOS first）；构建与真实运行分别记录。原规格的macOS x86_64、Windows11 x86_64、Ubuntu24.04 X11/Wayland降为非阻断记录项：代码保持cfg可移植，报告写NOT RUN与复现命令，后续另行授权；移动机器在M00确认。个人使用包记录本机真实配置、下载大小与磁盘占用；16GB内存/12GB磁盘仅沿原资料作观察起点，Android镜像预算随路线取消。缓存128MiB内存观察预算保持；媒体磁盘256MiB按ADR-054作为可调默认值，其他原资源预算保持。

工作流当前模板值memory_mib=4096、jobs=2、command_seconds=7200、session_seconds=10800、max_sessions=2；这是保留的控制器默认配置，开发启动前核实用户预算及每卡完整长命令覆盖。宿主macOS使用RSS采样，Linux原cgroup；线程1。资源不足时准确报告所需预算与完整命令，预算调整走明确授权与重审。

真实集群测试服务成员Docker部署，逐容器硬内存上限；容器硬上限合计+宿主进程树采样RSS纳入批准总预算，运行时同时记录actual usage，成功/失败/取消仅清理带本次身份标签的自产容器/网络/卷。桌面应用在目标系统运行；普通单进程fixture/真实集群部署类型在测试报告明确。当前未批准具体集群拓扑/容器预算；涉及该场景时须先明确额度并满足Docker契约，T21/M04若引入真实集群时对应验收拒绝漏项。

以下按ADR-044放宽SPEC §13：娱乐软件以无泄漏、无关键可靠性问题为门，响应类数值为记录目标，未达记差距不阻塞。T22在macOS arm64记录原始样本、测量起止、样本数/p95、CPU/GPU/OS、依赖版本、分辨率/帧率/码率/RSS。参考机定义及本地桥接样本数在T00/T22测量方案实证后锁定；缺失时标为待验证，禁止把本机校准写成产品硬门槛。目标未达需明确差距与复现；关键可靠性问题清零后再进入发行。

| 项目 | 目标与测量条件 |
|---|---|
| 壳首次可见 | 参考机启动后1.5秒内呈现窗口与状态 |
| UI动作反馈 | 常见点击100ms内显示状态变化 |
| 本地桥接纯处理 | 无外网fixture的元数据请求p95低于100ms |
| 宿主冷启动 | 已安装环境参考机目标60秒内健康，记录5次样本；Android引擎取消 |
| 播放 | 自有1080p30/H264+AAC样本连续10分钟，音画正常、可seek |
| 本地代理吞吐 | 20Mbps受控流，连续3分钟且保持流式内存上界 |
| 连续切集 | 50次切换后无旧音频、旧帧、悬挂会话或内存泄漏（owned RSS五切窗口中位数首末增长≤64MiB且均≤512MiB） |
| macOS UI | 最小窗口、125%/150%/200%DPI（或macOS等效缩放）与常见全屏状态可用 |

### C15 移动复用与发行（SPEC §10.4/§14/§16）

桌面lib.rs移动公共入口/cfg隔离；移动后续复用DTO/状态/页面/Rust HTTP、匹配及SQLite。Android/iOS各自本地执行共用Rust来源与网关，按真机网络策略验证TLS、来源/Session/cap隔离及断线恢复；Android单应用，iOS无需远程Android设备。360/390/768、44px触控目标、safe area/playsinline/系统返回、旋转、音频中断、前后台暂停及进程回收真机验收。最低OS/工具链M00实测，MG0..3逐项证据。

macOS arm64个人使用包实际签名公证状态、实际依赖许可与版本锁、离线打包资源按T23/T24，干净安装、初始化、首播、升级/卸载按ADR-058由T0.36/A2执行；原插件/引擎APK/SDK镜像排除打包。公开发行须另获用户授权。WebView2/WebKitGTK/GStreamer等其他平台项列NOT RUN。缺Apple签名凭据时产出未签名包并列为发行待办；真实源失败按当前公开请求记录日期/环境/阶段，访问控制停止报告，在线与fixture结果分别列明。

### C16 历史IPC的最小实现契约（SPEC §11.4/§18.6的规划细化）

此处固定T18的既定交付字段，当前实际protocol/保留原17命令；T18追加以下四个命令及schema/正负例，再实现Rust/SQLite，合计21命令。旧Android线格式保留归档和兼容声明。所有命令沿用invoke(command,{request_id,input})与结果data/error互斥、host_epoch UUID及既有ApiError。

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

### C17 桌面 Apple TV 化（ADR-057）

本节在宽度不小于960px的桌面布局上优先于C10、C11中的对应条款；小于960px的布局、明暗跟随和既有行为保持C10、C11。
界面：沉浸标题栏与悬浮侧栏；首页及分类页使用横向内容排，竖版海报，每排数量随窗口宽度变化，取代C10的“4..7卡/行”；筛选后使用海报墙。桌面固定深色，五种色系继续作为强调色，取代C11对桌面明暗跟随系统的要求。
播放器：沿用Video.js 10.3.0内核及官方元素，以官方元素组合的自定义皮肤取代`video-compat-skin`，修订ADR-055的“官方皮肤”；网关Session、单媒体所有权、250ms位置、旧epoch丢弃、焦点恢复、原生全屏和50次切集回收继续有效。
目录：`Bridge_MediaCard`、`Bridge_HomeSection`、`Bridge_HomeRequest`按T0.29扩展可选字段；Stremio组合筛选由Rust完成，服务端条件优先级为语言、年份、类型，单次最多扫描5页。MacCMS的服务端条件限于分类ID、分页和关键词。筛选含类型、年份、语言、地区、排序，每个栏目返回各维度可用性。TMDB的地区禁用；排序只在无语言和年份条件时可用。
按ADR-059，`HomeRequest.catalog_only`可选，缺省或false保持原首页行为；true仅用于Stremio、page=1、无section_ref和filter，返回宿主登记栏目描述及空items、page=1、has_next=false、next_page=null、计数0，不请求目录内容、不登记图片或推进分页游标。其他来源和非法组合明确报错。引用、取消、来源修订及语言世代规则保持；内容排进入视口才请求该栏内容。
描述阶段属于待加载，不能因空items判定为真实空结果；首个内容请求完成后才判断该排是否为空。页面排身份由来源及修订、语言世代、栏目引用和规范化筛选条件组成，加载、错误、结果、分页及重试各自隔离，旧响应不能覆盖当前排。catalog_only=true时section_ref和filter缺省或null等价，filter空对象视为非法组合。
契约检查：`scripts/check-contracts.py`收紧为归档字段逐项相等，只放行本节、T0.29列名及ADR-059的可选字段；ADR-065新增TMDB、直播、本地库可选设置，字体设置由ADR-070补充；命令追加集按卡递增，T0.32为HISTORY、CACHE、WATCHLIST，T0.34再加`search_catalog`。上文“原有DTO兼容”按此解释。
同源播放：`metadata_ref.identity.namespace`为`maccms`时，元数据与播放可来自同一MacCMS来源；Stremio元数据的跨源匹配和绑定校验保持。
IPC与权限：在C11的23项基础上增加`list_watchlist`、`add_watchlist`、`remove_watchlist`、`search_catalog`，合计27项，沿用request_id、engine_epoch、预算和取消规则；`list_watchlist`输入为空，返回`{entries}`；`add_watchlist`输入观看身份字段、title、kind、poster_cache_key，`remove_watchlist`输入`{content_id}`；`search_catalog`输入`{source_id,source_revision,query}`，返回`{items}`，按default_metadata预算、每来源串行，界面取消旧请求；四项都拒绝URL、cap和Cookie；主窗口权限只增加这4项及`core:window:allow-start-dragging`。CSP、CORS、TLS和媒体网关策略保持。
数据：SQLite版本升至2，新增watchlist表，`watch_history`结构保持。

## 验收与证据契约

按当前用户决定与ADR-020实施开发/产品测试分期：开发卡实际验证功能行为、构建、相关回归和必要短时集成，长时间连续播放及T0.2的窗口耐久验收集中在T0.26/T0.35产品测试；其他开发卡现有的短时功能循环、错误/边界反例及即时资源清理继续按卡执行。T0.2功能验证复用现有open/controls/stop真实短播，两格式各600秒和5次窗口开关由T0.35/A1承接；T0.25的10分钟网关播放由T0.26/A2承接（时长与次数以ADR-044为准）。C14的指标、时长、规模、原始样本及资源清理按ADR-044执行；开发阶段报告明确尚待产品测试的项目，最终交付具备当前源码/构建的完整产品测试证据。每次修改后的真实相关自验证、完整verify和独立巡检继续执行。

每卡当前工作/通过/验证/停止条件纳入TASKS；ADR-048迁移项注明原文及取代依据；覆盖表给准确验收ID。未来`tests/acceptance/<原卡号小写>.py`是该卡真实行为验收脚本，随当前卡实现，目前均NOT RUN。它调用真实前端/Rust/设备入口及必要fixture，断言每个列出用例；入口不存在、用例缺失、未执行、非零失败、必需平台缺失、证据陈旧均失败。各卡scope允许相应测试。跨平台与长测可分会话累计，但每一验收完整执行并落原run.py收据。

脚本`--report evidence/acceptance/<原卡号>-<suite>.json`输出UTF-8 JSON，至少包含card、suite、utc_started/finished、tested_source_digest、platform/arch/OS/WebView/dependency版本、commands(argv/exit/log)、cases(id/输入/预期/实得/结果/证据)、required/ran/passed/failed/skipped计数、资源部署/采样/清理与NOT RUN原因。case集合以卡内当前义务和覆盖表为要求，脚本需逐项关联；只读导入旧报告/空用例/预填passed时退出非零。同一次运行还以`--junit`写JUnit XML，控制器据此核对本次生成、非零通过与零失败；每项验收的`cases`冻结为本卡“需求”行的全部ID：脚本为每个ID输出一条同名汇总testcase（如`R27`），其下具体用例命名为`<ID>/<用例>`（如`R27/range-416`）；汇总testcase只在其全部必需子用例通过时通过，控制器要求每个汇总testcase出现且通过（ADR-007）。子用例skipped仅用于ADR-005非macOS平台项和卡内明确允许的条件项（如基线未出现DASH），并写明原因；macOS arm64必需子用例skipped时其汇总testcase失败。真实平台报告须匹配该平台真实执行环境及本次构建摘要；外部机器按原规格可由人类运行准确命令后带版本回填。长测保留时间序列、样本和清理日志。报告生成及断言由同一次当前代码运行完成；全量source摘要、完成资格与会话额度仍由原控制器判定。

T0.35产品独立验收的长测前置实跑与最终A1按明确证据链衔接：原run受控实跑完整时长和规模，原日志打印原始证据manifest SHA并由收据log_hash绑定，固定受验构建及源码经独立只读审查记录原始文件摘要后，最终A1在同source/protected/generation下完整重算全部原始样本与清理断言，同时核对前置收据/日志/manifest和独立报告。报告可先于A1且绑定固定受验构建提交，后续仅docs提交保持绑定。缺失、改写、旧基线或只引用passed摘要均失败。

T01测试专用最小loopback服务只入tests/fixtures，随机端口/cap/CORS/Range及超时清理；正式构建检查其排除。T06..T11保留HTTP和共用合成fixture历史；后续JSON来源以公开响应fixture覆盖行为，并在T0.19/T0.24补真实站点冒烟；全部晋级来源逐个验证JSON及媒体。在线失败保留具体原因，fixture成功与在线成功分别统计。每卡至少一个必需断言实际运行，零测试失败；公开媒体URL及凭据不写fixture/日志/报告，测试使用脱敏引用或自有媒体地址。

workflow.verify分别校验原归档契约与实际protocol/契约（scripts/check-contracts.py），覆盖全部本地/跨文件引用、原有DTO/17命令兼容、原正负例和语义/生命周期不变量。四个历史命令为既定追加集合，T18的history.rs存在时必须全部发布并为每个Args/Result补正负例。归档原始定义及回归向量保持不变，实际protocol/追加契约；此入口始终执行，缺文件即失败。另覆盖现有npm行为测试、类型/构建、导入摘要，以及Rust存在后的fmt/check/clippy/test。verify-workflow仅控制流程测试。Rust来源集成、macOS长媒体/故障/安装按具名卡执行，保留固定源码/制品/期望结果。未执行的业务长验收保持NOT RUN；当前Rust已创建，check-project沿原入口执行原生检查。

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
