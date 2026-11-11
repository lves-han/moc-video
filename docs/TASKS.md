# 墨客视频任务路线

状态：T0.1至T0.17已完成；当前T0.19按ADR-048迁移为Rust播放源路线，后续业务验收NOT RUN。48张桌面、6张后续移动：T0.28至T0.34为ADR-057新增的Apple TV化改造卡，原T0.28、T0.29按ADR-057顺延为T0.35、T0.36，已完成卡正文及其验收脚本中的“T0.28/A1”指顺延后的T0.35/A1；其余沿用原卡号；已完成卡正文及历史保持。原SPEC逐字节归档，生效差异见ADR-005、ADR-044、ADR-048及ARCHITECTURE。

平台范围（ADR-005/ADR-006，macOS first）：桌面必需验收目标只有macOS arm64（本机）。各卡“原始义务”及覆盖表中的macOS x86_64、Windows、Ubuntu X11/Wayland运行、安装、性能与签名要求降为非阻断记录项：代码保持cfg可移植，验收报告列为NOT RUN并给出复现命令，后续另行授权；本机macOS arm64缺测、失败或伪称其他平台通过时验收失败。

## 路线与执行前提

## 当前任务验收规则（2026-11-10）

按用户要求，自 T0.34 起每张任务卡只验收本卡内容；其他任务仅在本次改动影响其相关行为时做必要回归。任务卡只列直接验证需求所需的检查，不为凑覆盖重复已有任务。实际验证结果和未验证事项如实记录。

每卡不再强制执行全量 verify、Git 提交或独立巡检；按变更风险和需要自行选择相关检查。冻结文档或任务定义按用户已授权范围调整，无须为每次调整手工更新基线或先做独立文档复审。已完成的 T0.28–T0.34 验收标准不得擅自扩大，确需扩大须用户批准。保留已有状态、证据、失败记录及当前任务范围；任何任务都不得伪造验收。

本节优先于下文与之冲突的逐卡全量验证、提交、独立巡检和冻结修订流程；显式任务需求、用户决定以及必要的安全和数据完整性检查仍然有效。


当前顺序：T0.19 SuperVod JSON→T0.20多源身份/完整性→T0.21原生请求上下文→T0.22宿主及来源管理→T0.18设置/SQLite→T0.23配置事务→T0.24至T0.27桌面验收→T0.28至T0.34桌面Apple TV化改造→T0.35独立验收→T0.37线路记忆→T0.36交付→T0.38至T0.44本地视频、直播、设置重组、通用TMDB与MKV（ADR-065）→T0.45近似搜索→T0.47至T0.49独立播放服务（ADR-077/078，取代ADR-075/076规划的T0.46）。原脚本按文件顺序及依赖选卡；每卡独立验收。本次用户 2026-11-11 明确要求仅修订与复审文档、先不开发；播放服务三张卡等待新的开发指令，T0.47 设置阶段门。

播放服务 T0.47–T0.49 按个人娱乐应用验收（ADR-078）：一组直接相关自动检查加一次本机关键操作检查，可合并场景、复用已有检查。超时、缓存和并发值作为可调初值；长播、压力/容量、跨平台安装、完整故障矩阵和重复全量 verify 留待按需安排。保留找片/选集/选线/播放/续播、当前数据保留、基本鉴权与网络安全检查。仅调整这三张新卡，既有完成证据和其他卡保持其原范围；具体要求见方案第 10 节。

M系列按ADR-048改为本地Rust来源，保持后续独立授权阶段。桌面T25完成后外层监管停止并交付移动待办；M00执行前须明确移动授权。验收入口只做当卡行为断言，任务状态、冻结与完成资格沿原脚本。未完成卡脚本由当卡交付，当前业务验收NOT RUN；已有验收入口及完成证据保持，运行缺入口必然失败。

每卡验收公共前提/JSON报告/失败规则见ARCHITECTURE“验收与证据契约”。`all`覆盖下方当前义务及所有关联需求；依赖卡已实现后只调其真实公开入口。测试构建使用脱敏公开响应fixture和受控自有媒体；在线来源、WebView、macOS arm64安装和移动真机按卡明确实跑。首版仅个人使用，可用SuperVod JSON来源；其他六站及XML/特殊播放器解析暂缓。来源以公开行为独立实现，停止于验证码、Cloudflare或访问控制；TLS校验保持开启。macOS以外平台及缺失的发行签名凭据只在报告中列为NOT RUN/发行待办并给出准确argv，本卡其余必需项照常判定。拒绝用旧报告、零用例或缺目标通过。

验收脚本只创建并清理自己的fixture、随机端口、容器；同步/取消/异常均finally清理，deadline由卡定义。构建子进程复用父run.py资源范围、CARGO_BUILD_JOBS=2、测试线程1，不嵌套控制包装器。大验收跨会话累计且保留完整验收边界，开发前确认默认3小时×2会话能覆盖该卡，额度调整遵守人类授权和重审。

## 原卡号映射

| 原卡 | 工作流卡 | 名称 | 阶段 |
| --- | --- | --- | --- |
| T00 | T0.1 | 版本与环境基线 | 桌面 |
| T01 | T0.2 | 三平台 Tauri WebView 视频技术样机 | 桌面 |
| T05 | T0.3 | Rust宿主与统一命令契约 | 桌面 |
| N00 | T0.4 | MacCMS JSON来源 | 桌面 |
| N01 | T0.5 | Stremio HTTP来源 | 桌面 |
| T06 | T0.6 | 推荐首页数据适配 | 桌面 |
| T07 | T0.7 | 详情与集数规范化 | 桌面 |
| T08 | T0.8 | 跨源匹配 原始loadLinks与资源登记 | 桌面 |
| T09 | T0.9 | 通用媒体与图片网关 | 桌面 |
| T10 | T0.10 | HLS 代理保真 | 桌面 |
| T11 | T0.11 | 字幕与基线 DASH | 桌面 |
| T12 | T0.12 | 正式 WebView 播放控制 | 桌面 |
| T13 | T0.13 | 应用状态与导航 | 桌面 |
| T14 | T0.14 | 推荐首页界面 | 桌面 |
| T15 | T0.15 | 详情与选集界面 | 桌面 |
| T16 | T0.16 | 内嵌播放界面 | 桌面 |
| T17 | T0.17 | 源选择 引擎设置与受控交互 | 桌面 |
| T18 | T0.18 | 设置与全生命周期清理 | 桌面 |
| T02 | T0.19 | SuperVod JSON原生来源 | 桌面 |
| T03 | T0.20 | SuperVod JSON多源身份与协议完整性 | 桌面 |
| T04 | T0.21 | Rust来源请求上下文与故障隔离 | 桌面 |
| N02 | T0.22 | Rust来源接入宿主与来源管理 | 桌面 |
| T19 | T0.23 | 来源配置刷新与事务 | 桌面 |
| T20 | T0.24 | Rust来源与产品端到端 | 桌面 |
| T21 | T0.25 | 媒体网关与鉴权回归 | 桌面 |
| T22 | T0.26 | 三平台性能与故障注入 | 桌面 |
| T23 | T0.27 | 发行安装包 | 桌面 |
| TV1 | T0.28 | 桌面外壳、深色主题与焦点系统 | 桌面 |
| TV2 | T0.29 | 目录协议、TMDB筛选与默认来源修复 | 桌面 |
| TV3 | T0.30 | 首页与分类页 | 桌面 |
| TV4 | T0.31 | MacCMS分类与同源直通播放 | 桌面 |
| TV5 | T0.32 | 详情页、转场与待播清单 | 桌面 |
| TV6 | T0.33 | 沉浸播放器 | 桌面 |
| TV7 | T0.34 | 搜索、资料库、来源与设置 | 桌面 |
| T24 | T0.35 | 独立验收与问题修复 | 桌面 |
| LINE-PREFERENCE | T0.37 | 影片成功线路记忆 | 桌面 |
| X1 | T0.38 | 设置页重组与通用TMDB（Stremio方式） | 桌面 |
| X2 | T0.39 | 左侧菜单：本地视频与直播 | 桌面 |
| X3 | T0.40 | 直播（IPTV与直播源合并） | 桌面 |
| X4 | T0.41 | 通用TMDB：官方API方式 | 桌面 |
| X5 | T0.42 | 本地视频（mp4/mov/m4v） | 桌面 |
| X6 | T0.43 | MKV可行性实验（ffmpeg-next） | 桌面 |
| X7 | T0.44 | MKV支持 | 桌面 |
| X8 | T0.45 | 近似搜索 | 桌面 |
| SERVICE-1 | T0.47 | 播放服务一：元数据服务 | 桌面 |
| SERVICE-2 | T0.48 | 播放服务二：播放服务与App选线 | 桌面 |
| SERVICE-3 | T0.49 | 播放服务三：接口类来源 | 桌面 |
| T25 | T0.36 | 开发交付与最终说明 | 桌面 |
| M00 | T1.1 | Android / iOS 真机可行性与版本基线 | 后续移动 |
| M01 | T1.2 | Android本地Rust来源接入 | 后续移动 |
| M02 | T1.3 | 移动页面与播放器适配 | 后续移动 |
| M03 | T1.4 | Android 安装与发行验收 | 后续移动 |
| M04 | T1.5 | iOS本地Rust来源接入 | 后续移动 |
| M05 | T1.6 | iOS 真机闭环与移动最终交付 | 后续移动 |

## 需求覆盖

每行保留原文位置（章节取docs/SPEC.md，字典指replica-spec/DATA_STRUCTURES.md）；卡内反向需求指向这里。ADR-048逐项取代Android/原包义务；原文继续留在SPEC与replica-spec，REQ原卡号作为迁移主键。已完成卡保留其原验收语义，后续卡按本表及ADR-048验收。

| 需求 ID / 原文位置 | 可观察要求 | 卡 / 验收 ID | 范围或待决定 |
| --- | --- | --- | --- |
| U01 / §1.1 | Tauri/Rust、WebView、原生持久化 | T0.2/A1, T0.3/A1, T0.18/A1, T0.35/A1 | 按原规格完整保留 |
| U02 / §1.1,2.1 | 首版可用SuperVod JSON来源的搜索、详情、选集及解析 | T0.19/A1, T0.20/A1, T0.22/A1, T0.24/A1 | ADR-048取代给定索引与全部provider义务 |
| U03 / §1.1,8 | 推荐/详情/选集/播放完整真实闭环 | T0.14/A1, T0.15/A1, T0.16/A1, T0.24/A1 | 按原规格完整保留 |
| U04 / §1.1,5.1 | macOS arm64真实安装运行 | T0.2/A1, T0.21/A1, T0.24/A1, T0.27/A1（包级）, T0.35/A1, T0.36/A2 | ADR-005：macOS x86_64/Windows/Linux为非阻断记录项 |
| U05 / §1.1,2.2,6.3 | 依据公开行为独立实现的Rust来源，实际媒体闭环 | T0.19/A1, T0.20/A1, T0.22/A1, T0.24/A1 | ADR-048取代原7包ART/DEX兼容；其余六站暂缓 |
| U06 / §1.1,15 | 准确契约/依赖/错误与可复现交付 | T0.1/A1, T0.3/A1, T0.35/A1, T0.36/A1 | 按原规格完整保留 |
| U07 / §1.1,16 | 桌面公共入口/cfg/响应式、后续移动真机MG | T0.13/A1, T0.35/A1, T1.1/A1, T1.2/A1, T1.3/A1, T1.4/A1, T1.5/A1, T1.6/A1 | 按原规格完整保留 |
| U08 / §1.1,18 | Stremio元数据与Rust MacCMS播放来源独立选择、跨源匹配 | T0.4/A1, T0.5/A1, T0.8/A1, T0.22/A1, T0.24/A1 | ADR-048取代Cloudstream主播放 |
| U09 / §1.1,11.4 | 历史及SQLite跨重启/续播/失败完整性 | T0.13/A1, T0.16/A1, T0.18/A1, T0.24/A1 | 按原规格完整保留 |
| R01 / §1.2,1.3 | 独立恢复元数据/播放偏好，初版页面；来源可用后由用户选择 | T0.3/A1, T0.13/A1, T0.14/A1, T0.17/A1, T0.22/A1, T0.35/A1 | ADR-048移除准备Android默认流程 |
| R02 / §2.1 | 旧索引/制品只留历史；首版候选列表及实际支持状态可追溯 | T0.1/A1（历史）, T0.19/A1, T0.23/A1 | ADR-048移除插件下载/锁更新 |
| R03 / §2.2 | Rust宿主/公开JSON适配器运行，无Android环境前提 | T0.19/A1, T0.22/A1 | ADR-048取代原Application/加载器 |
| R04 / §2.3 | 每个核准JSON来源独立身份/revision、配置UUID稳定 | T0.20/A1, T0.23/A1 | ADR-048取代包→provider全集 |
| R05 / §2.4 | 原生媒体请求头/Referer与来源上下文保持、敏感字段留Rust | T0.7/A1, T0.8/A1, T0.21/A1, T0.25/A1 | ADR-048移除原插件interceptor语义 |
| R06 / §2.5,3,3.1 | Rust crate、类型分发、原生HTTP与共用媒体路径 | T0.3/A1, T0.4/A1, T0.5/A1, T0.9/A1, T0.22/A1, T0.47/A1 | ADR-048移除Android执行分支；ADR-077/078改为App/服务两个可执行crate加按需共享库crate |
| R07 / §3.2 | 核准Rust来源、内嵌视频、macOS arm64及真实数据交付 | T0.24/A1, T0.35/A1 | ADR-005/048；原包7/7与全集义务取代 |
| R08 / §4 | 复用已有前端、api/models/player单入口、模块/cfg及协议权威 | T0.1/A1, T0.3/A1, T0.12/A1, T0.13/A1 | 按原规格完整保留 |
| R09 / §5.1 | 目标系统/工具链与Rust来源实际资源测量 | T0.1/A1（历史）, T0.21/A1, T0.26/A1, T0.26/A2 | ADR-048移除AVD镜像及其预算 |
| R10 / §5.2 | Rust宿主先就绪，单来源失败与Stremio/其他来源隔离 | T0.3/A1, T0.21/A1, T0.22/A1, T0.24/A1 | ADR-048取代Android按需启动 |
| R11 / §5.3 | 仅清理本应用请求/会话/网关进程，保留外部资源 | T0.21/A1, T0.18/A1 | ADR-048移除serial/forward/AVD操作 |
| R12 / §5.4 | 网关cap独立随机鉴权、查询串脱敏、本地凭据留宿主 | T0.19/A1, T0.21/A1, T0.9/A1, T0.25/A1 | ADR-048移除ADB bootstrap，保留鉴权隔离 |
| R13 / §5.5 | 访问控制失败准确报告并停止，无规避/自动交互 | T0.21/A1, T0.22/A1 | ADR-048取代windowed引擎及Cookie同步 |
| R14 / §6.1 | 配置探测/发布事务，受影响来源失效、失败保持确认状态 | T0.23/A1 | ADR-048取代制品更新事务 |
| R15 / §6.2 | 实际JSON能力、四个公共网络槽、栏目index/部分失败 | T0.6/A1, T0.20/A1, T0.22/A1 | ADR-048取代原provider调用规则 |
| R16 / §6.3 | 逐候选公开JSON实测和支持状态；失败与未测单独记录 | T0.19/A1, T0.22/A1, T0.24/A1 | ADR-048取代全部实例/同包参考环境 |
| R17 / §7,7.1 | snake_case/可选字段/Rating与纯文本、完整DTO元数据 | T0.3/A1, T0.7/A1 | 按原规格完整保留 |
| R18 / §7.2 | 复用HTTP H向量、重复条目/集数组序号及不透明引用 | T0.20/A1, T0.6/A1, T0.7/A1 | ADR-048移除包注册键；HTTP身份兼容 |
| R19 / §7.3 | 所有错误code、请求deadline表、聚合提前1秒/传输2秒/4槽/连接空闲 | T0.3/A1, T0.6/A1, T0.8/A1, T0.9/A1, T0.5/A1, T0.25/A1 | 按原规格完整保留 |
| R20 / §8.1 | 窗口1200×800/最小960×640/360,390,768/双语/五色系×明暗与持久化 | T0.13/A1, T0.24/A1 | 按原规格完整保留 |
| R21 / §8.2 | 真实Stremio栏目/状态/4..7卡/懒图/去重/滚动/单栏失败重试 | T0.6/A1, T0.14/A1 | 按原规格完整保留 |
| R22 / §8.3 | 主详情与人工匹配选组/季/集、长简介和缺字段 | T0.7/A1, T0.8/A1, T0.15/A1 | 按原规格完整保留 |
| R23 / §8.4 | 内嵌实际管线控制/3秒隐藏/音量百分比/倍速/键盘/当前EOS下一集 | T0.12/A1, T0.16/A1 | 按原规格完整保留 |
| R24 / §8.5 | 元数据/播放源独立切换清理、诊断与健康分开且脱敏 | T0.13/A1, T0.17/A1, T0.22/A1 | 按原规格完整保留 |
| R25 / §9.1 | 匹配→resolve→Session、close/release分别幂等、旧清理保留新句柄 | T0.8/A1, T0.9/A1, T0.16/A1 | 按原规格完整保留 |
| R26 / §9.1,字典10 | Session独立强持LinkRecord、5分钟候选与活动/空闲租约 | T0.9/A1, T0.10/A1, T0.11/A1, T0.22/A1, T0.25/A1 | 按原规格完整保留 |
| R27 / §9.2 | 原生来源请求上下文保真、Range200/206/416、压缩头、64KiB流/8MiB清单 | T0.9/A1, T0.22/A1, T0.25/A1 | 按原规格完整保留 |
| R28 / §9.3 | HLS URI行/属性/key/map/音字幕/相对签名/序列与全部子资源 | T0.10/A1, T0.25/A1 | 按原规格完整保留 |
| R29 / §9.4 | 字幕结构化VTT、DASH条件性全基线模板/SIDX、DRM明确拒绝 | T0.11/A1, T0.22/A1, T0.25/A1 | 按原规格完整保留 |
| R30 / §10.1 | 系统WebView真实H264/AAC/HLS/MSE、平台依赖与安全传输 | T0.2/A1, T0.12/A1, T0.35/A1, T0.36/A2 | 按原规格完整保留 |
| R31 / §10.2 | 17invoke形状/权限/host_epoch/包装DTO/递归图片与事件卸载、旧cloudstream明确未支持 | T0.3/A1, T0.7/A1, T0.9/A1, T0.13/A1, T0.22/A1 | ADR-048保留线格式兼容，移除guest执行 |
| R32 / §10.3 | 一个video/player生命周期/250ms位置/旧epoch/自动播放提示/CORS/CSP | T0.2/A1, T0.12/A1, T0.16/A1, T0.35/A1 | 按原规格完整保留 |
| R33 / §10.4 | 锁文件、npm ci/type/build、Rustfmt/check/clippy/test与发行依赖 | T0.1/A1, T0.27/A1, T0.36/A1 | 按原规格完整保留 |
| R34 / §10.5 | 资源状态3秒只读/诊断序号、每尝试一次查询与一次过期恢复/失败呈现 | T0.9/A1, T0.12/A1, T0.14/A1, T0.25/A1 | 按原规格完整保留 |
| R35 / §11.1 | AppState/完整快照vs单栏/分页合并、index唯一、局部错误与世代 | T0.6/A1, T0.13/A1, T0.14/A1 | 按原规格完整保留 |
| R36 / §11.2 | 切来源/集/离页/宿主重启的请求取消、世代失效与清理 | T0.3/A1, T0.13/A1, T0.18/A1, T0.21/A1, T0.22/A1 | ADR-048移除guest重启分支 |
| R37 / §11.3 | 有限缓存/图片去重稳定URL/30分钟/4096 LRU与刷新一次 | T0.9/A1, T0.14/A1, T0.25/A1 | 按原规格完整保留 |
| R38 / §11.3 | Settingsv2/v1/原子损坏诊断/单写队列/未知提交重读/退出排空 | T0.3/A1, T0.18/A1 | 按原规格完整保留 |
| R39 / §11.3 | generation墓碑、sync_sequence、同锁提交/资源登记与取消临界区 | T0.3/A1, T0.4/A1, T0.5/A1, T0.18/A1 | 按原规格完整保留 |
| R40 / §11.4 | 历史页面分组/最近集/确认删除清空/当前集完成重播 | T0.13/A1, T0.18/A1, T0.24/A1 | 按原规格完整保留 |
| R41 / §11.4 | SQLite完整列/索引/事务迁移/参数SQL与长期身份/原来源定位 | T0.18/A1, T0.24/A1 | 按原规格完整保留 |
| R42 / §11.4 | 5秒目标+事件写入/单调序号/删除旧快照/时钟回拨/写失败与退出 | T0.16/A1, T0.18/A1, T0.24/A1 | 按原规格完整保留 |
| R43 / §11.4 | 重启原config/addon/video匹配→新Session/loadedmetadata seek与失败保留 | T0.18/A1, T0.24/A1 | 按原规格完整保留 |
| R44 / §12 | loopback/鉴权/请求响应opaque及图像限额/私有目录 | T0.3/A1, T0.9/A1, T0.23/A1, T0.25/A1 | ADR-048移除ZIP解压入口，保留安全与数据完整性 |
| R45 / §12 | 仅打包应用/依赖及核准来源配置，许可清单/个人使用说明 | T0.19/A1, T0.21/A1, T0.27/A1 | ADR-048移除插件执行/AVD隔离义务 |
| R46 / §12.1 | 全部HTTP入口DNS/peer绑定、5跳重定向/HTTPS降级/跨Origin头/环境代理 | T0.4/A1, T0.5/A1, T0.9/A1, T0.25/A1 | 按原规格完整保留 |
| R47 / §13 | 1.5秒壳/100ms UI/本地p95<100ms/60秒宿主冷启为记录目标，5次启动 | T0.26/A1, T0.26/A2 | ADR-044/048；无Android启动样本 |
| R48 / §13 | 1080p30 H264AAC10分钟、20Mbps3分钟、50切集无泄漏与DPI125/150/200 | T0.2/A1（功能）, T0.12/A1, T0.25/A1（功能）, T0.26/A1, T0.26/A2（网关耐久）, T0.35/A1（播放器耐久） | ADR-044放宽 |
| R49 / §14.1 | 公开行为fixture/自有媒体，测试替身与正式构建隔离 | T0.2/A1, T0.4/A1, T0.5/A1, T0.19/A1, T0.22/A1, T0.25/A1, T0.35/A1 | ADR-048取代原DEX网络替身 |
| R50 / §14.2,14.3 | G0-Core及Rust来源G2、G1/G3/G4/G5实际桌面闭环 | T0.1/A1（历史）, T0.2/A1, T0.22/A1, T0.24/A1, T0.25/A1, T0.35/A1, T0.36/A2 | ADR-048取消G0-CS、取代原包G2 |
| R51 / §15 | 先机器契约、真命令/日志/报告、复现/回归/数据及发布材料 | T0.3/A1, T0.35/A1, T0.36/A1 | 按原规格完整保留 |
| R52 / §16.1,16.2 | 桌面后移动分期与Rust/DTO/页面/cfg复用 | T0.13/A1, T1.1/A1, T1.6/A1 | 按原规格完整保留 |
| R53 / §16.3 | Android本地Rust来源、单应用、生命周期/权限隔离 | T1.1/A1, T1.2/A1, T1.3/A1, T1.4/A1 | ADR-048；移动阶段另获授权 |
| R54 / §16.4 | iOS本地Rust来源/网关，HTTPS校验、会话隔离及断网恢复 | T1.1/A1, T1.5/A1, T1.6/A1 | ADR-048取代远程引擎/配对部署；移动另获授权 |
| R55 / §16.5 | 44px/safearea/360,390,768/playsinline/返回/音频/后台/网络 | T1.3/A1, T1.6/A1 | 后续移动另获授权 |
| R56 / §16.6 | MG0..MG3真机、安装签名与生命周期最终交付 | T1.1/A1, T1.4/A1, T1.6/A1 | 后续移动另获授权 |
| R57 / §17 | 全部固定事实/工程决定/待实测追溯来源与原始证据 | T0.1/A1, T0.35/A1 | 按原规格完整保留 |
| R58 / §18.1 | 最多32配置/1..16manifest/匿名UUID/URLuserinfo-fragment与kind归属 | T0.3/A1, T0.4/A1, T0.5/A1, T0.17/A1 | 按原规格完整保留 |
| R59 / §18.2 | MacCMS完整参数/数字字符串/分组拆分/未知格式/分类与分页 | T0.4/A1, T0.6/A1, T0.7/A1, T0.8/A1 | 按原规格完整保留 |
| R60 / §18.3 | manifest完整事务/unpublished/编辑撤销/元数据资源失败warning与fallback | T0.5/A1, T0.6/A1, T0.7/A1 | 按原规格完整保留 |
| R61 / §18.3 | catalog/meta过滤/skip/路径前缀query/原video/movie条目/resolvefalse | T0.5/A1, T0.8/A1 | 按原规格完整保留 |
| R62 / §18.3.1 | 真实en/zh-CN扩展请求/字段覆盖回退/预算/语言缓存/乱序且播放保持 | T0.5/A1, T0.13/A1, T0.24/A1 | 按原规格完整保留 |
| R63 / §18.4 | host_epoch与来源revision/generation隔离、单源故障其他来源持续 | T0.3/A1, T0.9/A1, T0.21/A1, T0.22/A1, T0.24/A1 | ADR-048移除guest句柄映射 |
| R64 / §18.5 | HTTP H公式/revision/源编辑删除影响范围与独立最近源迁移 | T0.4/A1, T0.5/A1, T0.3/A1, T0.18/A1 | 按原规格完整保留 |
| R65 / §18.6 | MetadataIdentity全局映射需验证/自定义namespace/addon准确回源 | T0.8/A1, T0.5/A1, T0.18/A1 | 按原规格完整保留 |
| R66 / §18.6 | 最多2×10候选/外部ID自动确认条件/手动组集/无search/错误vs零匹配 | T0.8/A1, T0.22/A1, T0.24/A1 | 按原规格完整保留 |
| R67 / §18.6 | match最多256空闲5分与世代绑定、video/播放源乱序、下一集重新匹配 | T0.8/A1, T0.13/A1, T0.16/A1, T0.24/A1 | 按原规格完整保留 |
| C-PATH / §用户路径确认/ADR-001/C1 | 唯一根、venv/原脚本/driver与全局锁/原证据绑定 | T0.1/A1, T0.36/A1 | 按原规格完整保留 |
| C-RESOURCE / §moc-init/C14 | 预算全命令覆盖、真实集群Docker/容器硬限额合计+宿主RSS/实际采样及自产清理 | T0.1/A1, T0.25/A1, T0.26/A1, T0.26/A2, T1.5/A1 | 按原规格完整保留 |
| REQ-T00 / replica-spec/AI_TASKS.md T00 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.1/A1 | 桌面首版 |
| REQ-T01 / replica-spec/AI_TASKS.md T01 | 开发功能与产品耐久分期，原时长/规模/指标保持 | T0.2/A1（功能）, T0.35/A1（耐久） | 桌面首版 |
| REQ-T05 / replica-spec/AI_TASKS.md T05 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.3/A1 | 桌面首版 |
| REQ-N00 / replica-spec/AI_TASKS.md N00 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.4/A1 | 桌面首版 |
| REQ-N01 / replica-spec/AI_TASKS.md N01 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.5/A1 | 桌面首版 |
| REQ-T06 / replica-spec/AI_TASKS.md T06 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.6/A1 | 桌面首版 |
| REQ-T07 / replica-spec/AI_TASKS.md T07 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.7/A1 | 桌面首版 |
| REQ-T08 / replica-spec/AI_TASKS.md T08 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.8/A1 | 桌面首版 |
| REQ-T09 / replica-spec/AI_TASKS.md T09 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.9/A1 | 桌面首版 |
| REQ-T10 / replica-spec/AI_TASKS.md T10 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.10/A1 | 桌面首版 |
| REQ-T11 / replica-spec/AI_TASKS.md T11 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.11/A1 | 桌面首版 |
| REQ-T12 / replica-spec/AI_TASKS.md T12 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.12/A1 | 桌面首版 |
| REQ-T13 / replica-spec/AI_TASKS.md T13 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.13/A1 | 桌面首版 |
| REQ-T14 / replica-spec/AI_TASKS.md T14 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.14/A1 | 桌面首版 |
| REQ-T15 / replica-spec/AI_TASKS.md T15 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.15/A1 | 桌面首版 |
| REQ-T16 / replica-spec/AI_TASKS.md T16 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.16/A1 | 桌面首版 |
| REQ-T17 / replica-spec/AI_TASKS.md T17 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.17/A1 | 桌面首版 |
| REQ-T18 / replica-spec/AI_TASKS.md T18 | ADR-048修订后的T18工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.18/A1 | 桌面首版 |
| REQ-T02 / replica-spec/AI_TASKS.md T02 | ADR-048修订后的SuperVod JSON原生来源工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.19/A1 | 桌面首版 |
| REQ-T03 / replica-spec/AI_TASKS.md T03 | ADR-048修订后的SuperVod JSON多源身份与协议完整性工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.20/A1 | 桌面首版 |
| REQ-T04 / replica-spec/AI_TASKS.md T04 | ADR-048修订后的Rust来源请求上下文与故障隔离工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.21/A1 | 桌面首版 |
| REQ-N02 / replica-spec/AI_TASKS.md N02 | ADR-048修订后的Rust来源接入宿主与来源管理工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.22/A1 | 桌面首版 |
| REQ-T19 / replica-spec/AI_TASKS.md T19 | ADR-048修订后的来源配置刷新与事务工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.23/A1 | 桌面首版 |
| REQ-T20 / replica-spec/AI_TASKS.md T20 | ADR-048修订后的Rust来源与产品端到端工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.24/A1 | 桌面首版 |
| REQ-T21 / replica-spec/AI_TASKS.md T21 | ADR-048修订后的T21工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.25/A1（功能）, T0.26/A1（网关长测采样/协议准备）, T0.26/A2（完整10分钟网关耐久） | 桌面首版 |
| REQ-T22 / replica-spec/AI_TASKS.md T22 | ADR-048修订后的T22工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.26/A1, T0.26/A2 | 桌面首版 |
| REQ-T23 / replica-spec/AI_TASKS.md T23 | ADR-048修订后的T23工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.27/A1（包级，生命周期由REQ-T25-LIFECYCLE接续） | 桌面首版 |
| TV-SHELL / ADR-057 | 沉浸标题栏、悬浮可折叠侧栏、窄屏不变 | T0.28/A1 | 桌面Apple TV化 |
| TV-THEME / ADR-057 | 桌面固定深色、色系作强调色 | T0.28/A1 | 桌面Apple TV化 |
| TV-FOCUS / ADR-057 | 键盘空间导航、悬停与焦点一致、减弱动态 | T0.28/A1 | 桌面Apple TV化 |
| REQ-TV1 / ADR-057 | 桌面外壳、深色主题与焦点系统 | T0.28/A1 | 桌面Apple TV化 |
| TV-CATALOG-DTO / ADR-057 | 卡片与栏目协议扩展 | T0.29/A1 | 桌面Apple TV化 |
| TV-FILTER / ADR-057 | TMDB单条件与组合筛选、扫描上限、游标隔离 | T0.29/A1 | 桌面Apple TV化 |
| TV-SOURCE-UPGRADE / ADR-057 | 升级配置补TMDB一次 | T0.29/A1 | 桌面Apple TV化 |
| REQ-TV2 / ADR-057 | 目录协议、TMDB筛选与默认来源修复 | T0.29/A1 | 桌面Apple TV化 |
| TV-HOME / ADR-057 | Hero、继续观看、分类内容排、懒加载 | T0.30/A1 | 桌面Apple TV化 |
| TV-BROWSE / ADR-057 | 分类页、筛选栏、海报墙、状态恢复 | T0.30/A1 | 桌面Apple TV化 |
| REQ-TV3 / ADR-057 | 首页与分类页 | T0.30/A1 | 桌面Apple TV化 |
| TV-MACCMS-CAT / ADR-057 | 分类归一、排除项、客户端过滤 | T0.31/A1 | 桌面Apple TV化 |
| TV-MACCMS-PLAY / ADR-057 | MacCMS同源播放、历史与续播 | T0.31/A1 | 桌面Apple TV化 |
| REQ-TV4 / ADR-057 | MacCMS分类与同源直通播放 | T0.31/A1 | 桌面Apple TV化 |
| TV-DETAILS / ADR-057 | 全幅详情、续播信息、集卡、转场 | T0.32/A1 | 桌面Apple TV化 |
| TV-WATCHLIST / ADR-057 | SQLite待播清单与迁移 | T0.32/A1 | 桌面Apple TV化 |
| REQ-TV5 / ADR-057 | 详情页、转场与待播清单 | T0.32/A1 | 桌面Apple TV化 |
| TV-PLAYER / ADR-057 | 全窗口沉浸播放、自定义皮肤、托盘、下一集、切集回收 | T0.33/A1 | 桌面Apple TV化 |
| REQ-TV6 / ADR-057 | 沉浸播放器 | T0.33/A1 | 桌面Apple TV化 |
| TV-SEARCH / ADR-057 | 搜索页与请求取消 | T0.34/A1 | 桌面Apple TV化 |
| TV-LIBRARY / ADR-057 | 资料库三标签 | T0.34/A1 | 桌面Apple TV化 |
| TV-MANAGE / ADR-057 | 来源与设置分组列表 | T0.34/A1 | 桌面Apple TV化 |
| REQ-TV7 / ADR-057 | 搜索、资料库、来源与设置 | T0.34/A1 | 桌面Apple TV化 |
| REQ-T24 / replica-spec/AI_TASKS.md T24 | ADR-048修订后的T24工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.35/A1 | 桌面首版 |
| REQ-T25 / replica-spec/AI_TASKS.md T25 | ADR-048修订后的T25工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.36/A1 | 桌面首版 |
| REQ-T25-LIFECYCLE / ADR-058 | 干净真机或专用虚拟机上的安装、初始化、首播、升级、卸载 | T0.36/A2 | 桌面首版 |
| REQ-LINE-PREFERENCE / 用户当前聊天、ADR-061 | 首集成功线路用于后续集；用户明确要求重启持久化，无可靠匹配保留确认流程 | T0.37/A1 | 桌面首版 |
| REQ-X01 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | 左侧菜单「本地视频」默认显示，可播放本地文件 | T0.39/A1, T0.42/A1 | 桌面 |
| REQ-X02 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | IPTV与直播源合并为「直播」菜单，设置中配置后才显示 | T0.39/A1, T0.40/A1 | 桌面 |
| REQ-X03 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | 来源管理并入设置的播放设置 | T0.38/A1 | 桌面 |
| REQ-X04 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | 可添加的播放源与HTTP来源合并为一个列表 | T0.38/A1 | 桌面 |
| REQ-X05 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | TMDB配置移入语言与元数据，做成通用TMDB源 | T0.38/A1 | 桌面 |
| REQ-X06 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | 通用TMDB源同时支持TMDB官方API与Stremio TMDB扩展 | T0.38/A1, T0.41/A1 | 桌面 |
| REQ-X07 / 用户2026-11-11聊天、ADR-065、docs/FOLLOWUP-LOCAL-LIVE-TMDB.md | 用ffmpeg-next在程序内支持MKV，不用ffmpeg.wasm或外置ffmpeg可执行文件 | T0.43/A1, T0.44/A1 | 桌面 |
| REQ-X08 / 用户2026-11-11聊天（「搜索是否能近似搜索」「Implement it」） | MacCMS 搜索规范化与逐步放宽、TMDB 片名兜底、按接近程度排序、相近结果提示 | T0.45/A1 | 桌面 |
| REQ-SERVICE-META / 用户2026-11-11聊天（服务提供TMDB/Stremio元数据、每天更新主页排名与分类、代理TMDB；按审查意见修订并简化验收）、ADR-077/078 | 按需共享crate、持久令牌、本机/局域网接入、TMDB和自定义Stremio元数据、首次及每日首页、缓存、语言路由、当前历史/收藏与来源身份保留 | T0.47/A1, T0.47/A2 | 桌面；等开发指令 |
| REQ-SERVICE-PLAY / 用户2026-11-11聊天（多源自动/手动选线、元数据解耦、应用语言搜索与两级筛选、缓存可用源；按审查意见修订并简化验收）、ADR-075/076/077/078 | MacCMS JSON/XML、正确匹配与确认/撤销、来源与候选/线路生命周期、缓存/刷新/统计；App查询和测速均结束后选线、换线续播、旧轮次隔离、来源导入及移除全局播放源与MacCMS元数据 | T0.48/A1, T0.48/A2 | 桌面；依赖第一期 |
| REQ-SERVICE-PROVIDERS / 用户2026-11-11聊天（zyfun/TVBox/Stremio接口形式扩展；按审查意见修订并简化验收）、ADR-077/078 | T4查询式、DRPYJS0、T4 REST式、TVBox type 4、Stremio播放扩展的ID与能力适配、json解析；HTTP媒体支持状态 | T0.49/A1 | 桌面；向其他Stremio客户端输出stream为第四期可选 |
| REQ-M00 / replica-spec/AI_TASKS.md M00 | ADR-048修订后的M00工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.1/A1 | 后续移动 |
| REQ-M01 / replica-spec/AI_TASKS.md M01 | ADR-048修订后的Android本地Rust来源接入工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.2/A1 | 后续移动 |
| REQ-M02 / replica-spec/AI_TASKS.md M02 | ADR-048修订后的M02工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.3/A1 | 后续移动 |
| REQ-M03 / replica-spec/AI_TASKS.md M03 | ADR-048修订后的M03工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.4/A1 | 后续移动 |
| REQ-M04 / replica-spec/AI_TASKS.md M04 | ADR-048修订后的iOS本地Rust来源接入工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.5/A1 | 后续移动 |
| REQ-M05 / replica-spec/AI_TASKS.md M05 | ADR-048修订后的M05工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.6/A1 | 后续移动 |

## 任务卡

## 开发与产品测试分期（当前用户决定）

开发阶段以功能实现为中心：每卡实际完成相关构建/类型检查、功能行为与正负例回归、必要的短时真实集成验证，验证通过后交接，并沿原完成提交与独立巡检流程收尾。长时间连续播放及本次列出的窗口耐久验收集中到产品测试阶段；原时长、规模、指标、资源与清理要求保持。其他开发卡现有的短时功能循环、错误/边界反例及即时资源清理继续按卡执行。

T0.2的MP4/HLS耐久及窗口开关移交T0.35/A1，按ADR-044为各600秒及5次；T0.25的网关播放由T0.26/A2在真实产品链路完成，按ADR-044为10分钟。T0.26为产品性能/故障测试，T0.35为产品独立验收与修复，保留原文件顺序及依赖。原始义务原文作为来源保留，执行阶段按本条及ADR-020、ADR-044解释；后续产品测试按当时稳定源码和构建重新实测。短时功能通过与长期验收待测分别记录。


已完成卡“原始义务”保留历史原文；ADR-048迁移卡以“修订义务”为当前执行要求，原文可在replica-spec/AI_TASKS.md查阅。原卡号继续用于验收脚本和报告路径，状态统一到docs/PROGRESS.md。

### T0.1 T00 版本与环境基线
- [x] 完成
- 依赖：无
- 需求：U02, U06, R02, R08, R09, R33, R50, R57, C-PATH, C-RESOURCE, REQ-T00 → A1（覆盖表含精确对应）
- 范围：`docs/runtime.lock.json`, `docs/PROGRESS.md`, `rust-toolchain.toml`, `package-lock.json`, `src-tauri/Cargo.lock`, `android-engine/`, `licenses/`, `tests/`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t00.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "环境表逐目标写可操作机器/CI/blocked与准确复现命令；Core实际工具version及最小构建；原7包长度/SHA/manifest静态7/7；候选与实测锁分列，许可证清单；核对C1实际根、venv、driver/global锁及源码基线。平台缺失仅记录blocked，原包少包/错误摘要/未知工具伪填已验证时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t00.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T00-all.json",
      "--junit",
      "evidence/acceptance/T00-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T00-all.xml",
    "cases": [
      "U02",
      "U06",
      "R02",
      "R08",
      "R09",
      "R33",
      "R50",
      "R57",
      "C-PATH",
      "C-RESOURCE",
      "REQ-T00"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 原始义务（完整原卡）

- 对应：U01/U02/U04/U05，G0。
- 输入：全部文档、references/plugin-artifact-inventory.json、固定上游源码。
- 修改位置：application/docs/runtime.lock.json、rust-toolchain.toml、src-tauri/Cargo.lock、package-lock.json、Android依赖锁与docs/progress.md。
- 工作：记录Rust/Tauri/Vue/TypeScript/Vite/hls.js/Node/npm、Cloudstream候选提交、JDK/Gradle、emulator/tools、API35镜像revision及宿主架构。核验7包摘要并保留manifest；复核依赖许可证及发行材料。
- 通过：Core的Rust/Node/WebView工具版本、获取方式和最小构建命令明确；Android工具/镜像在同一清单单独记录计划版本与待验证状态，由T02/T04补实测；原包7/7身份静态核对。T00的Core完成条件按本句验收，Cloudstream分支独立记录，HTTP任务可继续。
- 验证：三平台环境检测记录；JSON解析与字段校验；官方工具version输出。此卡将验证计划、可用机器和实际运行结果分别记录。
- 停止条件：缺少Rust/WebView前提时暂停Core分支；SDK镜像、虚拟化或ABI前提缺失时只暂停Cloudstream分支，继续HTTP任务。

### T0.2 T01 三平台 Tauri WebView 视频技术样机
- [x] 完成
- 依赖：T0.1
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：U01, U04, R30, R32, R48, R49, R50, REQ-T01 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `src/player.ts`, `src/views/`, `tests/fixtures/`, `packaging/`, `tests/`, `docs/PROGRESS.md`, `package.json`, `package-lock.json`, `src/main.ts`, `src/App.vue`, `vite.config.ts`, `index.html`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t01.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "开发功能验收：本机macOS arm64真实Tauri WebView自有1080p30 MP4/HLS短时功能播放（复用现有open/controls/stop，以真实功能状态读回通过并沿原命令超时）；验证帧/音频/UI进度、暂停/seek/字幕/音量/倍速/全屏、5种布局与4种缩放、Origin/cap/CORS/CSP/Range、生产排除及正常stop/关闭零资源残留，实际相关回归和构建通过。其他平台沿ADR-005/006列NOT RUN。两格式各1800秒与20次窗口开关由T0.28/A1按原强度在产品测试完成，开发报告明确该后续义务。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t01.py",
      "--suite",
      "functional",
      "--report",
      "evidence/acceptance/T01-functional.json",
      "--junit",
      "evidence/acceptance/T01-functional.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T01-functional.xml",
    "cases": [
      "U01",
      "U04",
      "R30",
      "R32",
      "R48",
      "R49",
      "R50",
      "REQ-T01"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T00；对应U01/U04，G1。
- 修改：src-tauri/src/lib.rs与main.rs、src/player.ts、最小样机Vue页、Tauri权限/CSP、packaging依赖说明。
- 工作：建立Tauri窗口，video原生/HLS.js能力检测，播放自有MP4/HLS；验证资源cap、CORS、CSP、loopback网络策略、codec、字幕、音频、暂停、seek、DPI与清理；检查360/390宽度重排。此时Android网关尚未存在：在tests/fixtures下实现最小宿主loopback测试服务器（随机端口、`?cap=`校验、固定Origin CORS、Range），只用于样机与后续回归，禁止进入正式构建。
- 通过：macOS两种目标架构、Windows x86_64、Ubuntu X11/Wayland有实际样机记录；播放界面使用Tauri WebView内嵌区域；主线程保持响应。
- 验证：自有1080p30片段、30分钟播放、20次打开/关闭、窗口缩放与全屏，记录帧表现和资源。
- 停止条件：某平台出现渲染/音频/依赖问题，提交最小复现并修复；禁止先堆业务UI掩盖技术门失败。

### T0.3 T05 Rust宿主与统一命令契约
- [x] 完成
- 依赖：T0.1
- 需求：U01, U06, R01, R06, R08, R10, R17, R19, R31, R36, R38, R39, R44, R51, R58, R63, R64, REQ-T05 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `src/api.ts`, `src/models.ts`, `protocol/`, `tests/`, `docs/PROGRESS.md`, `package.json`, `package-lock.json`, `src/main.ts`, `src/App.vue`, `vite.config.ts`, `index.html`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t05.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "实际protocol/bridge.openapi.json、app.schema.json、examples/contract-cases.json均存在且由完整verify检查结构/引用/正负例/语义；既有17命令与归档兼容、拒绝工作协议缺失或漂移；17命令参数/包络与权限负例；HTTP-only无Android启动、get_sources、Settings v1/v2、原子失败与未知版本；单写队列/未知提交重读/世代墓碑/同步序号乱序；UUID重复/自身取消/父子级联；固定预算表可控时钟与旧结果无副作用。空ready、假成功、旧提交覆盖或字段漂移时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t05.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T05-all.json",
      "--junit",
      "evidence/acceptance/T05-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T05-all.xml",
    "cases": [
      "U01",
      "U06",
      "R01",
      "R06",
      "R08",
      "R10",
      "R17",
      "R19",
      "R31",
      "R36",
      "R38",
      "R39",
      "R44",
      "R51",
      "R58",
      "R63",
      "R64",
      "REQ-T05"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T00。
- 修改：SourceDispatch、宿主状态/设置、commands.rs/api.ts、DTO与契约测试。
- 工作：初始化host_epoch与Rust来源注册表，定义三类enum路由；实现runtime_status、默认application准备、get_sources、取消、Settings v2读写及v1迁移。定义17个命令，HTTP业务后续接入，Cloudstream分支由N02连接原Bridge。
- 通过：无SDK/ADB/AVD环境也可准备宿主、保存HTTP配置、枚举空/已配置来源、取消本机任务；宿主与guest_epoch有独立字段，契约fixture与权限负例通过。
- 验证：匿名配置、重复config_id、迁移、禁止URL形态、原子保存及失败、设置队列交错、旧generation晚到/删除再添加、同世代同步乱序、自身取消与重复活动UUID、父子级联取消、未知来源与UUID/状态规则。

| 命令/接口 | 行为验收任务 |
|---|---|
| Android bootstrap、GET /v1/health | T02；T04验证客体启动链 |
| runtime_status、application准备、get_sources、cancel_request、get/save_settings | T05；配置故障与退出清理由T18扩展 |
| MacCMS/Stremio协议和来源同步 | N00/N01 |
| get_home / get_details / match_playback / resolve_episode | T06 / T07 / T08 / T08 |
| open_playback、close_playback、release_resolution、get_resource_status及Rust资源服务 | T09；播放器接入T12 |
| Cloudstream准备、桥接路由、start/finish_interaction | N02；UI入口与状态fixture由T17提供 |
| Cloudstream制品刷新事务 | T19 |

### T0.4 N00 MacCMS JSON来源
- [x] 完成
- 依赖：T0.3
- 需求：U08, R06, R39, R46, R49, R58, R59, R64, REQ-N00 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/host.rs`, `src-tauri/src/maccms.rs`, `src-tauri/src/source_dispatch.rs`, `src-tauri/src/commands.rs`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/n00.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "受控MacCMS JSON各协议向量、URL参数一次编码、code/closed/数字字符串、分类/详情/search、多线路$与#语义/缺集/歧义/HTML/未知媒体、类型有界探测；配置禁用删除/修订、取消与SSRF peer/DNS/重定向/代理负例。错误被吞为空成功或媒体引用串线时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/n00.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/N00-all.json",
      "--junit",
      "evidence/acceptance/N00-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/N00-all.xml",
    "cases": [
      "U08",
      "R06",
      "R39",
      "R46",
      "R49",
      "R58",
      "R59",
      "R64",
      "REQ-N00"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T05。
- 对应：U08；修改src-tauri/src/maccms.rs、来源配置、HTTP fixture与身份向量。
- 工作：按开发规格18.2实现匿名JSON分类/分页/详情/搜索能力；播放分组与集数拆分；URL/headers保留原语义，生成规范引用；同步不启动Android。
- 通过：一个受控MacCMS JSON服务的完整数据映射正确；多组、多集、空结果、关闭接口、错误code、非JSON、歧义分组有准确结果；配置ID稳定，修订变化可失效缓存。
- 验证：协议向量、非数字页码、Unicode、URL中$及#编码、HTML播放页、超限、取消、重定向目标、禁用/删除；真实源闭环在T20记录。

### T0.5 N01 Stremio HTTP来源
- [x] 完成
- 依赖：T0.3, T0.4
- 需求：U08, R06, R19, R39, R46, R49, R58, R60, R61, R62, R64, R65, REQ-N01 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/host.rs`, `src-tauri/src/stremio.rs`, `src-tauri/src/source_dispatch.rs`, `src-tauri/src/commands.rs`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/n01.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "完整manifest事务（首次部分失败unpublished/旧快照刷新失败/编辑撤销）、字符串对象资源过滤与required extras/skip去重/前缀query/顺序/相同标题、meta按归属回退与warnings、电影无videos、inline/stream忽略、45秒聚合取消；真实所选扩展版本en与zh-CN请求/响应证据、字段回退/缓存隔离/中英中竞态。丢配置路径/误有resolve/陈旧语言覆盖时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/n01.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/N01-all.json",
      "--junit",
      "evidence/acceptance/N01-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/N01-all.xml",
    "cases": [
      "U08",
      "R06",
      "R19",
      "R39",
      "R46",
      "R49",
      "R58",
      "R60",
      "R61",
      "R62",
      "R64",
      "R65",
      "REQ-N01"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T05/N00。
- 对应：U08；修改src-tauri/src/stremio.rs、manifest/资源HTTP及fixture。
- 工作：一个配置组合多个manifest；保留配置路径前缀，catalog分页与meta/videos元数据映射；资源字符串与对象过滤，stream及inline streams保持忽略，resolve能力固定false。
- 通过：catalog/meta扩展组成元数据模型；stream-only配置显示缺少元数据能力；同名catalog独立，原video_id准确，按配置顺序展示；元数据部分失败与全部失败按规范处理。
- 验证：types/idPrefixes、required extras、skip/空页、扩展顺序、对象资源、无videos电影、inline streams与infoHash/ytId字段被忽略、不可信元数据地址、Cookie/headers、取消；manifest首次部分失败/刷新失败保留旧快照/配置编辑撤销、meta回退与部分失败warnings、catalog/meta配置可自动成为元数据源，其inline/stream字段不进入播放管线、45秒聚合预算与取消；协议向量保持合成标记。

- 语言补充：按18.3.1验证选定Stremio metadata扩展的en/zh-CN确切配置及请求，保留英文基础ID/引用，本地化同ID展示字段；覆盖中文缺失/失败回退、ja/fr映射en、分语言缓存和中→英→中乱序；记录真实元数据证据。

### T0.6 T06 推荐首页数据适配
- [x] 完成
- 依赖：T0.3, T0.4, T0.5
- 需求：R15, R18, R19, R21, R35, R59, R60, REQ-T06 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `src/models.ts`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t06.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "HTTP原生和共用三类合成home向量：单/多/重复/动态栏目、源顺序index、分页/空/全错/部分错/全量与单栏重试、budget预留与顺序provider；核对section identity向量及A成功B失败C成功索引完整。排序/失败范围/翻页引用错误时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t06.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T06-all.json",
      "--junit",
      "evidence/acceptance/T06-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T06-all.xml",
    "cases": [
      "R15",
      "R18",
      "R19",
      "R21",
      "R35",
      "R59",
      "R60",
      "REQ-T06"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T05/N00/N01。
- 修改：Rust统一home分发、HTTP映射、HomePage DTO与fixture；Cloudstream真实桥接由N02接入。
- 工作：HTTP来源按第18章取分类/catalog并映射栏目；Cloudstream映射getMainPage，保留动态mainPage与一次多栏；建立当前epoch/revision的section_ref和page；保留源给出顺序并约束has_next/next_page；sequential源串行。按开发规格7.3按来源选择单栏目/整页预算执行，全量首页的栏目失败写入failed_sections，指定单栏失败走错误包络。
- 通过：源首页栏目与数据真实映射；翻页使用正确原request数据；HTTP动态栏目及共用栏目fixture正常；全量/单栏重试/翻页范围与index保持；空/错分别返回。
- 验证：单栏、多栏、重复标题、动态栏目、page2、空页、部分栏目失败、全部栏目失败、顺序源；section_id使用identity_example向量；三类共用DTO的合成映射与聚合超时、A成功/B失败/C成功的混合顺序、单栏首屏重试保留其他栏目；真实7包provider测试在N02执行。

### T0.7 T07 详情与集数规范化
- [x] 完成
- 依赖：T0.3, T0.4, T0.5
- 需求：R05, R17, R18, R22, R31, R59, R60, REQ-T07 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `src/models.ts`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t07.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "HTTP详情Movie/Series/Anime、多季/线路/配音、JSON opaque原串、字段空值/评分/单位/中文、200集及10/2原始顺序、全部图片递归登记；源和修订归属、详情晚到负例。伪造缺失字段/改写引用/丢分组时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t07.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T07-all.json",
      "--junit",
      "evidence/acceptance/T07-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T07-all.xml",
    "cases": [
      "R05",
      "R17",
      "R18",
      "R22",
      "R31",
      "R59",
      "R60",
      "REQ-T07"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T05/N00/N01。
- 修改：统一details分发、MacCMS/Stremio映射、Details/Episode DTO；Cloudstream真实桥接由N02接入。
- 工作：统一三类详情/集数映射；Cloudstream调用load(opaque)，HTTP来源保留站点/扩展身份；输出Movie/Series/Anime与详情rating/actors/directors/regions/status/update_label等；按实际可用元数据与空值规则输出；保留分组kind/season/dub、Episode.index及原data；注册详情/相关卡/演员/单集图片。
- 通过：URL型与JSON型ref均保持原样；集数10/2顺序正确；无季/无年份/无图正确显示；多个线路分组完整。
- 验证：HTTP多季、opaque JSON合成引用、单片、中文特殊字符、超长列表及无可播条目；真实Ddys/SuperVod在N02执行。

### T0.8 T08 跨源匹配 原始loadLinks与资源登记
- [x] 完成
- 依赖：T0.7
- 需求：U08, R05, R19, R22, R25, R59, R61, R65, R66, R67, REQ-T08 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `src/api.ts`, `src/models.ts`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t08.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "受控MacCMS真实匹配与HTTP解析、共用Cloudstream接口fixture：同名重拍/别名/未知年份/外部ID唯一与歧义、季0/小数/跨季/分组/缺search与手动home/details/人工selection归属；2×10最多20候选/45秒、match最多256与5分钟、错误和零匹配区别；多回调/字幕/空link/false/异常/晚到/取消/释放所有权、原episode_ref、Stremio resolve拒绝。误自动匹配或伪绑定时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t08.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T08-all.json",
      "--junit",
      "evidence/acceptance/T08-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T08-all.xml",
    "cases": [
      "U08",
      "R05",
      "R19",
      "R22",
      "R25",
      "R59",
      "R61",
      "R65",
      "R66",
      "R67",
      "REQ-T08"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T07。
- 修改：matching.rs、match_playback应用命令、Rust resolve分发与宿主ResolutionStore、HTTP候选登记、共用DTO；Cloudstream关联在N02验证。
- 工作：先按18.6实现Stremio→选定provider匹配、手动确认、match_id绑定，再解析MacCMS候选并生成宿主句柄；Cloudstream侧执行loadLinks(...,isCasting=false)，收集回调；保留原link与provider归属、headers、referer、extractor data和interceptor。
- 通过：Resolution.episode_ref与原始episode data一致；候选顺序和名称稳定；空候选/异常/取消准确；资源按source_revision隔离；候选表可显式释放，释放后旧stream_id查找返回过期结果；引用保有契约通过合成所有权向量验证；实际Session行为由T09验收，桥接DELETE真实路径在N02验收。
- 验证：多回调、字幕、未知quality、false返回、零link（UPSTREAM_FAILED/no_links）、全部不可播放、异常、回调晚到、版本变化与45秒超时。

- 匹配验收：同名重拍/别名/年份缺失、外部ID确认、跨季/特别季/小数集、无search能力、人工选集、乱序/取消/修订变化、match_id过期，以及Stremio媒体resolve被拒绝。受控MacCMS先验收，Cloudstream真实行为在N02验收。

### T0.9 T09 通用媒体与图片网关
- [x] 完成
- 依赖：T0.8
- 需求：R06, R12, R19, R25, R26, R27, R31, R34, R37, R44, R46, R63, REQ-T09 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t09.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "Rust真实gateway受控Header/Cookie/Referer、Range200/206/416与If-Range、gzip/重定向/HTML/慢响应/断开、有界流、图像字节/像素限制；256位cap/Bearer/CORS/CSP隔离与上游无本地cap、SSRF实际peer绑定/混合DNS/私网跳转；可控时钟活动超5分钟与Resolution释放后Session独立、旧close/release、get_resource_status只读/序号诊断。会话串线、泄漏、错误状态与预填cap时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t09.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T09-all.json",
      "--junit",
      "evidence/acceptance/T09-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T09-all.xml",
    "cases": [
      "R06",
      "R12",
      "R19",
      "R25",
      "R26",
      "R27",
      "R31",
      "R34",
      "R37",
      "R44",
      "R46",
      "R63",
      "REQ-T09"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T08，G3基础。
- 修改：Rust MediaGateway/ImageGateway及宿主会话记录、HTTP fixtures；Android已有网关由N02复用。
- 工作：创建带resolution/stream/source_revision/episode_ref/stream_kind绑定的PlaybackSession与media_capability；ImageRef单资源cap/到期；CORS与查询串脱敏；资源ID映射；HTTP来源在Rust来源上下文执行，Cloudstream保留原provider interceptor；流式返回；转发Range、处理重定向、修正响应头、取消上游。
- 通过：HTTP fixture要求的Cookie/Referer真正参与；本地令牌留在网关；200/206/416语义正确；大流保持有界内存。
- 验证：Header/Cookie要求、Range/If-Range、gzip、重定向、断开、慢响应、HTML错误、超限图片；session410与取消、同集换线路保留候选、旧close/release晚到保持新会话/新解析、无Session离页显式释放候选、使用可控时钟验证活动流超过5分钟及候选表回收后的独立请求上下文读取、get_resource_status的ready/expired/failed与世代转换、实际连接公网校验/私网重定向/DNS重绑定。

### T0.10 T10 HLS 代理保真
- [x] 完成
- 依赖：T0.9
- 需求：R26, R28, REQ-T10 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t10.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实HTTP网关HLS主/媒体/音轨/字幕/key/IV/map/byterange/相对和签名/重定向基址/序列与低延迟未知tag组合播放；日志核对全部子请求同cap与来源处理后重写；活动超5分钟和释放候选后清单/key/segment继续。裸远程子URL、拦截器顺序/压缩头错误时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t10.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T10-all.json",
      "--junit",
      "evidence/acceptance/T10-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T10-all.xml",
    "cases": [
      "R26",
      "R28",
      "REQ-T10"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T09，G3。
- 修改：HLS清单解析/重写、resource登记、测试素材。
- 工作：主/媒体清单、音轨、字幕、key、init、相对/绝对URI、byterange、重定向基址、序列/连续性；HTTP先执行所属来源响应处理再重写，Cloudstream原interceptor顺序在N02验证。
- 通过：所有下游请求继续经过同一网关及原源上下文；AES-128、fMP4、相对URL和查询签名正确；无任意URL开放代理。
- 验证：每项独立fixture+组合播放；HTTP清单变换与重写顺序fixture；真实Yjys特殊拦截器在N02回归；请求日志证明key/segment也走代理；可控时钟跨5分钟与释放Resolution后继续刷新HLS/读取Key/分片。

### T0.11 T11 字幕与基线 DASH
- [x] 完成
- 依赖：T0.9
- 需求：R26, R29, REQ-T11 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `src/player.ts`, `package.json`, `package-lock.json`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t11.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实字幕VTT/SRT/ASS结构化转换时间轴/换行/基础文本与Cookie/头、切换清理及Resolution回收后加载；基线清晰DASH存在时验证BaseURL/List/Base/SIDX与Number/Time/RepresentationID/Bandwidth受约束模板、seek/query与非法变量；DRM明确错误。基线未出现DASH单列NOT RUN并在N02补查，出现后缺测失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t11.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T11-all.json",
      "--junit",
      "evidence/acceptance/T11-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T11-all.xml",
    "cases": [
      "R26",
      "R29",
      "REQ-T11"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T09。
- 修改：字幕resource、播放器字幕接入、必要的MPD结构化处理。
- 工作：VTT轨道，SRT/ASS结构化转换为VTT基础文本与鉴权；若基线输出DASH，处理BaseURL、SegmentList、SegmentBase、受约束模板展开。
- 通过：字幕可开关和切换，旧会话释放；基线DASH样本可播放/seek；DRM资源给出准确类型错误。
- 验证：字幕Header/Cookie、时间轴、切集、query、模板Number/Time/RepresentationID、非法模板参数；Resolution回收后字幕继续加载。
- 记录：基线DASH是否实际出现及其制品证据；未出现的类型标记为未验证。

### T0.12 T12 正式 WebView 播放控制
- [x] 完成
- 依赖：T0.2, T0.10, T0.11
- 需求：R08, R23, R30, R32, R34, R48, REQ-T12 → A1（覆盖表含精确对应）
- 范围：`src/player.ts`, `src/views/PlaybackView.vue`, `src/App.vue`, `tests/`, `docs/PROGRESS.md`, `src/api.ts`, `src/state.ts`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t12.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "正式player.ts连接真实PlaybackViewSession，单video/native-HLS/hls.js能力选择、PlayerEvent映射、autoplay、unknown duration、旧epoch与250ms合并、50次切集资源销毁；native video/img/track错误查询一次，expired一次恢复/401/502/codec分类、旧查询丢弃。重复播放器/旧音频/误读浏览器message为HTTP码时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t12.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T12-all.json",
      "--junit",
      "evidence/acceptance/T12-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T12-all.xml",
    "cases": [
      "R08",
      "R23",
      "R30",
      "R32",
      "R34",
      "R48",
      "REQ-T12"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T01/T10，T11按媒体基线接入。
- 修改：src/player.ts、PlaybackView.vue与播放行为测试。
- 工作：使用PlaybackViewSession的stream_kind/URL及episode绑定，按PlayerEvent/PlaybackState契约接入video/HLS事件、准备/缓冲/位置/EOS/错误、autoplay交互、资源清理与playback_epoch。
- 通过：位置事件合理合并；切流销毁HLS/track/监听并释放音频与HTTP；旧epoch事件丢弃；duration unknown可正确呈现；原生video/img/track的错误经状态查询按10.5恢复。
- 验证：50次切集、暂停seek、EOS、源失联、会话过期、图片LRU过期、401/502/解码失败区分、查询失败/旧查询晚到、CPU/内存与媒体资源回收。

### T0.13 T13 应用状态与导航
- [x] 完成
- 依赖：T0.3
- 需求：U07, U09, R01, R08, R20, R24, R31, R35, R36, R40, R52, R62, R67, REQ-T13 → A1（覆盖表含精确对应）
- 范围：`src/`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t13.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "沿用UiProvider/组件/i18n/history视图；AppState及Route/PlaybackContext/Resolution/Session交叉负例、返回/快切/主客epoch、移动lib入口和cfg；全量/单栏重试/分页/图片映射按index合并；英文默认/中文切换、五色系×明暗×双语/390px、系统动态切换与首帧、lang/title/a11y/存储失败、会话内历史交互。双来源混淆/旧响应或偏好丢失时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t13.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T13-all.json",
      "--junit",
      "evidence/acceptance/T13-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T13-all.xml",
    "cases": [
      "U07",
      "U09",
      "R01",
      "R08",
      "R20",
      "R24",
      "R31",
      "R35",
      "R36",
      "R40",
      "R52",
      "R62",
      "R67",
      "REQ-T13"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T05。
- 修改：src/state.ts、App.vue、api.ts、导航测试及src-tauri公共入口。
- 工作：app.schema.json定义的Vue AppState/Route/SourcesState/页面状态/PlaybackContext/PlaybackState.resolution、source/request/playback epoch、返回恢复、源选择状态；首页分页合并按开发规格11.1；Rust管理环境与会话，桌面模块cfg隔离。
- 通过：快切A/B、源切换、返回及引擎重启的状态一致；渲染过程保持无阻塞副作用，移动公共入口可检查。
- 验证：乱序响应、路由/上下文/详情集数/Resolution/Session绑定及非法候选负例、模态焦点、快捷键上下文、最小窗口和高DPI。

- 语言补充：按开发规格18.3.1建立英文基础文案与中文词典、顶部/设置页切换；ui_locale缺省en并经Settings队列持久化。验证中英文路由/筛选/选集稳定、文档lang/title/无障碍名称、存储失败、390px重排及播放中切换。

- 外观补充：基础UI采用Naive UI 2.45.3，UiProvider为唯一库配置；useOsTheme响应系统明暗，内置lightTheme/darkTheme及themeOverrides统一页面与控件。用户选择五种color_palette之一，缺省lime，按Settings队列持久化；首帧恢复色系，旧ui_theme仅兼容且忽略。验证五色系×明暗/双语/390px、系统运行中切换、导航/表单/错误/弹层/海报/播放器对比度，色系变更保留播放与筛选，存储失败明确反馈。

- 历史补充：增加#/history导航与HistoryView，按影片合并、最近时间分组、当前集进度与来源、续播/重播、单条删除和清空确认；预览使用会话内数据，明确刷新重置。正式SQLite持久化由T18接入，按开发规格11.4恢复稳定内容/video身份。

### T0.14 T14 推荐首页界面
- [x] 完成
- 依赖：T0.6, T0.13
- 需求：U03, R01, R21, R34, R35, R37, REQ-T14 → A1（覆盖表含精确对应）
- 范围：`src/views/HomeView.vue`, `src/components/FilmCard.vue`, `src/App.vue`, `src/style.css`, `tests/`, `docs/PROGRESS.md`, `src/api.ts`, `src/state.ts`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t14.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实Stremio目录通过invoke接HomeView、元数据自动恢复仅home/details、独立播放源变更保持首页；骨架/空/错/多栏部分失败原位重试/分页/去重/返回滚动、数百卡/无图/长标题/过期图片恢复/窄宽布局。fixture硬编码或单栏覆盖全页时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t14.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T14-all.json",
      "--junit",
      "evidence/acceptance/T14-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T14-all.xml",
    "cases": [
      "U03",
      "R01",
      "R21",
      "R34",
      "R35",
      "R37",
      "REQ-T14"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T06/T13。
- 修改：src/views/HomeView.vue、海报卡与栏目分页。
- 工作：真实栏目、骨架、分页、滚动恢复、有限图片缓存、错误/空页与重试；failed_sections在原栏目位置显示错误与单栏重试；首次启动按1.2独立恢复元数据配置与播放源，元数据默认选择只要求Stremio home/details。
- 通过：所选Stremio配置进入对应首页，播放源变化保留元数据首页；长列表与无图/长标题可读；返回恢复位置。
- 验证：多栏目、数百卡片、重复项、失败重试、过期图片资源、窗口宽度变化。

### T0.15 T15 详情与选集界面
- [x] 完成
- 依赖：T0.7, T0.13
- 需求：U03, R22, REQ-T15 → A1（覆盖表含精确对应）
- 范围：`src/views/DetailsView.vue`, `src/App.vue`, `src/style.css`, `tests/`, `docs/PROGRESS.md`, `src/api.ts`, `src/state.ts`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t15.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实Stremio主详情及候选provider详情按metadata_ref区分，metadata video选集→匹配确认界面→真实引用；多类型/200集/重复名/缺季年份海报/简介折叠纯文本/多源中性分组/旧请求拒绝。界面所选与原episode不一致时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t15.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T15-all.json",
      "--junit",
      "evidence/acceptance/T15-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T15-all.xml",
    "cases": [
      "U03",
      "R22",
      "REQ-T15"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T07/T13。
- 修改：src/views/DetailsView.vue、分组和集数控件。
- 工作：标题/海报/简介/元数据、单片播放、多季/线路网格、相关内容。
- 通过：选中项与实际Episode.ref对应；换组明确；字段缺失自然呈现；旧详情响应无法覆盖新页。
- 验证：Movie/Series/Anime、多源语义分组、200集、重复集名、空可播列表。

### T0.16 T16 内嵌播放界面
- [x] 完成
- 依赖：T0.8, T0.12, T0.13
- 需求：U03, U09, R23, R25, R32, R42, R67, REQ-T16 → A1（覆盖表含精确对应）
- 范围：`src/views/PlaybackView.vue`, `src/player.ts`, `src/App.vue`, `src/data/history.ts`, `src/style.css`, `tests/`, `docs/PROGRESS.md`, `src/api.ts`, `src/state.ts`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t16.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "共用正式WebView管线真实播放/暂停/seek/音量0..100百分比/倍速0.5,1,1.25,1.5,2/全屏恢复/字幕/线路/键盘焦点/隐藏控制；EOS当前epoch与auto_next匹配下一video、歧义待确认/末集禁用；约5秒及事件快照/loadedmetadata续播/旧写入。UI假状态或错误自动下一集时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t16.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T16-all.json",
      "--junit",
      "evidence/acceptance/T16-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T16-all.xml",
    "cases": [
      "U03",
      "U09",
      "R23",
      "R25",
      "R32",
      "R42",
      "R67",
      "REQ-T16"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T08/T12/T13。
- 修改：src/views/PlaybackView.vue、控制栏、线路/集数/字幕浮层。
- 工作：实际事件驱动控制；暂停、seek、音量、倍速、全屏、选集、下一集；隐藏控制和键盘操作。
- 通过：视频区内嵌、音画正常；所有状态反映真实管线；EOS自动下一集受当前epoch与选项约束。
- 验证：键鼠/焦点、缩放/DPI、全屏恢复、末集、网络错/缓冲、字幕和线路切换。

- 历史补充：按11.4捕获播放快照，周期及暂停/seek/切集/离页更新；恢复进度等待媒体metadata，记录来源与影片身份，防止旧播放事件写到新集。

### T0.17 T17 源选择 引擎设置与受控交互
- [x] 完成
- 依赖：T0.13, T0.14, T0.4, T0.5
- 需求：R01, R13, R24, R58, REQ-T17 → A1（覆盖表含精确对应）
- 范围：`src/views/`, `src/App.vue`, `src/api.ts`, `src/style.css`, `tests/`, `docs/PROGRESS.md`, `src/state.ts`
- 交付物：落实下列完整原卡义务；提供`tests/acceptance/t17.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实HTTP配置增改禁用删除、元数据/播放独立选择、Cloudstream状态fixture及Setup入口/可见交互确认/取消；诊断含实际版本与关联ID且脱敏。HTTP界面失败/把合成交互标真实/敏感URL泄露时失败；真正客体重启和人机验证判定在N02。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t17.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T17-all.json",
      "--junit",
      "evidence/acceptance/T17-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T17-all.xml",
    "cases": [
      "R01",
      "R13",
      "R24",
      "R58",
      "REQ-T17"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 原始义务（完整原卡）

- 依赖：T13/T14/N00/N01。
- 修改：src/views/SourcesView.vue、src/views/SetupView.vue、诊断UI、必要的Android交互入口。
- 工作：元数据配置与播放源独立选择，MacCMS URL与Stremio manifest列表的新增/编辑/禁用/删除，按kind展示状态；Cloudstream按包展示与准备/诊断入口；初始化错误与修复指引；INTERACTION_REQUIRED的可见引擎流程。T17使用合成状态fixture完成交互UI，真实人机验证响应的采样与判定在N02验收；判定不出的情况保持UPSTREAM_FAILED。
- 通过：HTTP配置与真实首页可操作；Cloudstream准备、源列表与交互入口用状态fixture验证；真实SuperVod全集与可见交互由N02验收。宿主/客体状态分开，诊断脱敏。
- 验证：HTTP配置行为与Cloudstream交互状态fixture、拒绝/取消、复制诊断；真实客体重启和HTTP播放持续在N02复测。

### T0.19 T02 SuperVod JSON原生来源
- [x] 完成
- 依赖：T0.1, T0.4, T0.8, T0.9
- 需求：U02, U05, R03, R12, R16, R45, R49, REQ-T02 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `tests/`, `docs/compatibility-report.json`, `docs/runtime.lock.json`, `docs/ARCHITECTURE.md`, `docs/TASKS.md`, `docs/DECISIONS.md`, `docs/PROGRESS.md`, `docs/HANDOFF-RUST-REWRITE.md`, `docs/HANDOFF-RUST-REWRITE-PROMPT.md`, `docs/reviews/doc-review/`, `android-engine/`
- 补充验收（ADR-049，A1关联R12/R49/REQ-T02）：公共DNS固定1.1.1.1与8.8.8.8，无系统DNS/hosts回退；用受控DNS验证A/AAAA、单服务器失败转移、两服务器失败/取消/超时、混合公网内网及Fake-IP拒绝、DNS重绑定/重定向/peer校验；真实网络验证固定公共DNS下JSON与媒体链路，失败如实记录。测试覆盖公共解析入口，其他平台NOT RUN沿原决定。
- 交付物：记录AES用途核实结果（C4，无需新增插件AES解密）及公开响应实现依据；复用maccms.rs/source_dispatch.rs/resolution.rs/gateway.rs，以现有maccms kind和配置接入。候选为优质、量子、红牛、无尽、光速、极速、豪华、淘片、卧龙。程序对其公开响应的解析与映射全部兼容；接口不可访问、响应非JSON、无视频或媒体不可播属来源状态，单独记录，不算程序失败；可访问的JSON被适配器拒绝才是程序缺陷。已读插件源码只作范围风险调查，产品代码依据公开HTTP响应独立实现，保留实现依据，全部原包及Android补丁保留历史且从运行/发行路径排除。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "公开响应fixture验证列表/搜索/分类分页/详情/完整组选集/直接MP4或HLS解析；正常零搜索结果按空列表/匹配not_found呈现；HTML/非JSON/错误code/畸形字段/超限明确失败，解析无媒体按no_links错误呈现。复用现有来源管理手工配置入口（T0.17已交付，预设发现/添加UI留T0.22），至少一个候选通过真实公开JSON列表→详情→选集→resolve→Rust网关→打包WebView短播闭环；每个来源分别记录兼容性与来源可用性，HTTP200单独记录。脱敏记录网络/日期/来源状态，凭据/媒体签名URL留原生内存。媒体按多个视频抽样记录，单个视频或媒体域名失败只记入该样本，属来源状态；API被访问控制拒绝才停止请求该来源，媒体域名被拒只跳过该域名，均不规避并报告；安全负例与cap隔离保留。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t02.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T02-all.json",
      "--junit",
      "evidence/acceptance/T02-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T02-all.xml",
    "cases": [
      "U02",
      "U05",
      "R03",
      "R12",
      "R16",
      "R45",
      "R49",
      "REQ-T02"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作：记录AES用途核实结果（C4，无需新增插件AES解密）及公开响应实现依据；复用maccms.rs/source_dispatch.rs/resolution.rs/gateway.rs，以现有maccms kind和配置接入。候选为优质、量子、红牛、无尽、光速、极速、豪华、淘片、卧龙。程序对其公开响应的解析与映射全部兼容；接口不可访问、响应非JSON、无视频或媒体不可播属来源状态，单独记录，不算程序失败；可访问的JSON被适配器拒绝才是程序缺陷。已读插件源码只作范围风险调查，产品代码依据公开HTTP响应独立实现，保留实现依据，全部原包及Android补丁保留历史且从运行/发行路径排除。
- 通过与反证：公开响应fixture验证列表/搜索/分类分页/详情/完整组选集/直接MP4或HLS解析；正常零搜索结果按空列表/匹配not_found呈现；HTML/非JSON/错误code/畸形字段/超限明确失败，解析无媒体按no_links错误呈现。复用现有来源管理手工配置入口（T0.17已交付，预设发现/添加UI留T0.22），至少一个候选通过真实公开JSON列表→详情→选集→resolve→Rust网关→打包WebView短播闭环；每个来源分别记录兼容性与来源可用性，HTTP200单独记录。脱敏记录网络/日期/来源状态，凭据/媒体签名URL留原生内存。媒体按多个视频抽样记录，单个视频或媒体域名失败只记入该样本，属来源状态；API被访问控制拒绝才停止请求该来源，媒体域名被拒只跳过该域名，均不规避并报告；安全负例与cap隔离保留。
- 停止条件：缺必要公开行为依据、访问控制、真实媒体未验证或验收失败时如实记录，保留失败历史；仅fixture通过时在线闭环保持NOT RUN。

### T0.20 T03 SuperVod JSON多源身份与协议完整性
- [x] 完成
- 依赖：T0.19
- 需求：U02, U05, R04, R15, R18, REQ-T03 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `tests/`, `docs/compatibility-report.json`, `docs/PROGRESS.md`
- 交付物：仅覆盖T0.19晋级的JSON来源，复用配置UUID/HTTP身份公式；不增加其他六站。独立来源/revision、H向量、相同影片ID跨来源隔离、重复组选集保持源序及可选性、分页/部分失败处理。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "对全部支持JSON来源逐个执行公开响应fixture；两次启动身份稳定，改端点/禁用/删除/以新UUID重加旧引用失效，同UUID且同端点重加按身份公式仍为同一来源、旧引用继续有效；同名/同ID来源及组/集重复保持独立；数字与数字字符串/空字段/长标题/200集/多组/无分类/页尾/部分栏目失败逐项断言。丢集、乱序、串源、错误静默空列表或旧revision可用时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t03.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T03-all.json",
      "--junit",
      "evidence/acceptance/T03-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T03-all.xml",
    "cases": [
      "U02",
      "U05",
      "R04",
      "R15",
      "R18",
      "REQ-T03"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作：仅覆盖T0.19晋级的JSON来源，复用配置UUID/HTTP身份公式；不增加其他六站。独立来源/revision、H向量、相同影片ID跨来源隔离、重复组选集保持源序及可选性、分页/部分失败处理。
- 通过与反证：对全部支持JSON来源逐个执行公开响应fixture；两次启动身份稳定，改端点/禁用/删除/以新UUID重加旧引用失效，同UUID且同端点重加按身份公式仍为同一来源、旧引用继续有效；同名/同ID来源及组/集重复保持独立；数字与数字字符串/空字段/长标题/200集/多组/无分类/页尾/部分栏目失败逐项断言。丢集、乱序、串源、错误静默空列表或旧revision可用时失败。
- 停止条件：缺必要公开行为依据、访问控制、真实媒体未验证或验收失败时如实记录，保留失败历史；仅fixture通过时在线闭环保持NOT RUN。

### T0.21 T04 Rust来源请求上下文与故障隔离
- [x] 完成
- 依赖：T0.20, T0.3, T0.9
- 需求：U04, R05, R09, R10, R11, R12, R13, R36, R45, R63, REQ-T04 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `tests/`, `docs/runtime.lock.json`, `docs/PROGRESS.md`
- 交付物：复用resolution.rs原生上下文与gateway.rs，按当前JSON来源实际需求处理Referer/头/来源域隔离。只实现已经观察到的必要处理，保持TLS和公共出站策略；移除运行时对AVD/ADB/APK的准备依赖。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "无Android工具/镜像时Rust宿主及播放工作；来源A失败/超时/取消/被拒绝时来源B与Stremio继续；来源请求头跨源隔离、跨Origin敏感头清除、本地cap不发上游；401/403/挑战响应准确失败并停止访问，证书错误保持失败；切源/改配置/宿主重启与旧请求晚到不复活资源；取消与退出回收本项目请求/流/进程，外部资源保持。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t04.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T04-all.json",
      "--junit",
      "evidence/acceptance/T04-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T04-all.xml",
    "cases": [
      "U04",
      "R05",
      "R09",
      "R10",
      "R11",
      "R12",
      "R13",
      "R36",
      "R45",
      "R63",
      "REQ-T04"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作：复用resolution.rs原生上下文与gateway.rs，按当前JSON来源实际需求处理Referer/头/来源域隔离。只实现已经观察到的必要处理，保持TLS和公共出站策略；移除运行时对AVD/ADB/APK的准备依赖。
- 通过与反证：无Android工具/镜像时Rust宿主及播放工作；来源A失败/超时/取消/被拒绝时来源B与Stremio继续；来源请求头跨源隔离、跨Origin敏感头清除、本地cap不发上游；401/403/挑战响应准确失败并停止访问，证书错误保持失败；切源/改配置/宿主重启与旧请求晚到不复活资源；取消与退出回收本项目请求/流/进程，外部资源保持。
- 停止条件：缺必要公开行为依据、访问控制、真实媒体未验证或验收失败时如实记录，保留失败历史；仅fixture通过时在线闭环保持NOT RUN。

### T0.22 N02 Rust来源接入宿主与来源管理
- [x] 完成
- 依赖：T0.20, T0.21, T0.3, T0.9, T0.17
- 需求：U02, U05, U08, R03, R05, R06, R10, R13, R15, R16, R24, R26, R27, R29, R36, R49, R50, R63, R66, REQ-N02 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `src/`, `protocol/`, `docs/compatibility-report.json`, `tests/`, `docs/PROGRESS.md`
- 交付物：将已验证JSON来源作为可选预设配置通过现有设置入口添加，复用maccms kind、配置上限/UUID/独立偏好；保留用户已有HTTP配置。移除页面Android准备/原包交互提示与不可用入口，过时cloudstream配置/偏好保留可诊断迁移说明，保持既有17命令线格式兼容（cloudstream准备/交互明确返回未支持错误）；protocol只随实际契约变化补正负例。修复来源校验中文提示。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实Rust Host+打包WebView验收支持来源可发现/显式添加/独立选择/重启恢复、增改禁用删与上限重复；Stremio推荐→详情→指定video匹配JSON来源→人工确认→解析→播放切集；MacCMS无验证外部ID继续要求确认，歧义暂停自动下一集；失败来源与最近请求诊断准确，受控场景验证Session强持有/候选释放后播放继续/Range/HLS/字幕/条件DASH/取消/清理。保留用户配置、错误及旧历史；合成与公开实测分别统计，全部支持来源能力可追溯。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/n02.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/N02-all.json",
      "--junit",
      "evidence/acceptance/N02-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/N02-all.xml",
    "cases": [
      "U02",
      "U05",
      "U08",
      "R03",
      "R05",
      "R06",
      "R10",
      "R13",
      "R15",
      "R16",
      "R24",
      "R26",
      "R27",
      "R29",
      "R36",
      "R49",
      "R50",
      "R63",
      "R66",
      "REQ-N02"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作：将已验证JSON来源作为可选预设配置通过现有设置入口添加，复用maccms kind、配置上限/UUID/独立偏好；保留用户已有HTTP配置。移除页面Android准备/原包交互提示与不可用入口，过时cloudstream配置/偏好保留可诊断迁移说明，保持既有17命令线格式兼容（cloudstream准备/交互明确返回未支持错误）；protocol只随实际契约变化补正负例。修复来源校验中文提示。
- 通过与反证：真实Rust Host+打包WebView验收支持来源可发现/显式添加/独立选择/重启恢复、增改禁用删与上限重复；Stremio推荐→详情→指定video匹配JSON来源→人工确认→解析→播放切集；MacCMS无验证外部ID继续要求确认，歧义暂停自动下一集；失败来源与最近请求诊断准确，受控场景验证Session强持有/候选释放后播放继续/Range/HLS/字幕/条件DASH/取消/清理。保留用户配置、错误及旧历史；合成与公开实测分别统计，全部支持来源能力可追溯。
- 停止条件：缺必要公开行为依据、访问控制、真实媒体未验证或验收失败时如实记录，保留失败历史；仅fixture通过时在线闭环保持NOT RUN。

### T0.18 T18 设置与全生命周期清理
- [x] 完成
- 依赖：T0.16, T0.17, T0.22
- 需求：U01, U09, R11, R36, R38, R39, R40, R41, R42, R43, R64, R65, REQ-T18 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `src/api.ts`, `src/models.ts`, `src/data/history.ts`, `src/data/history-model.ts`, `src/views/HistoryView.vue`, `src/App.vue`, `protocol/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t18.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 契约顺序：按ARCHITECTURE C16的四个历史命令、字段与写入序号规则补齐protocol正负样例后再进入Rust实现；这是本卡既定交付。既有17个命令、原有DTO保持兼容；偏离C16定义时才按冻结修订程序处理。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "C16四个历史IPC实际参数/结果/每命令正负例及写序号、删除屏障、Session确认身份和match/video绑定负例，工作协议纳入完整verify；合法非UUID的match_id贯通匹配/解析/Session/历史，拒绝空/超长/未知/错误归属句柄；活动播放超过5分钟强制GC匹配/候选后继续保存，切集/切源/离页/退出延迟提交已接纳快照仍保留准确影片与video进度，旧Session关闭后新发未接纳快照拒绝，删除/清空屏障拒绝已接纳旧写，写失败相同序号同载荷可重试而不同载荷拒绝；实际Settings损坏/迁移/原子失败/退出队列、50次切换与自身进程/流/网关清理；SQLite实际文件、PRAGMA版本事务回滚/参数绑定、完整历史列与索引、跨重启/磁盘权限/乱序/时钟回拨、删除清空与待写快照、旧事件/退出排空/未保存提示；历史IPC先同步protocol与正负例再接现有history入口。仅Map/localStorage、丢原文件或删后复活时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t18.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T18-all.json",
      "--junit",
      "evidence/acceptance/T18-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T18-all.xml",
    "cases": [
      "U01",
      "U09",
      "R11",
      "R36",
      "R38",
      "R39",
      "R40",
      "R41",
      "R42",
      "R43",
      "R64",
      "R65",
      "REQ-T18"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 依赖：T16/T17。
- 修改：src-tauri/src/settings.rs、history.rs、前端卸载/关闭与崩溃恢复测试。
- 工作：延续T05的Settings v2及v1迁移，验证source_configs和音量/auto_next/窗口/最近源；原子写入；schema_version与损坏恢复；前后台ResumeSnapshot仅驻留内存；取消全链；自身进程、流与图像清理。
- 通过：退出后无本项目残余媒体声/请求；用户其他进程和设备状态保留；配置异常可定位；重启状态合理。
- 验证：写入中断、权限错误、中文目录、kill/relaunch、窗口关闭、连续50次切换。

- SQLite补充：按11.4实现Rust本地历史仓储与迁移；先同步机器DTO、IPC和样例，再替换前端history数据入口。验证跨重启、迁移失败、磁盘/权限错误、原子更新、乱序写入、删除/清空与待写快照、退出写队列；Settings v2维持JSON兼容。

### T0.23 T19 来源配置刷新与事务
- [x] 完成
- 依赖：T0.20, T0.3, T0.22
- 需求：R02, R04, R14, R44, REQ-T19 → A1（覆盖表含精确对应）
- 范围：`docs/HANDOFF-CODEX-T0.24.md`, `src-tauri/src/`, `src/views/NativeSourcesView.vue`, `src/state.ts`, `src/api.ts`, `src/models.ts`, `docs/runtime.lock.json`, `tests/`, `docs/PROGRESS.md`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
- 交付物：落实下列修订义务；提供`tests/acceptance/t19.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "公开JSON来源配置与探测快照事务：无变更/探测失败保持确认配置及播放；编辑/禁用/删除提交时停止受影响来源会话/撤销资源/拒绝旧结果；失败保留旧配置和注册表且已停会话保持停止，其他来源连续；并发sync_sequence/generation核对、取消、超限/非法URL/错误响应反例。部分发布或旧引用复活时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t19.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T19-all.json",
      "--junit",
      "evidence/acceptance/T19-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T19-all.xml",
    "cases": [
      "R02",
      "R04",
      "R14",
      "R44",
      "REQ-T19"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：复用settings.rs/source_dispatch.rs与来源管理入口，测试配置探测锁外、保存同锁、配置原子替换/失败保持确认值、修订与世代失效、无变更保持播放及并发取消。删除插件索引刷新/下载/ZIP安装产品路径；旧制品仅留历史。配置刷新失败保持前一完整状态，报告保留请求失败原因。

### T0.24 T20 Rust来源与产品端到端
- [x] 完成
- 依赖：T0.14, T0.15, T0.16, T0.17, T0.18, T0.23, T0.22
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：U02, U03, U04, U05, U08, U09, R07, R10, R16, R20, R40, R41, R42, R43, R50, R62, R63, R66, R67, REQ-T20 → A1（覆盖表含精确对应）
- 范围：`package.json`, `package-lock.json`, `vite.config.ts`, `protocol/`, `scripts/check-contracts.py`, `scripts/workflow.py`, `scripts/tests/`, `src/`, `src-tauri/`, `docs/compatibility-report.json`, `docs/release-evidence/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t20.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "本机macOS arm64真实Stremio→核准Rust JSON来源推荐/详情/匹配确认/内嵌播放切集；受控HTTP媒体完整正负例与单源故障隔离；每个支持来源按实际能力记录公开JSON和媒体链路，在线失败与fixture结果分列；真实SQLite重启/跨季video/换源重匹配/当前集EOS/失败保留历史及双语明暗390px；媒体磁盘缓存真实命中减少上游请求、精确Range/来源修订/请求上下文隔离、cap失效不命中、过期与no-store/no-cache、LRU容量与临时字节核算、重启恢复、取消断流无残片、并发清理/降额屏障、设置页改容量/关闭/清理/失败反馈与播放继续。Video.js 10.3.0官方播放器/控制栏实际渲染和操作、MP4/HLS字幕/倍速/音量/原生全屏/seek/图标提示/键盘/中英文390px、单实例及50次切集/卸载旧音频事件隔离和缓存历史续播保持；缺真实公开来源闭环、macOS运行、官方播放器真实操作，或只复制fixture摘要时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t20.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T20-all.json",
      "--junit",
      "evidence/acceptance/T20-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T20-all.xml",
    "cases": [
      "U02",
      "U03",
      "U04",
      "U05",
      "U08",
      "U09",
      "R07",
      "R10",
      "R16",
      "R20",
      "R40",
      "R41",
      "R42",
      "R43",
      "R50",
      "R62",
      "R63",
      "R66",
      "R67",
      "REQ-T20"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：真实Host+打包WebView验证元数据至播放/切集、独立选择/偏好、配置更新、历史续播/失败保存与资源回收；每个支持JSON来源逐项复核能力与原始公开响应证据。补受控网络错误、歧义确认、字幕/线路及自动下一集暂停/恢复；至少一个真实公开JSON来源完整短播。来源失败保留具体日期/环境/阶段与重试入口，禁止用原插件执行结果替代独立实现。


- 用户新增缓存交付（ADR-054）：按C9/C11实现本地媒体磁盘缓存、Settings可选容量字段及get_media_cache/clear_media_cache IPC，缓存文件不含URL/凭据原文；真实设置按钮及受控HTTP请求计数/文件断言拒绝仅展示占位功能。旧协议正负例全部保留，并增加新两命令正负例；图标播放控制的悬浮说明、状态切换、键盘名称及390px行为一并验收。


- 用户指定播放器迁移（ADR-055）：使用Video.js 10.3.0官方播放器与控制栏替换自搭基础实现，必要官方HTML/HLS包精确锁版，Vue精确自定义标签白名单；保留原缓存/切集/字幕/历史/事件身份与错误恢复，扩展业务接线不复制基础控制栏，不接CDN或分析服务。旧T01/T12/T16播放器能力由本卡最新实现承接，测试定位改为官方组件真实行为，原安全/资源门和必需反例保持。

### T0.25 T21 媒体网关与鉴权回归
- [x] 完成
- 依赖：T0.10, T0.11, T0.12, T0.22
- 需求：R05, R12, R19, R26, R27, R28, R29, R34, R37, R44, R46, R48, R49, R50, C-RESOURCE, REQ-T21 → A1（覆盖表含精确对应）
- 范围：`src-tauri/src/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t21.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "macOS播放器经Rust真实gateway访问同一受控自有媒体集合及权限负例、每条HLS/key/segment/header/Cookie/Range/重定向/body变换/慢流背压功能、跨Session/过期cap/图片LRU/播放租约/取消/大小/模板/CSP/CORS/Tauri权限、上游无本地凭据；真实集群若使用须Docker/硬限额合计+宿主RSS总预算/实际用量/自产清理。错子请求或资源漏账时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t21.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T21-all.json",
      "--junit",
      "evidence/acceptance/T21-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T21-all.xml",
    "cases": [
      "R05",
      "R12",
      "R19",
      "R26",
      "R27",
      "R28",
      "R29",
      "R34",
      "R37",
      "R44",
      "R46",
      "R48",
      "R49",
      "R50",
      "C-RESOURCE",
      "REQ-T21"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 依赖：T10/T11/T12/N02。
- 修改：tests/contract、Rust gateway tests与共用自有媒体集合。
- 工作：自动验证清单/Key/分片、header、Cookie、Range、重定向、body修改、取消、图片cap到期所属DTO刷新、播放滑动租约、cap到期/撤销、大小限制、跨session隔离，以及Tauri命令权限/CSP/CORS负例。
- 通过：三平台播放器访问同一受控媒体集合；每条子请求可关联会话；任意URL/路径输入被拒绝；本地令牌不会出现在上游。
- 验证：包络与资源权限负例，过期引用，非法模板输入，超大body/图像，慢流背压功能；网关耐久播放移交T0.26/A2真实产品验收（ADR-044为10分钟）。

### T0.26 T22 三平台性能与故障注入
- [x] 完成
- 执行阶段：产品测试；承接T0.25移交的10分钟真实网关播放（ADR-044），沿R48/REQ-T22与A2实测，保留资源/清理门。
- 依赖：T0.18, T0.23, T0.24, T0.25
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：R09, R47, R48, C-RESOURCE, REQ-T21, REQ-T22 → A1, A2（覆盖表含精确对应）
- 范围：`src/`, `src-tauri/`, `docs/release-evidence/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t22.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "本项suite=startup-ui：macOS arm64实际执行5次Rust宿主冷启动，记录参考机/OS/CPU/GPU/依赖、起止时间与原始RSS及响应样本；1.5秒窗口、100ms UI、本地无外网桥接p95<100ms、宿主健康60秒仅记录目标差距。验证最小窗口和125/150/200等效缩放、网络/来源/文件故障与恢复、即时资源清理；核对网关长测自有媒体1080p30/H264AAC、20Mbps负载、采样/预算/清理方案可用。REQ-T21及R48在本suite仅覆盖长测准备和UI/故障，完整时长/50切集由A2执行。零用例、缺原始样本、未解决关键可靠性问题或伪称其他平台通过时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t22.py",
      "--suite",
      "startup-ui",
      "--report",
      "evidence/acceptance/T22-startup-ui.json",
      "--junit",
      "evidence/acceptance/T22-startup-ui.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T22-startup-ui.xml",
    "cases": [
      "R09",
      "R47",
      "R48",
      "C-RESOURCE",
      "REQ-T21",
      "REQ-T22"
    ],
    "timeout_seconds": 7200
  },
  {
    "id": "A2",
    "description": "本项suite=macos-arm64：通过当前真实打包播放器和Rust网关播放自有1080p30 H264AAC连续10分钟、20Mbps受控流连续3分钟；实际50次切集无旧音频/旧帧/悬挂会话，owned RSS五切窗口中位数首末增长≤64MiB且均≤512MiB。保留真实参考机/环境、起止、媒体与负载、逐样本RSS和CPU/GPU、取消/退出资源清理原始证据；沿C14预算，真实集群若使用须Docker硬限额合计+宿主RSS与自产清理。R47在本suite核对A1同源码/基线的5次启动原始收据，不重复运行启动；REQ-T21在本suite承接完整10分钟网关耐久。缺时长/样本/清理、资源漏账或关键可靠性缺陷时失败；其他平台NOT RUN。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t22.py",
      "--suite",
      "macos-arm64",
      "--report",
      "evidence/acceptance/T22-macos-arm64.json",
      "--junit",
      "evidence/acceptance/T22-macos-arm64.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T22-macos-arm64.xml",
    "cases": [
      "R09",
      "R47",
      "R48",
      "C-RESOURCE",
      "REQ-T21",
      "REQ-T22"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 依赖：T18/T19/T20/T21。
- 修改：证据与必要的局部修复。
- 工作：测量参考机启动、Rust来源、代理吞吐、1080p30、切集/缩放、WebView媒体资源、CPU/RSS；注入网络/来源/文件故障。
- 通过：本规格性能目标达到或有可复现的明确差距及修复；关键可靠性问题清零。
- 验证：每平台独立报告；先定位再优化，避免无关重构与提前引入新播放后端。

### T0.27 T23 发行安装包
- [x] 完成
- 依赖：T0.26
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：U04, R33, R45, REQ-T23 → A1（覆盖表含精确对应）
- 范围：`packaging/`, `src-tauri/`, `licenses/`, `docs/runtime.lock.json`, `package.json`, `package-lock.json`, `README.md`, `src/data/defaultSources.json`, `src/data/sourcePresets.ts`, `src/main.ts`, `src/state.ts`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t23.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "当前源码构建的macOS arm64个人使用安装包 → 期望：本地前端无运行CDN和开发绝对路径，实际依赖许可与版本锁一一对应，安装包不含原插件、引擎APK或SDK镜像，ZIP和DMG解包及ad-hoc签名密封复验通过，Developer ID和公证状态如实记录，缺Apple凭据时列入待办。本验收只检查安装包，不启动应用窗口、虚拟机或其他机器；干净系统安装、初始化、首播、升级、卸载由T0.36/A2执行（ADR-058）。其他平台NOT RUN。会被拒绝：产物与当前源码不对应、未签名包声称已签名或公证、依赖证据缺失、包内含禁止材料。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t23.py",
      "--suite",
      "package",
      "--report",
      "evidence/acceptance/T23-package.json",
      "--junit",
      "evidence/acceptance/T23-package.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T23-package.xml",
    "cases": [
      "U04",
      "R33",
      "R45",
      "REQ-T23"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：Tauri构建、本地资源、实际依赖和版本锁/许可通知；移除Android运行时/原包发行材料要求及产物。无开发机绝对路径；干净机器安装/首播/升级/卸载按ADR-058移至T0.36/A2；缺签名凭据如实列未签名与发行待办，公开发行另获用户授权。
- `tests/acceptance/t23.py` 增加 `--suite package` 和 `--suite lifecycle`：package 运行资源、平台、当前构建、依赖、产物、归档、签名和反例检查，只输出 U04、R33、R45、REQ-T23 及具名行为用例；lifecycle 供 T0.36/A2 使用。`packaging/T23.md` 同步说明。
- `t23.py` 移除 `--suite all`；`DEFERRED.reproduce_argv` 改为 `--suite package`；报告标明实际 suite；`save()` 的 limitations 写明生命周期由 T0.36/A2 执行。

### T0.28 TV1 桌面外壳、深色主题与焦点系统
- [x] 完成
- 依赖：T0.27
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-SHELL, TV-THEME, TV-FOCUS, REQ-TV1 → A1（覆盖表含精确对应）
- 范围：`src/App.vue`, `src/style.css`, `src/ui/`, `src/components/`, `src/i18n.ts`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/`, `src-tauri/src/automation.rs`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成沉浸式窗口、悬浮侧栏、桌面固定深色主题、键盘焦点系统和共用内容排组件。提供`tests/acceptance/tv1.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "环境：真实打包窗口，宽度960、1200、1512及窄屏768、390 → 期望：标题栏融入画面且窗口可拖动，悬浮侧栏可折叠，桌面固定深色且色系仍生效，方向键可在卡片和内容排间移动，鼠标悬停与键盘焦点样式一致，减弱动态下无推镜和倾斜，窄屏布局与改造前一致。会被拒绝：窗口无法拖动、焦点丢失到页面外、窄屏样式改变、用截图代替真实焦点断言。；计算样式核对本卡“页面风格规格”中的颜色、字号、尺寸、圆角、动效和状态，并保存960、1200、1512宽度截图供走查。会被拒绝：规格值被改动而未记录校准结果、状态样式缺失。MediaRow 在本卡由夹具挂载核对，真实页面接线由 T0.30 验收；`capabilities/main.json` 的权限集合必须等于基线加 `core:window:allow-start-dragging`，`tauri.conf.json` 的 CSP 必须与基线一致，多授予权限或放宽 CSP 时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv1.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV1-all.json",
      "--junit",
      "evidence/acceptance/TV1-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV1-all.xml",
    "cases": [
      "TV-SHELL",
      "TV-THEME",
      "TV-FOCUS",
      "TV-SHELL/overlay-titlebar-drag",
      "TV-SHELL/floating-sidebar-collapse",
      "TV-SHELL/narrow-unchanged",
      "TV-THEME/desktop-dark-palette",
      "TV-FOCUS/keyboard-navigation",
      "TV-FOCUS/hover-focus-parity",
      "TV-FOCUS/reduced-motion",
      "TV-THEME/visual-tokens",
      "TV-SHELL/permissions-exact",
      "REQ-TV1"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- 开工前对照本机 TV App 校准侧栏位置、焦点放大比例和动效时长，结果写入 PROGRESS。
- `lib.rs` 主窗口使用 Overlay 标题栏并隐藏标题文字；`capabilities/main.json` 增加 `core:window:allow-start-dragging`，其余权限保持。
- 取消顶栏；侧栏悬浮在内容之上，窗口宽度小于1200px时默认折叠；元数据源和播放源收进侧栏底部，语言和色系入口移到设置页。本卡接入现有首页、观看历史、来源、设置入口，后续卡各自增加分类、搜索、资料库入口。
- 宽度不小于960px时固定深色，五种色系作为强调色；小于960px的布局、明暗跟随和样式保持现状。
- 新增 `src/ui/focus.ts` 和 `src/components/MediaRow.vue`：原生 DOM 焦点上的方向键导航、Esc 逐级返回；内容排横向吸附、悬停箭头、竖版海报；动画只用 transform 和 opacity。初始参数：放大1.08、200ms、倾斜不超过4度。
- `automation.rs` 的验收宽度白名单增加1512。本机显示器逻辑宽度为1512，真实窗口验收宽度为960、1200、1512；1920宽度记为 NOT RUN，接外接显示器时补测。

#### 页面风格规格（ADR-057）

**规格来源与校准。** 本卡定义 T0.28 至 T0.34 共用的设计规格。开工时对照本机 TV App 校准，调整后的最终值写入 PROGRESS，后续卡沿用最终值。

**实现方式。** 变量集中在 `src/ui/theme.ts` 的 `resolveTheme`，以 `--tv-*` 输出，只在宽度不小于960px时生效；Naive UI 通过 `themeOverrides` 同步。毛玻璃同时写 `backdrop-filter` 和 `-webkit-backdrop-filter`（macOS 14 的 WebView 需要后者）。

**颜色。**

| 变量 | 值 | 用途 |
| --- | --- | --- |
| `--tv-bg` | `#0b0b0d` | 页面底色 |
| `--tv-surface-1` | `rgba(255,255,255,.06)` | 海报占位、分组卡片 |
| `--tv-surface-2` | `rgba(255,255,255,.10)` | 胶囊、输入框 |
| `--tv-surface-hover` | `rgba(255,255,255,.16)` | 悬停态 |
| `--tv-glass` | `rgba(30,30,34,.62)` | 玻璃底色：侧栏、面板、控制条 |
| `--tv-glass-filter` | `blur(30px) saturate(160%)` | 玻璃模糊 |
| `--tv-hairline` | `rgba(255,255,255,.10)` | 分隔线、玻璃描边 |
| `--tv-text` | `#f5f5f7` | 主文字 |
| `--tv-text-2` | `rgba(235,235,245,.62)` | 次要文字 |
| `--tv-text-3` | `rgba(235,235,245,.38)` | 禁用态 |
| `--tv-button` / `--tv-button-ink` | `#f5f5f7` / `#0b0b0d` | 主按钮 |
| `--tv-danger` | `#ff453a` | 删除、错误 |
| `--accent` | 当前色系 night 值 | 进度条、选中勾、当前集 |

**字体。** 沿用系统字体栈，字间距0，时间和集数使用 `tabular-nums`。

| 层级 | 字号/行高/字重 |
| --- | --- |
| Hero 片名 | `clamp(40px,4.4vw,72px)` / 1.05 / 700 |
| 页面标题 | 34px / 1.15 / 700 |
| 内容排标题 | 22px / 1.25 / 600 |
| 正文 | 15px / 1.5 / 400 |
| 卡片标题 | 14px / 1.3 / 500 |
| 辅助信息 | 13px / 1.4 / 400 |
| 角标 | 11px / 1.2 / 600 |

**尺寸。** 间距以4px为基准；内容左边距为侧栏外沿加32px，右边距48px（宽度不小于1440px时64px）；内容排上下间距40px，卡片间距16px（不小于1440px时20px）。圆角：卡片12px，面板和侧栏20px，分组卡片14px，小浮层（筛选面板、搜索框、下一集卡片）16px，胶囊和按钮999px。

**动效。** 标准曲线 `cubic-bezier(.2,.8,.2,1)`；焦点200ms，浮层和页面300ms，Hero 淡入600ms；减弱动态时关闭位移、缩放和推镜，只保留淡入。

**窗口与侧栏。** 顶部44px为拖动区，其中的按钮和链接不触发拖动。侧栏距窗口上、左、下各12px，展开宽220px，折叠宽72px，宽度过渡240ms；玻璃底、1px `--tv-hairline` 描边、圆角20px；顶部留52px给红绿灯。导航项高36px、圆角10px、图标20px、文字14/500，悬停 `--tv-surface-1`，当前项底色 `rgba(255,255,255,.14)`；折叠时只显示图标并提供悬浮说明。底部来源胶囊高32px、文字13px、超长省略。

**图标。** 继续使用 `Icon.vue` 的线性风格，1.75px描边；补充搜索、资料库、侧栏折叠、综艺、纪录片、短剧、体育、待播图标。

**焦点态。** 悬停和键盘焦点相同：放大1.08、阴影 `0 18px 40px rgba(0,0,0,.55)`、顶部斜向高光（透明度0至.18随光标移动）；键盘焦点额外显示2px `--tv-text` 内描边。按钮和胶囊焦点放大1.04。

**内容排 `MediaRow`。** 标题行高28px、下方12px，标题可点时右侧显示箭头。竖版海报宽 `clamp(132px,12.5vw,220px)`、比例2:3；横版卡宽 `clamp(240px,22vw,360px)`、比例16:9。卡片下方标题单行省略，辅助信息单行。排内上下各留24px，避免放大被裁切；右缘48px渐隐露出下一张。翻页按钮44px圆形玻璃，垂直居中于海报，悬停内容排时淡入，到头隐藏；隐藏横向滚动条（同时设置 `scrollbar-width:none` 和 `::-webkit-scrollbar`）。

**加载与缺图。** 占位为 `--tv-surface-1`，透明度在.5至.8之间1.2秒循环；图片加载后淡入200ms；缺图时显示居中片名，15/600。

### T0.29 TV2 目录协议、TMDB筛选与默认来源修复
- [x] 完成
- 依赖：T0.27
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-CATALOG-DTO, TV-FILTER, TV-SOURCE-UPGRADE, REQ-TV2 → A1（覆盖表含精确对应）
- 范围：`protocol/`, `src/models.ts`, `src/api.ts`, `src/state.ts`, `src-tauri/src/stremio.rs`, `src-tauri/src/home.rs`, `src-tauri/src/host.rs`, `src-tauri/src/contracts.rs`, `src-tauri/src/settings.rs`, `src/data/defaultSources.json`, `scripts/check-contracts.py`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成卡片与栏目协议扩展、TMDB单条件及组合筛选、分页游标隔离，并修复升级用户缺少TMDB来源。提供`tests/acceptance/tv2.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "输入：受控Stremio目录fixture及内置TMDB实际请求样本，覆盖单条件、年份+类型、语言+类型、语言+年份、占位条目、翻页和语言切换 → 期望：请求携带正确的genre与skip，卡片含backdrop、plot、genres，组合结果全部符合条件，单次最多扫描5页并返回已扫描与命中数，不同筛选的游标互不影响，占位条目按空结果返回；已有MacCMS配置且缺Stremio的设置补上TMDB一次，用户随后删除时保持删除。会被拒绝：筛选值未发送、结果含不符合条件的影片、游标串用、重复补回用户已删除的来源。；字段缺失时按“页面风格规格”的对应表返回空值，筛选项带可显示名称，并返回已扫描数和命中数。会被拒绝：缺失字段返回空字符串或占位值、筛选项缺少显示名称。排序映射和各来源可用维度按工作义务返回：请求已被禁用的维度时明确报错。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv2.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV2-all.json",
      "--junit",
      "evidence/acceptance/TV2-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV2-all.xml",
    "cases": [
      "TV-CATALOG-DTO",
      "TV-FILTER",
      "TV-SOURCE-UPGRADE",
      "TV-CATALOG-DTO/card-fields",
      "TV-CATALOG-DTO/section-kind-facets",
      "TV-FILTER/single-condition",
      "TV-FILTER/priority-routing",
      "TV-FILTER/scan-limit-progress",
      "TV-FILTER/cursor-isolation",
      "TV-FILTER/placeholder-empty",
      "TV-FILTER/localized-values",
      "TV-SOURCE-UPGRADE/adds-once",
      "TV-SOURCE-UPGRADE/respects-removal",
      "TV-CATALOG-DTO/style-fields",
      "TV-FILTER/sort-routing",
      "TV-FILTER/facet-availability",
      "REQ-TV2"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- `Bridge_MediaCard` 增加可选 `backdrop`、`plot`、`genres`；`Bridge_HomeSection` 增加 `kind` 与可用筛选项；`Bridge_HomeRequest` 增加可选 `filter`（类型、年份、语言、地区、排序）；修改 `protocol/` 后重新生成 `src/models.ts`。
- `stremio.rs:904` 发送 `genre=…&skip=…`；筛选值按来源清单声明校验。
- 路由优先级为语言、年份、类型：一个条件交给服务端，其余在 Rust 用目录返回的 `genre`、`year` 过滤。仅类型时电影用 `tmdb.top`，剧集用 `tmdb.latest`。
- 客户端过滤时每次最多连续请求5页，凑够一屏即停，返回已扫描数和命中数供界面显示“继续查找”。2026-11-08实测5页100条的命中范围为8到95条。
- `genres` 为 `No Results` 的占位条目按空结果处理，TMDB 剧集“热门”目录恢复内容时自动显示。
- 验证中文筛选值在英文基础数据加中文覆盖的 `localized` 链路上取到正确分类。
- 修复 PROGRESS 记录的升级场景：已有播放源且缺 Stremio 的配置自动补一次中文 TMDB。
- `Bridge_HomeRequest.filter` 增加 `sort`，取值 `trending_day`、`trending_week`、`popular`、`latest`。无语言和年份条件时，排序选择目录：趋势用 `tmdb.trending`（`genre=Day/Week`），热门用 `tmdb.top`，最新用 `tmdb.latest`；此时类型在目录支持时走服务端，趋势目录下走 Rust 过滤。选了语言或年份时排序不可用。剧集的排序选项为趋势和最新。
- 每个栏目返回可用筛选项：TMDB 的“地区”为禁用，因目录没有地区字段；排序按上一条声明可用性；MacCMS 的维度由 T0.31 声明。
- `Bridge_HomeSection` 增加可选 `scanned_count`、`matched_count`；`Settings` 增加可选 `metadata_preset_applied`，记录 TMDB 已补过，用户删除后不再补回。
- `check-contracts.py` 收紧为归档字段逐项相等，只放行本卡列名的可选字段（`Bridge_MediaCard`、`Bridge_HomeSection`、`Bridge_HomeRequest`、`Settings`），不做全量放行。

#### 页面风格规格（ADR-057）

本卡负责界面所需字段，样式由 T0.30 落地。字段与展示的对应关系：

| 字段 | 展示位置 | 缺失时 |
| --- | --- | --- |
| `backdrop` | Hero、横版卡 | 模糊放大的海报（40px模糊、1.2倍）叠加遮罩 |
| `plot` | Hero 简介（最多3行） | 隐藏简介，版面不留空 |
| `genres` | Hero 信息行（最多2个）、筛选 | 隐藏 |
| `rating` | 信息行评分 | 值为空或0时隐藏 |
| `year` | 信息行 | 隐藏 |
| 筛选项 | `FilterBar` 面板 | 该维度胶囊禁用，文字用 `--tv-text-3` |

筛选结果返回已扫描数和命中数，供界面显示“已查找 100 部，找到 12 部”；中文环境下筛选项按来源声明的显示名输出。

### T0.30 TV3 首页与分类页
- [x] 完成
- 依赖：T0.28, T0.29
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-HOME, TV-BROWSE, REQ-TV3 → A1（覆盖表含精确对应）
- 范围：`src/App.vue`, `src/views/HomeView.vue`, `src/views/BrowseView.vue`, `src/components/`, `src/style.css`, `src/i18n.ts`, `src/state.ts`, `src/data/history.ts`, `protocol/`, `src/models.ts`, `src-tauri/src/stremio.rs`, `src-tauri/src/home.rs`, `src-tauri/src/host.rs`, `src-tauri/src/contracts.rs`, `scripts/check-contracts.py`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成Apple TV式首页、分类页、筛选栏与海报墙。提供`tests/acceptance/tv3.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "环境：真实打包窗口接内置TMDB及受控fixture → 期望：首页显示可轮播Hero、继续观看和按分类的横向内容排；电影、电视剧、动画、纪录片入口显示各自内容；TMDB 分类下类型、年份、语言可组合，排序在无语言和年份时可用，地区按来源声明禁用并显示原因，选中后切换海报墙并滚动加载，条件写入地址，返回详情后恢复筛选和位置；内容排进入视口才请求；空结果给出去掉条件的快捷入口。会被拒绝：各分类显示相同内容、筛选后结果与条件不符、返回后丢失状态、首屏一次请求全部内容排、缺少按类型或按年份内容排。；计算样式核对本卡“页面风格规格”中的颜色、字号、尺寸、圆角、动效和状态，并保存960、1200、1512宽度截图供走查。会被拒绝：规格值被改动而未记录校准结果、状态样式缺失。首页内容排在真实页面按 T0.28 的 MediaRow 规格核对海报宽度、留白和翻页按钮。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv3.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV3-all.json",
      "--junit",
      "evidence/acceptance/TV3-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV3-all.xml",
    "cases": [
      "TV-HOME",
      "TV-BROWSE",
      "TV-HOME/hero-rotation-pause",
      "TV-HOME/continue-watching",
      "TV-HOME/category-rows",
      "TV-HOME/lazy-rows",
      "TV-BROWSE/distinct-categories",
      "TV-BROWSE/filter-bar-combination",
      "TV-BROWSE/poster-wall-pagination",
      "TV-BROWSE/see-all",
      "TV-BROWSE/restore-state",
      "TV-BROWSE/empty-result",
      "TV-HOME/visual-spec",
      "TV-HOME/genre-year-rows",
      "TV-HOME/media-row-spec",
      "TV-HOME/catalog-pending",
      "TV-HOME/filtered-row-isolation",
      "REQ-TV3"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- 按 ADR-059 增加 `HomeRequest.catalog_only`；真实 Host 断言仅列栏目时内容请求为零、旧全量请求兼容、非法参数组合及不支持的来源明确报错；首页仅在栏目进入视口后请求其内容，沿用原引用、取消和分页规则。
- A1 的 `TV-HOME/catalog-pending` 在真实 Host/窗口检查栏目描述为待加载而非空结果：有描述时渲染可进入视口的排，进入视口请求第一页；仅实际首请求完成且为空才显示该排空态，验证失败、重试及来源切换。`section_ref` 和 `filter` 缺省或 null 等价；`filter: {}` 属于有筛选参数，在 catalog_only=true 时拒绝，并验证正反例。非 Stremio 来源明确拒绝该模式，T0.31 继续使用其支持路径。
- A1 的 `TV-HOME/filtered-row-isolation` 以来源及修订、语言世代、栏目引用和规范化筛选条件区分页面排，独立保存加载、错误、结果与分页，旧响应不得覆盖新状态；真实 Host 与受控 fixture 验证同引用不同筛选并发、乱序、分页、重试以及来源/语言切换，多个排各自显示正确内容。
- Hero 取本周趋势前5部，8秒交叉淡入并缓慢推镜，悬停、获得焦点或窗口隐藏时暂停。
- 继续观看使用 `list_history` 和现有 `historyPoster`，横版卡带进度条。
- 首页依次为 Hero、继续观看、本周趋势电影与剧集、最新上线与热门电影、按类型（动作、喜剧、科幻、剧情，来自 `tmdb.top` 的 `genre`）、按年份（今年、去年，来自 `tmdb.year` 的 `genre`）、华语韩语日语精选；分类页默认依次为趋势、最新、今年、去年和该分类的类型排；首页和分类页的 `.home-card`、`.home-section` 等现有测试选择器保留。
- 新增 `BrowseView.vue` 和 `FilterBar.vue`，路由 `#/browse/<分类>?genre=…`，电影、电视剧、动画、纪录片四个分类共用该视图并接入侧栏。
- 筛选维度为类型、年份、语言、地区、排序，每维单选；已选排序后再选语言或年份时自动清除排序并提示；毛玻璃面板支持方向键；内容排标题进入该排的海报墙。

#### 页面风格规格（ADR-057）

**Hero。** 高 `min(72vh,760px)`，最小460px，铺满窗口顶部并位于侧栏之下。背景 `object-fit:cover`，焦点位置 `center 20%`。遮罩两层：自下而上 `--tv-bg` 到透明（占高度55%），自左向右 `rgba(11,11,13,.75)` 到透明（占宽度60%）。推镜8秒内缩放1至1.06。

**Hero 文案。** 左对齐内容左边距，底部距Hero 56px，最大宽560px，自上而下：角标（13/600，`--tv-text-2`）、片名、信息行（14px，`--tv-text-2`，用“·”分隔）、简介（15px，3行截断）、按钮组（上方20px）。主按钮高44px、最小宽120px、`--tv-button` 底、带播放图标；次按钮高44px、玻璃底。指示点位于右下，距边48px，圆点6px，当前项20×6px胶囊；点击切换。

**滚动。** Hero 随滚动以0.4倍速度位移，滚动到自身高度60%时透明度降至.25。第一条内容排上移32px，压在 Hero 渐隐区。

**继续观看。** 横版卡，底部内嵌3px进度条，轨道 `rgba(255,255,255,.3)`、填充 `--accent`；角标显示“第 3 集”，下方标题加“剩 12 分钟”。悬停时居中显示56px玻璃播放圆钮。

**首页竖版海报。** 无常驻文字叠加，仅悬停时放大；分类标签放在卡片下方的辅助信息。

**分类页头部。** 默认结构同 Hero，高 `min(56vh,560px)`；角标显示分类名。选中筛选后高度以300ms收缩到240px，隐藏简介和按钮，片名改为分类名。

**筛选栏。** 位于 Hero 下方，滚动到顶部后吸附在距窗口顶12px处，吸附时带玻璃底。胶囊高36px、水平内边距14px、14/500：默认 `--tv-surface-2`，悬停 `--tv-surface-hover`，已选 `--tv-button` 底加深色文字和关闭图标。右侧“清除全部”为13px文字按钮。

**筛选面板。** 玻璃底、圆角16px、内边距12px、宽360至480px、最大高360px并可滚动，出现时从胶囊处淡入并缩放.96至1，200ms。选项网格最小列宽96px，单项高36px、圆角10px，当前项右侧带 `--accent` 勾。

**海报墙。** 网格按竖版海报宽度自动填充，行间距28px、列间距16px，排序变化时淡入300ms。查找进度13px `--tv-text-2`，“继续查找”为36px玻璃胶囊。

**空状态。** 居中，图标40px，标题17/600，说明14px，下方列出“去掉某个条件”胶囊。

### T0.31 TV4 MacCMS分类与同源直通播放
- [x] 完成
- 依赖：T0.30
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-MACCMS-CAT, TV-MACCMS-PLAY, REQ-TV4 → A1（覆盖表含精确对应）
- 范围：`protocol/`, `src/models.ts`, `src-tauri/src/maccms.rs`, `src-tauri/src/matching.rs`, `src-tauri/src/host.rs`, `src-tauri/src/home.rs`, `src/state.ts`, `src/App.vue`, `src/views/BrowseView.vue`, `src/views/HomeView.vue`, `src/i18n.ts`, `src/style.css`, `src/components/`, `scripts/check-contracts.py`, `src/data/history.ts`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成MacCMS分类归一、综艺短剧体育入口及MacCMS影片同源播放。提供`tests/acceptance/tv4.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "输入：8个默认MacCMS来源的分类样本及受控播放fixture → 期望：不同来源的同义分类归一到同一子分类，成人向和非影片分类不显示，当前播放源缺失的分类隐藏，年份、地区、语言在客户端正确过滤；一部MacCMS影片完成详情、播放、切集、写入历史和续播，TMDB影片的跨源匹配行为不变。会被拒绝：出现排除分类、换源后入口指向无效分类、MacCMS影片无法写入历史、同源绑定放宽影响跨源校验。；计算样式核对本卡“页面风格规格”中的颜色、字号、尺寸、圆角、动效和状态，并保存960、1200、1512宽度截图供走查。会被拒绝：规格值被改动而未记录校准结果、状态样式缺失。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv4.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV4-all.json",
      "--junit",
      "evidence/acceptance/TV4-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV4-all.xml",
    "cases": [
      "TV-MACCMS-CAT",
      "TV-MACCMS-PLAY",
      "TV-MACCMS-CAT/normalize-names",
      "TV-MACCMS-CAT/excluded-classes",
      "TV-MACCMS-CAT/hide-missing",
      "TV-MACCMS-CAT/client-filter",
      "TV-MACCMS-PLAY/same-source-play",
      "TV-MACCMS-PLAY/history-resume",
      "TV-MACCMS-PLAY/stremio-binding-kept",
      "TV-MACCMS-CAT/visual-spec",
      "REQ-TV4"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- 内置分类映射表：国产剧、内地剧、大陆剧归为华语剧，`电视剧/国产剧` 这类带前缀的名称先取末段；综艺、动漫、短剧、体育、地区剧同样归一。
- 默认排除伦理片、福利、擦边短剧，以及预告片、资讯、公告、头条、电影解说。
- 综艺、短剧、体育入口绑定当前播放源，服务端只用分类 ID、分页和关键词；年份、地区、语言沿用5页扫描上限在客户端过滤（2026-11-08实测接口忽略 `year`、`area`）。
- `maccms.rs:1008` 生成 `namespace` 为 `maccms` 的 `metadata_ref`；`match_playback` 遇到同一 MacCMS 来源直接返回 `matched`。
- `state.ts:428`、`state.ts:435` 仅在同一 MacCMS 来源时放宽绑定；`App.vue` 续播逻辑接受 MacCMS 身份；`PlaybackContext` 和历史表结构保持。
- `state.ts:716` 的 `navigate` 同样放宽到同一 MacCMS 来源；`history.ts` 的 `historyPoster` 支持 MacCMS 身份，继续观看显示海报。
- MacCMS 分类声明类型、年份、语言、地区可用，排序不可用。

#### 页面风格规格（ADR-057）

**海报。** MacCMS 图片比例不一，统一 2:3 裁切，`object-fit:cover`，缺图样式同 T0.28。

**更新状态角标。** `vod_remarks` 显示在海报左下，距边8px，玻璃底、高22px、水平内边距8px、11/600、最大宽为卡片宽减16px并省略，例如“更新至第08集”。

**评分。** `vod_score` 为空或0时隐藏，其余沿用信息行样式。

**无横幅影片。** Hero 和详情背景使用海报模糊放大（40px、1.2倍），加 T0.30 的两层遮罩，右侧距边64px显示竖版海报，高为头部的60%，圆角12px。

**综艺、短剧、体育页。** 沿用分类页结构；子分类以横向胶囊排在筛选栏前，样式同筛选胶囊。短剧默认竖版海报，体育默认横版卡。

**来源提示。** 详情信息行末尾显示来源名，13px `--tv-text-2`；当前播放源缺失分类时，侧栏入口直接隐藏，不显示禁用态。

### T0.32 TV5 详情页、转场与待播清单
- [x] 完成
- 依赖：T0.30
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-DETAILS, TV-WATCHLIST, REQ-TV5 → A1（覆盖表含精确对应）
- 范围：`protocol/`, `src/models.ts`, `src/views/DetailsView.vue`, `src/views/HomeView.vue`, `src/App.vue`, `src/api.ts`, `src/state.ts`, `src/data/`, `src/style.css`, `src/i18n.ts`, `src-tauri/src/history.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/host.rs`, `src-tauri/src/contracts.rs`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/`, `src-tauri/build.rs`, `src/components/`, `scripts/check-contracts.py`, `src-tauri/tests/t05.rs`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成全幅详情页、页面转场和SQLite待播清单。提供`tests/acceptance/tv5.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "环境：真实打包窗口及临时数据目录 → 期望：详情页显示全幅背景、评分信息、剧照集卡、演员排和相关排，主按钮按历史显示继续播放及剩余时间；加入待播后首页出现该影片，重启仍保留，移除后消失，旧数据库升级不丢历史；海报到详情有转场，在禁用 View Transitions API 的夹具中回退交叉淡入（真实 macOS 14 记为 NOT RUN）。会被拒绝：续播信息与历史不一致、待播重启丢失、数据库升级破坏历史记录。；计算样式核对本卡“页面风格规格”中的颜色、字号、尺寸、圆角、动效和状态，并保存960、1200、1512宽度截图供走查。会被拒绝：规格值被改动而未记录校准结果、状态样式缺失。迁移失败时显式报错且旧库可用；`capabilities/main.json` 的权限集合必须等于 T0.28 结果加3项待播权限，CSP 与基线一致。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv5.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV5-all.json",
      "--junit",
      "evidence/acceptance/TV5-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV5-all.xml",
    "cases": [
      "TV-DETAILS",
      "TV-WATCHLIST",
      "TV-DETAILS/fullbleed-layout",
      "TV-DETAILS/resume-label",
      "TV-DETAILS/episode-cards",
      "TV-DETAILS/cast-related",
      "TV-DETAILS/transition-fallback",
      "TV-WATCHLIST/add-remove",
      "TV-WATCHLIST/persist-restart",
      "TV-WATCHLIST/migration-keeps-history",
      "TV-WATCHLIST/home-row",
      "TV-DETAILS/visual-spec",
      "TV-WATCHLIST/migration-failure",
      "TV-WATCHLIST/permissions-exact",
      "REQ-TV5"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- 用现有 `backdrop`、`actors` 和 `Bridge_Episode` 的 `poster`、`plot`、`duration_ms`，详情接口有 `logo` 时显示 Logo，否则显示大字片名。
- 保留 `.details-play`、`.details-episode`、`.details-group` 等现有测试选择器和匹配确认弹层。
- `history.rs` 数据库升到版本2，新增 watchlist 表，身份字段沿用 `watch_history`；迁移失败显式报错并保持旧库可用。
- 新增 `list_watchlist`、`add_watchlist`、`remove_watchlist` 三个 IPC 及对应权限。
- 海报飞入详情使用 View Transitions，不支持时300ms交叉淡入。
- `check-contracts.py` 的命令追加集增加 WATCHLIST，`t05.rs` 的命令总数改为26。

#### 页面风格规格（ADR-057）

**头部。** 高 `min(78vh,820px)`，最小520px，全幅背景，遮罩同 Hero，自下而上渐隐占高度65%。内容左下对齐，距底56px，最大宽620px。

**标题。** 有 Logo 时最大高120px、最大宽420px；否则用 Hero 片名字号。下方信息行依次为评分、年份、类型、时长、地区。

**简介。** 15px、最多3行；超长时末尾显示“更多”，打开玻璃弹层，弹层最大宽640px、圆角20px、内边距28px。

**操作区。** 主按钮高48px、最小宽160px、`--tv-button` 底，文字“播放”或“继续播放 · 第 3 集 · 剩 12 分钟”，进度存在时按钮底部内嵌3px `--accent` 进度。待播按钮为48px圆形玻璃，未加入显示加号，已加入显示勾，切换时图标淡入200ms。

**季标签。** 同筛选胶囊，当前季为已选样式，横向可滚动。

**集卡。** 横版，宽 `clamp(260px,24vw,380px)`，图片圆角12px；标题15/600“3. 标题”，简介13px两行，时长13px `--tv-text-2`。当前集显示 `--accent` 角标“正在观看”，已看过的集底部显示进度。无剧照时用背景图模糊版加集数大字。

**演员排。** 头像88px圆形，姓名13px居中两行，角色12px `--tv-text-2`；无头像时显示姓氏首字。

**信息区。** 三列网格，列间距32px；标签12px `--tv-text-2`，内容14px。窄到960px时两列。

**转场。** 海报飞入详情头部300ms，详情内容延后100ms淡入；返回时海报飞回并恢复滚动位置。

**匹配确认弹层。** 玻璃底、圆角20px、最大宽640px，候选项样式同季标签，操作区沿用现有按钮语义。

**待播清单（首页）。** 排在继续观看之后，竖版海报，空清单时整排隐藏。

### T0.33 TV6 沉浸播放器
- [x] 完成
- 依赖：T0.28
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-PLAYER, REQ-TV6 → A1（覆盖表含精确对应）
- 范围：`src/views/PlaybackView.vue`, `src/videojs.ts`, `src/style.css`, `src/i18n.ts`, `src/App.vue`, `src/ui/`, `src/state.ts`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成全窗口沉浸播放界面，播放内核和业务链路保持。提供`tests/acceptance/tv6.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "环境：真实打包窗口播放自有MP4与HLS → 期望：解析期间显示背景和片名，全窗口播放，控件和光标3秒无操作后隐藏，进度预览、前进后退10秒、字幕、音量、倍速、全屏可用；底部托盘切换选集和线路；片尾20秒显示可取消的下一集卡片；Esc按托盘、全屏、详情逐级返回；错误浮层可重试；50次切集后无残留媒体和内存泄露。会被拒绝：解析期间黑屏、同时存在两套控件、切集残留旧音频、Esc直接退出到首页。；计算样式核对本卡“页面风格规格”中的颜色、字号、尺寸、圆角、动效和状态，并保存960、1200、1512宽度截图供走查。会被拒绝：规格值被改动而未记录校准结果、状态样式缺失。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv6.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV6-all.json",
      "--junit",
      "evidence/acceptance/TV6-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV6-all.xml",
    "cases": [
      "TV-PLAYER",
      "TV-PLAYER/loading-backdrop",
      "TV-PLAYER/immersive-autohide",
      "TV-PLAYER/custom-skin-controls",
      "TV-PLAYER/tray-episodes-streams",
      "TV-PLAYER/next-episode-card",
      "TV-PLAYER/escape-levels",
      "TV-PLAYER/error-overlay",
      "TV-PLAYER/switch-recycle-50",
      "TV-PLAYER/visual-spec",
      "TV-PLAYER/resume-seek-race",
      "REQ-TV6"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- 用 Video.js 10.3.0 官方元素（控制层、时间滑块与预览、快进、字幕、缓冲指示、手势、热键）组合自定义皮肤，取代 `video-compat-skin`，内核、网关 Session、历史和原生全屏适配保持。
- 按 ADR-059 仅在恢复目标与当前媒体时间不同时执行定位；验证零位置暂停不会将取消中的播放请求误报为故障，非零恢复、时长限制及会话身份保持原契约。
- A1 的 `TV-PLAYER/resume-seek-race` 必须实际运行非零数量的回归测试，覆盖零位置暂停竞争、非零与超时长夹取、旧会话事件隔离，并在真实窗口验证当前播放可暂停且不产生错误；报告测试数及结果。
- `data-action` 业务按钮收进底部托盘并保留属性；状态文字改为视觉隐藏的无障碍文本。
- 同步修改 `tests/fixtures/videojs_controls.ts` 等绑定官方皮肤的夹具，断言强度保持。
- 键盘：左右10秒、上下音量、空格播放暂停、F全屏、M静音。

#### 页面风格规格（ADR-057）

**画面。** 背景 `#000`，视频 `object-fit:contain`，隐藏窗口其他界面。控件显示时底部覆盖高30%的黑色渐隐遮罩，透明度200ms过渡。

**加载态。** 背景图40px模糊并压暗到35%亮度，居中显示片名（28/700）、集名（15px）和28px转圈指示；解析超过10秒时显示“正在查找可用线路”。

**顶部。** 左上距边24px：44px圆形玻璃返回钮，右侧片名17/600、集名13px `--tv-text-2`，避开红绿灯（全屏外向下留28px）。

**控制条。** 左右和底部各距24px，玻璃底、圆角20px、内边距12px 20px，分两行。

**进度条。** 轨道高4px、悬停8px，圆角满，轨道 `rgba(255,255,255,.3)`，缓冲 `rgba(255,255,255,.5)`，已播 `--tv-text`；拖动点14px，仅悬停和拖动时显示。预览时间为玻璃胶囊，12px。两端时间13px等宽数字，右端显示剩余时间。

**按钮。** 点击区44px、图标24px，悬停圆形 `--tv-surface-hover`。左组为后退10秒、播放暂停（图标28px）、前进10秒；右组为字幕、音量、倍速、选集托盘、全屏。音量滑杆悬停时展开到96px。

**底部托盘。** 从控制条位置上滑，高 `min(46vh,420px)`，玻璃底、顶部圆角20px，过渡300ms；顶部三个标签（选集、线路、字幕与音频）使用筛选胶囊样式。选集为横版集卡；线路为44px行，显示线路名和清晰度，当前项带 `--accent` 勾，不可播放项用 `--tv-text-3` 并显示原因。

**下一集卡片。** 右下距边24px、控制条上方，宽300px，玻璃底、圆角16px，含16:9剧照、“下一集”12/600、集名14/500、“立即播放”和“取消”。片尾20秒显示卡片；开启自动下一集时，真实 EOS 后显示5秒倒计时环再切集，取消后留在当前集；关闭时只显示按钮。

**字幕。** 白色28px（宽度不小于1440px时34px），2px黑色描边阴影，距底8%；控件显示时上移到控制条上方，过渡200ms。

**错误浮层。** 居中玻璃面板、宽420px、圆角20px，错误标题17/600、说明14px，主按钮“重试”，次按钮“换线路”。

**缓冲。** 居中40px转圈，延迟300ms显示，避免短暂闪烁。

### T0.34 TV7 搜索、资料库、来源与设置
- [x] 完成
- 依赖：T0.31, T0.32
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：TV-SEARCH, TV-LIBRARY, TV-MANAGE, REQ-TV7 → A1（覆盖表含精确对应）
- 范围：`protocol/`, `src/models.ts`, `src/App.vue`, `src/api.ts`, `src/state.ts`, `src/views/SearchView.vue`, `src/views/HistoryView.vue`, `src/views/NativeSourcesView.vue`, `src/views/SettingsView.vue`, `src/style.css`, `src/i18n.ts`, `src-tauri/src/stremio.rs`, `src-tauri/src/maccms.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/host.rs`, `src-tauri/src/contracts.rs`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/`, `src-tauri/build.rs`, `src/components/`, `scripts/check-contracts.py`, `src-tauri/tests/t05.rs`, `tests/`, `docs/PROGRESS.md`
- 交付物：完成搜索页、资料库和来源设置页的样式统一。提供`tests/acceptance/tv7.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C17中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "环境：真实打包窗口及受控搜索fixture → 期望：⌘F打开搜索，输入停顿300ms后按电影、电视剧、综艺短剧分排显示结果，旧请求结果被丢弃；空状态显示类型卡片；资料库分继续观看、待播清单、观看历史，删除和清空行为保持；来源和设置页改为分组列表且全部原有操作可用。会被拒绝：过期搜索结果覆盖新结果、搜索绕过请求取消、来源或设置功能缺失。；计算样式核对本卡“页面风格规格”中的颜色、字号、尺寸、圆角、动效和状态，并保存960、1200、1512宽度截图供走查。会被拒绝：规格值被改动而未记录校准结果、状态样式缺失。`capabilities/main.json` 的权限集合必须等于 T0.32 结果加搜索权限，CSP 与基线一致。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/tv7.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/TV7-all.json",
      "--junit",
      "evidence/acceptance/TV7-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/TV7-all.xml",
    "cases": [
      "TV-SEARCH",
      "TV-LIBRARY",
      "TV-MANAGE",
      "TV-SEARCH/debounce-cancel",
      "TV-SEARCH/grouped-results",
      "TV-SEARCH/empty-genres",
      "TV-LIBRARY/three-tabs",
      "TV-LIBRARY/history-actions-kept",
      "TV-MANAGE/grouped-lists",
      "TV-MANAGE/actions-kept",
      "TV-SEARCH/visual-spec",
      "TV-SEARCH/permissions-exact",
      "REQ-TV7"
    ],
    "timeout_seconds": 3600
  }
]
```
#### 工作义务（ADR-057）

- 新增 `search_catalog` IPC 及权限，沿用 request_id、engine_epoch、预算和取消规则；Stremio 用声明 `search` 的目录，MacCMS 用 `wd`。
- `stremio.rs:469` 的 `search` 能力按清单实际声明填写。
- 新增 `SearchView.vue`，接入侧栏和 ⌘F。
- `HistoryView.vue` 升级为资料库三标签，历史分组、删除、清空逻辑复用。
- 来源和设置页只改样式，字段、校验和操作保持。
- `check-contracts.py` 的命令追加集增加 `search_catalog`，`t05.rs` 的命令总数改为27。

#### 页面风格规格（ADR-057）

**搜索页。** 无 Hero，页面顶部距窗口72px；输入框高56px、最大宽720px、圆角16px、`--tv-surface-2`、文字20px，左侧搜索图标，有内容时右侧清除按钮，聚焦时1px `--tv-hairline` 变为 `--tv-text`。

**搜索空状态。** 类型卡片网格，最小列宽200px、高96px、圆角14px、`--tv-surface-1`，类型名17/600左下对齐，悬停放大1.04。

**搜索结果。** 按电影、电视剧、综艺短剧分排，沿用 `MediaRow`；加载中保留上一次结果并降低透明度到.6；无结果时居中提示，样式同 T0.30 空状态。

**资料库。** 页面标题34/700，距窗口顶72px；下方三段式分段控件，高36px、玻璃底、圆角满，当前段 `rgba(255,255,255,.18)`。内容为海报墙，继续观看用横版卡。

**观看历史。** 按今天、昨天、更早分组，组标题17/600，吸附时带玻璃底；悬停卡片右上显示28px圆形玻璃删除钮；“清空”为13px `--tv-danger` 文字按钮，确认弹层用玻璃样式。

**来源与设置页。** 内容最大宽720px并居中，顶部距窗口72px，页面标题34/700。分组标题13/600 `--tv-text-2`，下方8px；分组卡片 `--tv-surface-1`、圆角14px；行最小高52px、内边距12px 16px，行间分隔线左缩进16px，左侧标题15px、说明13px，右侧控件。

**表单控件。** 输入框和下拉高36px、圆角10px、`--tv-surface-2`；开关沿用 Naive 组件，开启色为 `--accent`；添加来源使用玻璃弹层。

**来源状态。** 行首8px圆点：可用绿色 `#30d158`，禁用 `--tv-text-3`，失败 `--tv-danger`；错误说明13px显示在该行内。

**色系设置。** 五个32px圆形色块，当前项带2px `--tv-text` 外环；语言使用分段控件。

### T0.35 T24 独立验收与问题修复
- [x] 完成
- 执行阶段：产品测试；承接T0.2两格式各600秒及5次窗口开关（ADR-044），按当前受验源码/构建重新执行；复用现有原生夹具、长播断言及资源清理，历史短测与失败只作历史证据。
- 依赖：T0.33, T0.34
- 需求：U01, U04, R30, R32, R48, R49, REQ-T01, U06, U07, R01, R07, R50, R51, R57, REQ-T24 → A1（覆盖表含精确对应）
- 范围：`src/`, `src-tauri/`, `packaging/`, `docs/release-evidence/`, `tests/`, `docs/PROGRESS.md`
- 音频采样（ADR-060）：按用户明确决定，本卡关闭原生及页面音频采样，音频采样与测量字段留空或 null，标记 SKIPPED_USER_AUTHORIZED；A1 和独立报告写明音频实际表现未验证，G1 结论限定于非音频义务。轨道、AudioContext、会话等非采样资源清理仍须检查，其他验收条件保持。
- 安装生命周期（ADR-058）：干净系统安装、初始化、首播、升级、卸载由 T0.36/A2 执行，本卡的真实运行证据核对不包含这部分。
- 交付物：落实下列修订义务；提供`tests/acceptance/t24.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 独立验收：本卡必须由独立只读审阅者审查固定构建及受验提交，作者按报告修复后再次复审。审阅原始报告保存于`docs/reviews/T24/round-<N>.md`（随文档提交），正文写明所审提交SHA与源码摘要，末行`ACCEPTANCE-VERDICT: CLEAR`或`ACCEPTANCE-VERDICT: BLOCKING`；t24.py只读取N数值最大的轮次，提交/摘要与本次构建不一致、末行缺失或为BLOCKING时失败。 本卡先经原run执行同源码/构建/protected/generation的t24-endurance前置实跑并保存原始样本，payload在原日志输出证据manifest SHA以由收据log_hash绑定；独立只读报告复核本批原始证据及固定WIP构建，记录文件SHA，再执行A1完整重算同批所有长播/生命周期断言并核对原收据/日志/manifest/独立报告，拒绝仅passed摘要、缺失或改写证据及旧源码/基线。报告允许先于最终A1，绑定受验构建提交与源码；其后仅docs提交保持证据绑定。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "本产品测试阶段先执行T0.2移交的真实1080p30 MP4与HLS各600秒、5次完整窗口开关及帧/UI/资源清理（ADR-044，音频采样按 ADR-060 跳过），以当前源码/构建实测；缺少、失败或沿用旧源码时失败。随后独立只读验收者逐修订U01..U09（U07首版结构）及G0-Core/G1..G5检查当前提交，复算Rust来源支持清单与平台真实运行安装媒体/数据/历史证据，行为复测高影响问题清零；作者修复后再次独立复审。核对当前仅个人使用，公开发行未获授权；缺独立结论/未解决阻断/复制旧报告时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t24.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T24-all.json",
      "--junit",
      "evidence/acceptance/T24-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T24-all.xml",
    "cases": [
      "U01",
      "U04",
      "R30",
      "R32",
      "R48",
      "R49",
      "REQ-T01",

      "U06",
      "U07",
      "R01",
      "R07",
      "R50",
      "R51",
      "R57",
      "REQ-T24"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 依赖：T23。
- 修改：必要修复与最终证据。
- 工作：按ADR-048修订U01至U06及U08/U09、G0-Core/G1至G5完整审查，并检查U07的移动公共入口/平台隔离/响应式布局与实施计划；核对独立实现依据、核准来源清单、媒体代理、实际平台运行与安装包。
- 通过：每个硬条件有证据；已知失败可准确归因；文档与最终实现一致；高影响问题清零。
- 验证：按行为复测，避免仅复制实现细节；结论包含实际测试命令和结果。
- 执行：可由独立审查步骤或明确授权的审查agent执行，避免开发者自行省略关键验收。

### T0.37 已播放线路记忆
- [x] 完成
- 依赖：T0.35
- 环境：macos-arm64 必需；其他桌面平台按既有矩阵记录。
- 启动次序：C5 修订生效并经最终独立文档 CLEAR 前，仅允许原 task.py start 建立本卡状态；实现与验收须等待契约修订和基线完成。
- 需求：REQ-LINE-PREFERENCE → A1
- 范围：`src/`, `src-tauri/`, `tests/`, `docs/PROGRESS.md`
- 交付物：为每部影片记住实际播放成功的来源及可跨剧集识别的线路；下一集及其他集优先使用该影片上次成功线路，应用重启后保留。用户主动选择优先；线路缺失、来源失效或不可用时沿现有匹配/选择流程继续。记录不得包含播放 URL、令牌、临时 Session、Resolution 或 Stream 标识，影片及来源修订之间严格隔离。仅在来源修订一致、内容匹配唯一可信、当前目标集及线路有唯一可靠对应且可播放时自动选择；线路重排、同名、缺集、乱序、跨季或同名候选导致无法确认时保留原确认流程，不以旧 group_id 或数组下标单独证明对应；来源 revision 和内容 ID 未变仍须用首次确认时的非敏感稳定内容证据交叉核验当前快照，字段冲突或无法证明时回确认。沿用现有公开协议和命令，复用现有持久化机制；读取损坏、权限/配额或写入失败时继续当前播放及原匹配流程，保留既有已确认记录，不伪报保存成功，并提供可定位错误；恢复可写后允许再次保存，迟到结果不得覆盖新选择。若实现需要协议扩展，先按冻结流程明确最小变更并复审。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实入口至少提供三条线路，首集前两条失败或不可用、第三条实际解码并推进后才记忆；第二、三集和应用重启后根据当前数据匹配并优先第三条，证明前两条未被优先重试且未复用旧 Session/Resolution/Stream。写入只由当前身份及会话的实际播放推进触发，resolve 成功或 playable 声明不足以写入。验证手动选择覆盖和抢占、不同影片隔离、缺失/失效时回退、失败或过期请求不得覆盖已成功选择、来源修订变化、存储损坏/不可用/恢复及迟到读写结果、敏感临时数据不落盘；真实入口覆盖线路重排、同名、缺集、乱序、跨季、同名候选及同 revision/同 vod_id 内容替换，无法可靠对应时保留确认。至少通过生产 MacCMS 适配器、真实 Host 和窗口，使用原格式 vod_play_from/vod_play_url 的同影片三线路多集数据，验证首次用户确认后的第2/3集唯一可靠映射，包括生产形态中 season/number 为 null、可唯一解析并与元数据交叉核对的标签；无号、重复号、跨季歧义或相似伪造标签无法证明时保留确认。同时以真实同源 MacCMS 场景验证换线后后续集、重启、历史 video 身份和旧会话安全，重排/歧义回确认或原流程。以当前源码覆盖匹配→播放→切集→历史及手动选择的相关回归，保留现有播放与匹配安全约束。记录 T0.35 的固定构建证据属于变更前提交，禁止声称其验证本卡新源码。",
    "argv": ["{python}", "-B", "tests/acceptance/line_preference.py", "--report", "evidence/acceptance/LINE-PREFERENCE.json", "--junit", "evidence/acceptance/LINE-PREFERENCE.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/LINE-PREFERENCE.xml",
    "cases": ["REQ-LINE-PREFERENCE", "REQ-LINE-PREFERENCE/three-lines", "REQ-LINE-PREFERENCE/later-episodes", "REQ-LINE-PREFERENCE/restart", "REQ-LINE-PREFERENCE/races", "REQ-LINE-PREFERENCE/identity-and-fallback", "REQ-LINE-PREFERENCE/storage-and-secrets", "REQ-LINE-PREFERENCE/playback-history", "REQ-LINE-PREFERENCE/same-source"],
    "timeout_seconds": 1800
  }
]
```

### T0.38 X1 设置页重组与通用TMDB（Stremio方式）
- [x] 完成
- 依赖：T0.37
- 环境：macos-arm64 必需。
- 需求：REQ-X03, REQ-X04, REQ-X05, REQ-X06 → A1
- 范围：`src/`, `src-tauri/`, `protocol/`, `tests/`, `scripts/check-contracts.py`（Settings 契约允许项）, `docs/PROGRESS.md`, `docs/BACKLOG.md`, `evidence/manual/`
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（D2、D5、T2.1）
- 交付物：设置页「语言与元数据」加入通用 TMDB 设置：启用开关、方式（本卡只实现 Stremio 扩展）、实例地址（默认 elfhosted，可修改）、语言（默认跟随应用）；manifest 地址按语言生成；其他自定义 Stremio 扩展归入本节。「播放设置」并入原来源管理：当前元数据源与播放源选择、统一播放源列表（已添加的可编辑、启用/禁用、删除；未添加的预设排在后面并带「添加」，同一 API 地址只出现一次）、可折叠诊断。按 ADR-065 不做旧版本兼容与迁移：`defaultSources.json` 删除 TMDB Stremio 条目，默认设置启用新 TMDB 设置，删除无用的 `metadata_preset_applied`，删除 `#/sources` 路由与 sources/setup 页面及左侧「来源管理」入口。新 TMDB 源使用固定标识，切换语言后观看记录仍可打开。开工前先实测 elfhosted 非中文语言路径能否返回 manifest，结果写入 PROGRESS。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "全新设置启动 → 首页显示 TMDB 内容且语言与应用一致；切换应用语言 → manifest 地址随之变化，手动指定语言时保持不变；切换 TMDB 语言后已有观看记录仍能打开详情。播放设置中预设与已添加来源合并为一个列表，添加预设后不出现重复，编辑/启用/禁用/删除生效。会被拒绝：语言写死、预设与已添加来源重复显示、仍可通过 #/sources 或左侧菜单进入旧来源管理。",
    "argv": ["{python}", "-B", "tests/acceptance/x1_settings_tmdb.py", "--report", "evidence/acceptance/T0.38-A1.json", "--junit", "evidence/acceptance/T0.38-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.38-A1.xml",
    "cases": ["REQ-X03/playback-settings", "REQ-X04/merged-list", "REQ-X05/tmdb-language", "REQ-X06/stremio-mode", "REQ-X05/history-stable"],
    "timeout_seconds": 1800
  }
]
```

### T0.39 X2 左侧菜单：本地视频与直播
- [x] 完成
- 依赖：T0.38
- 环境：macos-arm64 必需。
- 需求：REQ-X01, REQ-X02 → A1
- 范围：`src/`, `src-tauri/`, `protocol/`, `tests/`, `scripts/check-contracts.py`（Settings 契约允许项）, `docs/PROGRESS.md`, `evidence/manual/`
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（D1、T2.2）
- 交付物：桌面侧栏在「资料库」前加入「本地视频」`#/local`（一直显示）与「直播」`#/live`（存在已启用的直播配置时显示）；移动端底栏同步。菜单显示条件需要可保存的直播配置，故本卡提前交付 T0.40 的设置字段 `live_configs: [{config_id, name, url, epg_url?, enabled}]`（仅数据与校验，列表下载、解析及设置页管理仍由 T0.40 交付）；`#/local`、`#/live` 页面内容由 T0.40、T0.42 交付，本卡提供可进入的空页面。焦点导航能到达新菜单项。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "无直播配置 → 侧栏有「本地视频」、无「直播」；加入并启用一条直播配置 → 「直播」立即出现；全部禁用或删除 → 消失。键盘/遥控焦点可依次到达新菜单项并进入对应页面。会被拒绝：「本地视频」随配置隐藏、禁用后「直播」仍显示、新项无法获得焦点。",
    "argv": ["{python}", "-B", "tests/acceptance/x2_navigation.py", "--report", "evidence/acceptance/T0.39-A1.json", "--junit", "evidence/acceptance/T0.39-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.39-A1.xml",
    "cases": ["REQ-X01/local-always", "REQ-X02/live-conditional", "REQ-X02/focus"],
    "timeout_seconds": 1200
  }
]
```

### T0.40 X3 直播（IPTV与直播源合并）
- [x] 完成
- 依赖：T0.39
- 环境：macos-arm64 必需。
- 需求：REQ-X02 → A1
- 范围：`src/`, `src-tauri/`, `protocol/`, `tests/`, `scripts/check-contracts.py`（新增命令契约允许项）, `docs/PROGRESS.md`, `evidence/manual/`
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（D1、T2.3）
- 交付物：设置新增 `live_configs: [{config_id, name, url, epg_url?, enabled}]`，在设置页「直播」一节管理。新增 Rust `live.rs` 复用现有 requests 下载列表，按内容自动识别 M3U/M3U8（`#EXTINF`、`group-title`、`tvg-logo`）与 TXT（`频道名,地址`、`#genre#` 分组），输出统一频道列表（名称、分组、台标、地址、是否支持）；新增命令在 capabilities 登记。直播页左侧分组与频道，右侧播放器。播放复用 `gateway.open` 并新增直播模式：不进媒体缓存、m3u8 清单每次重新拉取、不写观看记录、不显示进度条。只支持 http/https 与 HLS，`rtp://`、`udp://`、`rtmp://` 标为不支持。`epg_url` 只保存不解析。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "M3U 与 TXT 夹具 → 各自解析出预期分组、频道数、台标及不支持标记；本机 HLS 直播夹具经网关播放 → 清单持续刷新、媒体缓存与观看记录均无写入；列表下载失败或格式无法识别 → 明确报错。会被拒绝：两种格式需手动选择、直播分片写入缓存、直播写入观看记录、不支持的协议被尝试播放。",
    "argv": ["{python}", "-B", "tests/acceptance/x3_live.py", "--report", "evidence/acceptance/T0.40-A1.json", "--junit", "evidence/acceptance/T0.40-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.40-A1.xml",
    "cases": ["REQ-X02/m3u", "REQ-X02/txt", "REQ-X02/unsupported-protocol", "REQ-X02/live-gateway", "REQ-X02/no-history", "REQ-X02/errors"],
    "timeout_seconds": 1800
  }
]
```

### T0.41 X4 通用TMDB：官方API方式
- [x] 完成
- 依赖：T0.38
- 环境：macos-arm64 必需；真实 TMDB 网络可达性按当时网络记录。
- 需求：REQ-X06 → A1
- 范围：`src/`, `src-tauri/`, `protocol/`, `tests/`, `docs/PROGRESS.md`, `evidence/manual/`
- 决定：Q1 用户于 2026-11-11 聊天选择「Settings file」，API Key 随设置文件保存。
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（D2、T2.4、Q1）
- 交付物：TMDB 设置新增「TMDB 官方 API」方式：API Key（支持 v3 Key 与 v4 访问令牌）、可修改的 API 地址与图片地址。新增 Rust `tmdb.rs` 输出与现有 Stremio 数据源相同的首页、详情、搜索数据，图片经网关，复用 `matching.rs` 匹配播放源。两种方式的内容标识统一为 `tmdb:<id>`。Key 存放位置按用户对 Q1 的决定实现，开工前确认。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "TMDB API 夹具服务 → 首页、详情、搜索正常且图片经网关；同一影片在 API 与 Stremio 方式下标识一致，切换方式后观看记录可打开；自定义 API 地址生效；Key 无效 → 明确错误。会被拒绝：Key 出现在日志、诊断报告或观看记录中，两种方式产生不同内容标识。",
    "argv": ["{python}", "-B", "tests/acceptance/x4_tmdb_api.py", "--report", "evidence/acceptance/T0.41-A1.json", "--junit", "evidence/acceptance/T0.41-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.41-A1.xml",
    "cases": ["REQ-X06/api-catalog", "REQ-X06/shared-identity", "REQ-X06/custom-endpoint", "REQ-X06/invalid-key", "REQ-X06/secret-redaction"],
    "timeout_seconds": 1800
  }
]
```

### T0.42 X5 本地视频（mp4/mov/m4v）
- [x] 完成
- 依赖：T0.39
- 环境：macos-arm64 必需。
- 需求：REQ-X01 → A1
- 范围：`src/`, `src-tauri/`, `protocol/`, `tests/`, `scripts/check-contracts.py`（新增命令与设置契约允许项）, `docs/PROGRESS.md`, `evidence/manual/`
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（T2.5）
- 交付物：新增依赖 `tauri-plugin-dialog`，选择文件或文件夹加入本地库，路径保存在设置中。网关新增 `file` 资源复用现有 Range 逻辑，只读取本地库内路径。观看记录以文件路径哈希为 `content_id`。WebView 无法解码的格式显示「格式不支持」。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "受控 mp4/mov 样本加入本地库 → 播放与拖动正常；重启后本地库和续播位置保留；文件移动或删除 → 明确错误；mkv 在 T0.44 前 → 显示格式不支持。会被拒绝：网关可读取本地库以外的路径、拖动重新从头读取整个文件、文件缺失时静默空白。",
    "argv": ["{python}", "-B", "tests/acceptance/x5_local_video.py", "--report", "evidence/acceptance/T0.42-A1.json", "--junit", "evidence/acceptance/T0.42-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.42-A1.xml",
    "cases": ["REQ-X01/play-and-seek", "REQ-X01/persist", "REQ-X01/missing-file", "REQ-X01/path-scope", "REQ-X01/unsupported-format"],
    "timeout_seconds": 1800
  }
]
```

### T0.43 X6 MKV可行性实验（ffmpeg-next）
- [x] 完成
- 依赖：T0.37
- 环境：macos-arm64 必需。
- 需求：REQ-X07 → A1
- 范围：`experiments/mkv-remux/`, `tests/`, `docs/DECISIONS.md`, `docs/PROGRESS.md`
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（D3、T2.6、Q2）
- 交付物：独立实验 crate，不改产品代码。使用 `ffmpeg-next` 的 `build` feature（LGPL，不开 `build-license-gpl`），核实：能否只编译 matroska/mp4/hls 封装与解封装及少量音频编解码器；体积增量；macOS arm64 构建耗时及所需工具（clang、pkg-config、是否需要 nasm）；H.264+AAC 与 HEVC+DTS 样本重新封装为 fMP4 HLS 后能否在 WKWebView 播放。结论及 Q2（静态链接或 dylib）建议追加为 ADR。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "实验 crate 在本机构建 → 输出体积、耗时、启用组件清单；两个受控 MKV 样本重新封装 → 产物通过 ffprobe 或等效检查且在 WKWebView 实际播放；报告列出许可证结论。会被拒绝：依赖系统 brew ffmpeg、开启 GPL 组件、只报告构建成功而无播放结果。",
    "argv": ["{python}", "-B", "tests/acceptance/x6_mkv_spike.py", "--report", "evidence/acceptance/T0.43-A1.json", "--junit", "evidence/acceptance/T0.43-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.43-A1.xml",
    "cases": ["REQ-X07/static-build", "REQ-X07/size-report", "REQ-X07/remux-h264-aac", "REQ-X07/remux-hevc-dts", "REQ-X07/license"],
    "timeout_seconds": 7200
  }
]
```

### T0.44 X7 MKV支持
- [x] 完成
- 依赖：T0.42, T0.43
- 环境：macos-arm64 必需。
- 阶段门：T0.43 的 ADR 结论为可行，并由用户决定 Q2（FFmpeg 分发方式）与 Q3（是否覆盖网络来源 MKV）
- 需求：REQ-X07 → A1
- 范围：`src-tauri/`, `src/`, `licenses/`, `tests/`, `packaging/release.py`（Frameworks 中的 FFmpeg 动态库与许可证）, `.cargo/config.toml`（FFMPEG_DIR）, `protocol/`, `docs/PROGRESS.md`
- 决定：ADR-067（FFmpeg 动态库放入 .app；只覆盖本地文件）。
- 计划文档：`docs/FOLLOWUP-LOCAL-LIVE-TMDB.md`（D3、T2.7）
- 交付物：网关新增 `remux.rs`，用 libavformat 读取 MKV，视频与音频轨道直接复制并输出 fMP4 分片 HLS；可直接复制 H.264/HEVC 视频及 AAC/MP3/AC3/E-AC3/FLAC/Opus 音频；DTS/TrueHD 等转 AAC；内嵌 SRT/ASS 字幕提取后交 `subtitle.rs` 转 VTT；按 Cues 建立分片索引支持拖动。更新第三方许可证清单。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "大于 4 GB 的受控 MKV → 播放与拖动正常；DTS 样本有声音；内嵌字幕可切换；停止播放 → FFmpeg 相关句柄与临时数据全部释放；视频编码不受支持 → 明确错误。会被拒绝：先整体转码再播放、拖动后从头处理、停止后残留句柄、许可证清单缺少 FFmpeg。",
    "argv": ["{python}", "-B", "tests/acceptance/x7_mkv.py", "--report", "evidence/acceptance/T0.44-A1.json", "--junit", "evidence/acceptance/T0.44-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.44-A1.xml",
    "cases": ["REQ-X07/large-file-seek", "REQ-X07/audio-transcode", "REQ-X07/embedded-subtitles", "REQ-X07/cleanup", "REQ-X07/unsupported-codec", "REQ-X07/notices"],
    "timeout_seconds": 3600
  }
]
```

### T0.45 X8 近似搜索
- [x] 完成
- 依赖：T0.44
- 环境：macos-arm64 必需。
- 需求：REQ-X08 → A1
- 范围：`src-tauri/`, `src/`, `tests/`, `docs/PROGRESS.md`, `evidence/manual/`
- 交付物：MacCMS 来源的 `wd` 只做子串匹配。搜索先按输入原样查询；无结果时依次尝试规范化（去空格标点、全角转半角）、去掉「第 X 季/部/集」与末尾续集编号、取片名前段；仍无结果时用内置 TMDB 来源搜到的中文/原名再查 MacCMS。结果按与输入的接近程度排序。搜索页在没有任何结果包含输入文字时提示「没有完全匹配，已显示相近结果」。不做错别字纠正与繁简转换。不改桥接契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "子串匹配的 MacCMS 夹具 → 「复仇者联盟 4」「庆余年 第二季」经放宽找到，原样能找到时不放宽；「流浪地球」最接近的排在前；内置 TMDB 就绪后「Inception」找到「盗梦空间」，未就绪时为空；不存在的片名为空；搜索页只在结果不含输入时提示。会被拒绝：原样有结果仍放宽、结果顺序不按接近程度、TMDB 不可用时报错中断、精确结果也显示提示。",
    "argv": ["{python}", "-B", "tests/acceptance/x8_search.py", "--report", "evidence/acceptance/T0.45-A1.json", "--junit", "evidence/acceptance/T0.45-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.45-A1.xml",
    "cases": ["REQ-X08/relaxed-queries", "REQ-X08/ranking", "REQ-X08/tmdb-alias", "REQ-X08/approximate-hint"],
    "timeout_seconds": 1800
  }
]
```

### T0.47 播放服务一：元数据服务
- [ ] 完成
- 依赖：T0.45
- 阶段门：用户另行明确要求开始播放服务开发；本次仅修订与复审文档（ADR-078）。
- 环境：当前 macos-arm64 App；服务端与受控上游以本机 Docker 运行，沿用资源执行器并清理自建容器/临时数据。配置的局域网服务用代表性连接样本验证。
- 需求：REQ-SERVICE-META → A1, A2
- 范围：`Cargo.toml`（新增 workspace）, `Cargo.lock`, `crates/`, `server/`, `src-tauri/`, `src/`, `protocol/`, `tests/`, `packaging/`, `licenses/`, `docs/runtime.lock.json`, `docs/PROGRESS.md`
- 交付物：按 `docs/FOLLOWUP-PLAYBACK-SERVICE.md` 第 2.2、3、4、5.1、5.2、5.7、7.3、7.5、8.1、9、10 节实现。建立 workspace 与 `moc-core`，按本期实际调用迁入共用代码，保持现有播放。服务提供 axum/SQLite、持久令牌及环境变量优先级、health、TMDB 与自定义 Stremio 元数据登记/管理/配置导入、按元数据来源隔离的 manifest/catalog/meta 路由、原始片名和别名、缓存、首次无快照生成及每日首页（含综艺）、失败保留旧快照。App 服务访问支持本机/局域网地址，令牌限定配置 origin 和基础路径，本地服务域名可用系统 DNS；来源/媒体沿 ADR-068 允许内网、回环和 Fake-IP，保留 TLS 与跨 origin 凭据处理。App 增加服务设置，保留元数据来源选择与语言，适配自建服务语言和目录，维持当前 TMDB 与自定义 Stremio 的逻辑身份、历史和收藏；服务地址/令牌变化只更新传输配置。未连接服务时内置 TMDB 保留到第二期结束。提供 Dockerfile、反向代理日志关闭/脱敏示例、关键接口的 OpenAPI 文档及新增依赖许可记录。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "本机 Docker 服务接受控 TMDB 与一个自定义 Stremio 元数据样本：目录/搜索/分页/详情与关键字段正确，原始片名和别名可用于匹配，来源同名 ID 与语言缓存互相隔离；用可控时间验证缓存命中和过期、首次无快照后生成、综艺目录及更新失败保留旧快照。检查错误令牌被拒、自动令牌重启复用、环境变量优先，以及样例 App/服务/代理输出脱敏；本机/局域网服务可达，来源及私网媒体按 ADR-068 接受；令牌请求跨 origin 跳转被拒，普通跳转清除敏感头。workspace 构建与受拆分影响的既有相关检查通过，记录并清理自建容器。复用场景，校验关键接口即可。",
    "argv": ["{python}", "-B", "tests/acceptance/playback_service.py", "--suite", "meta-server", "--report", "evidence/acceptance/T0.47-A1.json", "--junit", "evidence/acceptance/T0.47-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.47-A1.xml",
    "cases": ["REQ-SERVICE-META/metadata-and-language", "REQ-SERVICE-META/cache-and-home", "REQ-SERVICE-META/token-and-redaction", "REQ-SERVICE-META/configured-endpoints", "REQ-SERVICE-META/workspace"],
    "timeout_seconds": 1800
  },
  {
    "id": "A2",
    "description": "一次本机 App 关键操作检查（可自动化，也可由验收入口记录实际操作）：保存本机服务地址与令牌，浏览 TMDB 和一个 Stremio 元数据源的目录/搜索/详情，切换语言；首期沿原播放流程短时播放并续播接入前保存的 TMDB 记录，收藏保持一份，自定义 Stremio 内容身份保留；更改服务地址或令牌后仍指向原内容。观察首次准备首页、错误鉴权和服务不可达的提示，未配置服务时内置 TMDB 可用；检查一次诊断脱敏。未完成的操作如实报告。",
    "argv": ["{python}", "-B", "tests/acceptance/playback_service.py", "--suite", "meta-app", "--report", "evidence/acceptance/T0.47-A2.json", "--junit", "evidence/acceptance/T0.47-A2.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.47-A2.xml",
    "cases": ["REQ-SERVICE-META/app-browse", "REQ-SERVICE-META/current-records", "REQ-SERVICE-META/app-errors-and-fallback"],
    "timeout_seconds": 2400
  }
]
```

### T0.48 播放服务二：播放服务与App选线
- [ ] 完成
- 依赖：T0.47
- 环境：当前 macos-arm64 App；服务和受控来源以本机 Docker 运行，沿用资源执行器并清理自建资源。
- 需求：REQ-SERVICE-PLAY → A1, A2
- 范围：`Cargo.toml`, `Cargo.lock`, `crates/`, `server/`, `src-tauri/`, `src/`, `protocol/`, `tests/`, `licenses/`, `docs/runtime.lock.json`, `docs/PROGRESS.md`
- 交付物：按方案第 5.3–5.8、6.1–6.6、7.1–7.3、7.5、8.2–8.5、9、10 节实现。服务端接入 MacCMS JSON/XML，两次片名查询与两级筛选，未知类型需确认、标题年份冲突排除、唯一满足条件者自动确认；对应关系按元数据来源/类型/影片/播放来源保存，正常增集继续复用。实现 NDJSON 查询、确认/撤销/取消、短期缓存、句柄有效期、刷新与统计、来源管理与本期类型配置导入；第三期类型列入跳过清单。App 原生轮次保存地址，查询和待测均结束或整轮到期后自动选最快可播线路，手动可提前选择；确认/取消/切集丢弃迟到结果，过期重查且保留进度，失败按线路刷新一次再换线，上报失败仅影响统计。移除全局播放源、MacCMS 元数据和同源直通、App 内跨源匹配与线路记忆、短剧/体育入口及内置 TMDB 直连（Q5），保留服务 TMDB/Stremio 元数据选择与当前历史/收藏。来源上传按稳定 ID 幂等保存，成功项才移除本地配置。关键接口同步 OpenAPI，受影响的已有检查随行为调整并简记原因；普通媒体继续原安全规则。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "少量受控 JSON/XML 来源覆盖两次片名查询、全文结果过滤、衍生视频/季集错误/标题年份冲突排除、未知类型及多候选需确认、唯一一致候选自动确认。组合检查全部启用来源被查询、失败或超时互不阻断、查询结束后确认/撤销、增集复用与身份冲突重查；用可控时间检查短期缓存、刷新、过期/重启 410，取消和来源改配置后的旧确认被拒。检查导入成功项才移除本地配置、本期外类型跳过、统计与一次凭据脱敏/网络规则反例。",
    "argv": ["{python}", "-B", "tests/acceptance/playback_service.py", "--suite", "play-server", "--report", "evidence/acceptance/T0.48-A1.json", "--junit", "evidence/acceptance/T0.48-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.48-A1.xml",
    "cases": ["REQ-SERVICE-PLAY/matching", "REQ-SERVICE-PLAY/mapping-lifecycle", "REQ-SERVICE-PLAY/cache-and-refresh", "REQ-SERVICE-PLAY/all-sources-and-failures", "REQ-SERVICE-PLAY/import-and-safety"],
    "timeout_seconds": 1800
  },
  {
    "id": "A2",
    "description": "一次本机 App 关键流程检查：普通文件/HLS 线路显示查询与测速进度，模拟来源查询先结束而最快线路稍后测通，等待后自动选对并短时播放推进；手动提前选择、需确认后加入、换线保留进度、失败刷新一次再换线均可操作。切集或重新查找后旧结果不覆盖，过期重查保留进度；已有历史按新轮次续播，TMDB/自定义 Stremio 元数据仍可选择。观察地址和请求头留在宿主、测速绕过媒体缓存、来源导入及移除全局播放源/MacCMS 元数据；服务错误有重试入口，本地视频/直播抽查入口仍可用。",
    "argv": ["{python}", "-B", "tests/acceptance/playback_service.py", "--suite", "play-app", "--report", "evidence/acceptance/T0.48-A2.json", "--junit", "evidence/acceptance/T0.48-A2.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.48-A2.xml",
    "cases": ["REQ-SERVICE-PLAY/wait-probes-and-play", "REQ-SERVICE-PLAY/manual-confirm-and-switch", "REQ-SERVICE-PLAY/rounds-and-resume", "REQ-SERVICE-PLAY/settings-and-errors"],
    "timeout_seconds": 2700
  }
]
```

### T0.49 播放服务三：接口类来源
- [ ] 完成
- 依赖：T0.48
- 环境：当前 macos-arm64 App；服务和受控来源以本机 Docker 运行，沿用资源执行器并清理自建资源。
- 需求：REQ-SERVICE-PROVIDERS → A1
- 范围：`Cargo.toml`, `Cargo.lock`, `crates/`, `server/`, `src/`, `protocol/`, `tests/`, `licenses/`, `docs/runtime.lock.json`, `docs/PROGRESS.md`
- 交付物：按方案第 1、6.6、6.7、7.2、10 节新增 `t4_query`（drpy-node/TVBox type 4）、`t4_drpyjs0`、`t4_rest`（CatVod）、`stremio_stream` 与 json 解析适配。以实际接口样本核对 T4 参数/初始化、字符串和交替数组 url、header 与 parse/jx，按 flag 选择 json 解析接口；需要嗅探时标明原因。Stremio 根据 manifest 能力与 ID 前缀，映射 TMDB/已核实元数据外部 ID 到电影 IMDb ID 或剧集 ID，缺 ID/类型或前缀不支持时继续其他来源；HTTP 媒体按播放器实际能力筛选。更新来源管理、导入与关键 OpenAPI；向其他 Stremio 客户端输出 stream 留在第四期。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "每种新增适配类型一个代表性受控样本，合并核对搜索/详情/play 参数、T4 字符串和数组 url、header、json 解析及需嗅探状态，导入按实际类型接收或跳过。Stremio 样本覆盖电影和剧集 ID、两个元数据来源同名 ID 的隔离、缺 IMDb/前缀不匹配继续其他来源；BT 和播放器暂不支持的格式有明确处理。一次本机 App 操作实际播放一条 T4 与一条 Stremio HTTP 线路并观察进度。外部真实接口有条件时检查，其临时故障单独记录。",
    "argv": ["{python}", "-B", "tests/acceptance/playback_service.py", "--suite", "providers", "--report", "evidence/acceptance/T0.49-A1.json", "--junit", "evidence/acceptance/T0.49-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.49-A1.xml",
    "cases": ["REQ-SERVICE-PROVIDERS/t4-samples", "REQ-SERVICE-PROVIDERS/stremio-ids", "REQ-SERVICE-PROVIDERS/parsers-and-import", "REQ-SERVICE-PROVIDERS/app-playback"],
    "timeout_seconds": 2400
  }
]
```

### T0.36 T25 开发交付与最终说明
- [x] 完成
- 依赖：T0.37
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：U06, R33, R51, C-PATH, REQ-T25 → A1；U04, R30, R50, REQ-T25-LIFECYCLE → A2（覆盖表含精确对应）
- 范围：`README.md`, `docs/release-evidence/`, `docs/runtime.lock.json`, `packaging/`, `src-tauri/`, `src/`, `tests/`, `docs/PROGRESS.md`
- 证据衔接：交付说明区分 T0.35 变更前固定构建验收与 T0.37 当前源码线路/相关播放回归，不把旧证据表述为新源码全量验收。
- 移动阶段：本卡完成并独立巡检CLEAR后停止桌面监管，报告移动清单；T1.1的阶段门由控制器拦截，等待人类授权（ADR-007）。
- 交付物：落实下列修订义务；提供`tests/acceptance/t25.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "干净源码目录按最终README准确命令复现构建、运行、初始化及诊断，并仅验证本卡交付或本次改动影响的行为；macOS arm64安装包和源码/版本锁/许可证/报告可定位且摘要一致，状态与证据一一对应，列出真实限制、未运行平台和后续移动清单。不可复现步骤/遗失材料/宣称未跑平台通过时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t25.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T25-all.json",
      "--junit",
      "evidence/acceptance/T25-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T25-all.xml",
    "cases": [
      "U06",
      "R33",
      "R51",
      "C-PATH",
      "REQ-T25"
    ],
    "timeout_seconds": 7200
  },
  {
    "id": "A2",
    "description": "在本机 macOS arm64 中，经用户授权临时隔离现有应用和数据，以当前安装包及独立测试数据检查安装、初始化、实际播放、旧版升级和卸载；检查完成后恢复原有应用与数据并逐文件核对。期望用户确认画面和声音正常，升级保留设置与观看历史，卸载保留测试数据，原有应用及数据与备份一致。报告如实说明本机隔离测试和未验证的干净系统、签名或公证情况。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t25_local.py",
      "--report",
      "evidence/acceptance/T25-lifecycle.json",
      "--junit",
      "evidence/acceptance/T25-lifecycle.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T25-lifecycle.xml",
    "cases": [
      "U04",
      "R30",
      "R50",
      "REQ-T25-LIFECYCLE"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 依赖：T24。
- 修改：application/README.md、docs/progress.md、release-evidence、最终版本锁。
- 工作：重写README围绕最终实现给出构建、运行、测试、依赖初始化、诊断及发布状态；列出实际保留限制。
- 通过：另一开发AI可从全新目录按说明复现；任务状态与证据对应；实际平台材料和Rust来源支持清单可追溯。
- 最终报告：Changed / Why / Verified / Remaining，附安装包、源码、版本锁和验收报告路径。
- 本机生命周期（用户决定）：使用当前安装包和隔离的测试数据在本机检查，用户确认实际播放画面和声音；结束后恢复用户原有应用与数据。`t25_local.py` 输出 U04、R30、R50、REQ-T25-LIFECYCLE；干净系统及签名/公证状态如实记录。
- 升级判据：隔离测试资料由旧版数据库 v1 升级到 v2，设置除窗口位置和新增预设标记外保持，观看历史逐行一致；仅操作本次测试数据。
- 新观察写入前保留 `evidence/releases/T23-clean-macos/` 的旧材料。A2 需要用户到场确认声音，开工时提前告知用户准备真机或虚拟机。

### T1.1 M00 Android / iOS 真机可行性与版本基线
- [ ] 完成
- 依赖：T0.36, T0.3, T0.12, T0.13
- 阶段门：移动阶段（M00..M05）需人类明确授权，并给出真实设备及所需签名
- 需求：U07, R52, R53, R54, R56, REQ-M00 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `docs/mobile-baseline.json`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/m00.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 阶段前提：桌面T25交付后，确认人类明确授权移动阶段及真实设备/签名，才开始本卡。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "明确移动授权后Android/iPhone真机壳与本地Rust来源/网关、Stremio→JSON来源匹配/媒体；锁OS/targets/JDK/NDK/Xcode/真实设备及CORS/CSP/Origin/safearea/旋转/后台；两平台各有真机安装与短播证据，缺设备/签名/媒体或本地执行路径时对应MG0失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/m00.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/M00-all.json",
      "--junit",
      "evidence/acceptance/M00-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/M00-all.xml",
    "cases": [
      "U07",
      "R52",
      "R53",
      "R54",
      "R56",
      "REQ-M00"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：明确移动授权后Android/iPhone真机壳与本地Rust来源/网关、Stremio→JSON来源匹配/媒体；锁OS/targets/JDK/NDK/Xcode/真实设备及CORS/CSP/Origin/safearea/旋转/后台；两平台各有真机安装与短播证据，缺设备/签名/媒体或本地执行路径时对应MG0失败。
- 复用桌面DTO/来源适配/匹配/网关/历史；按真实平台需要增加小型Kotlin/Swift能力。独立APK、原包加载、配对远程Android引擎的归档义务由本地Rust路径取代。
- 阶段前提：桌面稳定上线后，人类另行明确移动授权，提供真实设备与所需签名；本次桌面开发授权不启用移动阶段。

### T1.2 M01 Android本地Rust来源接入
- [ ] 完成
- 依赖：T1.1, T0.19, T0.20, T0.3, T0.22
- 需求：U07, R53, REQ-M01 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/m01.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 阶段前提：桌面T25交付后，确认人类明确授权移动阶段及真实设备/签名，才开始本卡。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "Android真机单应用Rust来源与网关、Stremio匹配确认→播放切集；本地cap/来源头留原生、HTTPS证书/出站策略和Session隔离；进程回收重启/网络断开恢复/旧host句柄失效，cfg排除桌面专用代码。泄露凭据/串源或资源残留时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/m01.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/M01-all.json",
      "--junit",
      "evidence/acceptance/M01-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/M01-all.xml",
    "cases": [
      "U07",
      "R53",
      "REQ-M01"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：Android真机单应用Rust来源与网关、Stremio匹配确认→播放切集；本地cap/来源头留原生、HTTPS证书/出站策略和Session隔离；进程回收重启/网络断开恢复/旧host句柄失效，cfg排除桌面专用代码。泄露凭据/串源或资源残留时失败。
- 复用桌面DTO/来源适配/匹配/网关/历史；按真实平台需要增加小型Kotlin/Swift能力。独立APK、原包加载、配对远程Android引擎的归档义务由本地Rust路径取代。
- 阶段前提：桌面稳定上线后，人类另行明确移动授权，提供真实设备与所需签名；本次桌面开发授权不启用移动阶段。

### T1.3 M02 移动页面与播放器适配
- [ ] 完成
- 依赖：T1.2, T0.14, T0.15, T0.16, T0.17
- 需求：U07, R53, R55, REQ-M02 → A1（覆盖表含精确对应）
- 范围：`src/`, `src-tauri/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/m02.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 阶段前提：桌面T25交付后，确认人类明确授权移动阶段及真实设备/签名，才开始本卡。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "Android真机真实推荐→详情→video匹配→播放切集，safearea/44px/360/390/768、playsinline/触控/返回优先级/旋转/来电耳机/前后台暂停恢复/弱网和Wi-Fi蜂窝/进程回收；桌面公共组件回归与旧epoch清理。生命周期丢进度/泄流或布局溢出时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/m02.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/M02-all.json",
      "--junit",
      "evidence/acceptance/M02-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/M02-all.xml",
    "cases": [
      "U07",
      "R53",
      "R55",
      "REQ-M02"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：Android真机真实推荐→详情→video匹配→播放切集，safearea/44px/360/390/768、playsinline/触控/返回优先级/旋转/来电耳机/前后台暂停恢复/弱网和Wi-Fi蜂窝/进程回收；桌面公共组件回归与旧epoch清理。生命周期丢进度/泄流或布局溢出时失败。
- 复用桌面DTO/来源适配/匹配/网关/历史；按真实平台需要增加小型Kotlin/Swift能力。独立APK、原包加载、配对远程Android引擎的归档义务由本地Rust路径取代。
- 阶段前提：桌面稳定上线后，人类另行明确移动授权，提供真实设备与所需签名；本次桌面开发授权不启用移动阶段。

### T1.4 M03 Android 安装与发行验收
- [ ] 完成
- 依赖：T1.3
- 需求：U07, R53, R56, REQ-M03 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `packaging/`, `docs/mobile-baseline.json`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/m03.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 阶段前提：桌面T25交付后，确认人类明确授权移动阶段及真实设备/签名，才开始本卡。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "支持矩阵Android干净真机单应用签名安装/核准JSON来源首播/升级/数据迁移/回收恢复/卸载，版本与实际依赖许可配套清单。个人使用包与实际签名状态明确，公开发行另获授权；缺渠道材料如实列待办，伪称发行通过时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/m03.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/M03-all.json",
      "--junit",
      "evidence/acceptance/M03-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/M03-all.xml",
    "cases": [
      "U07",
      "R53",
      "R56",
      "REQ-M03"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：支持矩阵Android干净真机单应用签名安装/核准JSON来源首播/升级/数据迁移/回收恢复/卸载，版本与实际依赖许可配套清单。个人使用包与实际签名状态明确，公开发行另获授权；缺渠道材料如实列待办，伪称发行通过时失败。
- 复用桌面DTO/来源适配/匹配/网关/历史；按真实平台需要增加小型Kotlin/Swift能力。独立APK、原包加载、配对远程Android引擎的归档义务由本地Rust路径取代。
- 阶段前提：桌面稳定上线后，人类另行明确移动授权，提供真实设备与所需签名；本次桌面开发授权不启用移动阶段。

### T1.5 M04 iOS本地Rust来源接入
- [ ] 完成
- 依赖：T1.1, T0.3, T0.9, T0.10, T0.25, T0.22
- 需求：U07, R54, C-RESOURCE, REQ-M04 → A1（覆盖表含精确对应）
- 范围：`src-tauri/`, `packaging/`, `src/views/`, `protocol/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/m04.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 阶段前提：桌面T25交付后，确认人类明确授权移动阶段及真实设备/签名，才开始本卡。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "真实iPhone本地Rust来源、Stremio匹配确认及Rust loopback媒体；源站HTTPS证书校验、来源/Session/cap隔离、凭据留原生、断网恢复、HLS所有子资源/Range/字幕及有界流，进程回收后旧句柄失效。明文降级/绕证书/跨Session读取/旧资源泄漏时失败；若引入真实集群按C14 Docker与合并预算验收。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/m04.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/M04-all.json",
      "--junit",
      "evidence/acceptance/M04-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/M04-all.xml",
    "cases": [
      "U07",
      "R54",
      "C-RESOURCE",
      "REQ-M04"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：真实iPhone本地Rust来源、Stremio匹配确认及Rust loopback媒体；源站HTTPS证书校验、来源/Session/cap隔离、凭据留原生、断网恢复、HLS所有子资源/Range/字幕及有界流，进程回收后旧句柄失效。明文降级/绕证书/跨Session读取/旧资源泄漏时失败；若引入真实集群按C14 Docker与合并预算验收。
- 复用桌面DTO/来源适配/匹配/网关/历史；按真实平台需要增加小型Kotlin/Swift能力。独立APK、原包加载、配对远程Android引擎的归档义务由本地Rust路径取代。
- 阶段前提：桌面稳定上线后，人类另行明确移动授权，提供真实设备与所需签名；本次桌面开发授权不启用移动阶段。

### T1.6 M05 iOS 真机闭环与移动最终交付
- [ ] 完成
- 依赖：T1.4, T1.5, T1.3
- 需求：U07, R52, R54, R55, R56, REQ-M05 → A1（覆盖表含精确对应）
- 范围：`src/`, `src-tauri/`, `packaging/`, `README.md`, `docs/mobile-baseline.json`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/m05.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 阶段前提：桌面T25交付后，确认人类明确授权移动阶段及真实设备/签名，才开始本卡。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "iPhone本地Rust真实推荐→详情→选集→匹配确认→内嵌播放切集，safearea/旋转/音频中断/后台/系统返回与离线恢复、TLS/会话隔离，签名安装干净设备/升级卸载、修订MG0..MG3全证据；Android和桌面公共行为回归，实际发行状态完整。缺真机或必需MG项未测时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/m05.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/M05-all.json",
      "--junit",
      "evidence/acceptance/M05-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/M05-all.xml",
    "cases": [
      "U07",
      "R52",
      "R54",
      "R55",
      "R56",
      "REQ-M05"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：iPhone本地Rust真实推荐→详情→选集→匹配确认→内嵌播放切集，safearea/旋转/音频中断/后台/系统返回与离线恢复、TLS/会话隔离，签名安装干净设备/升级卸载、修订MG0..MG3全证据；Android和桌面公共行为回归，实际发行状态完整。缺真机或必需MG项未测时失败。
- 复用桌面DTO/来源适配/匹配/网关/历史；按真实平台需要增加小型Kotlin/Swift能力。独立APK、原包加载、配对远程Android引擎的归档义务由本地Rust路径取代。
- 阶段前提：桌面稳定上线后，人类另行明确移动授权，提供真实设备与所需签名；本次桌面开发授权不启用移动阶段。

