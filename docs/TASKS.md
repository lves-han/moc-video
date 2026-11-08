# 映象视频任务路线

状态：T0.1至T0.17已完成；当前T0.19按ADR-048迁移为Rust播放源路线，后续业务验收NOT RUN。29张桌面、6张后续移动沿用原卡号；已完成卡正文及历史保持。原SPEC逐字节归档，生效差异见ADR-005、ADR-044、ADR-048及ARCHITECTURE。

平台范围（ADR-005/ADR-006，macOS first）：桌面必需验收目标只有macOS arm64（本机）。各卡“原始义务”及覆盖表中的macOS x86_64、Windows、Ubuntu X11/Wayland运行、安装、性能与签名要求降为非阻断记录项：代码保持cfg可移植，验收报告列为NOT RUN并给出复现命令，后续另行授权；本机macOS arm64缺测、失败或伪称其他平台通过时验收失败。

## 路线与执行前提

当前顺序：T0.19 SuperVod JSON→T0.20多源身份/完整性→T0.21原生请求上下文→T0.22宿主及来源管理→T0.18设置/SQLite→T0.23配置事务→T0.24至T0.29桌面验收交付。原脚本按文件顺序及依赖选卡；每卡独立验收。当前开发授权来自2026-11-05聊天，文档独立CLEAR后执行。

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
| T24 | T0.28 | 独立验收与问题修复 | 桌面 |
| T25 | T0.29 | 开发交付与最终说明 | 桌面 |
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
| U01 / §1.1 | Tauri/Rust、WebView、原生持久化 | T0.2/A1, T0.3/A1, T0.18/A1, T0.28/A1 | 按原规格完整保留 |
| U02 / §1.1,2.1 | 首版可用SuperVod JSON来源的搜索、详情、选集及解析 | T0.19/A1, T0.20/A1, T0.22/A1, T0.24/A1 | ADR-048取代给定索引与全部provider义务 |
| U03 / §1.1,8 | 推荐/详情/选集/播放完整真实闭环 | T0.14/A1, T0.15/A1, T0.16/A1, T0.24/A1 | 按原规格完整保留 |
| U04 / §1.1,5.1 | macOS arm64真实安装运行 | T0.2/A1, T0.21/A1, T0.24/A1, T0.27/A1, T0.28/A1 | ADR-005：macOS x86_64/Windows/Linux为非阻断记录项 |
| U05 / §1.1,2.2,6.3 | 依据公开行为独立实现的Rust来源，实际媒体闭环 | T0.19/A1, T0.20/A1, T0.22/A1, T0.24/A1 | ADR-048取代原7包ART/DEX兼容；其余六站暂缓 |
| U06 / §1.1,15 | 准确契约/依赖/错误与可复现交付 | T0.1/A1, T0.3/A1, T0.28/A1, T0.29/A1 | 按原规格完整保留 |
| U07 / §1.1,16 | 桌面公共入口/cfg/响应式、后续移动真机MG | T0.13/A1, T0.28/A1, T1.1/A1, T1.2/A1, T1.3/A1, T1.4/A1, T1.5/A1, T1.6/A1 | 按原规格完整保留 |
| U08 / §1.1,18 | Stremio元数据与Rust MacCMS播放来源独立选择、跨源匹配 | T0.4/A1, T0.5/A1, T0.8/A1, T0.22/A1, T0.24/A1 | ADR-048取代Cloudstream主播放 |
| U09 / §1.1,11.4 | 历史及SQLite跨重启/续播/失败完整性 | T0.13/A1, T0.16/A1, T0.18/A1, T0.24/A1 | 按原规格完整保留 |
| R01 / §1.2,1.3 | 独立恢复元数据/播放偏好，初版页面；来源可用后由用户选择 | T0.3/A1, T0.13/A1, T0.14/A1, T0.17/A1, T0.22/A1, T0.28/A1 | ADR-048移除准备Android默认流程 |
| R02 / §2.1 | 旧索引/制品只留历史；首版候选列表及实际支持状态可追溯 | T0.1/A1（历史）, T0.19/A1, T0.23/A1 | ADR-048移除插件下载/锁更新 |
| R03 / §2.2 | Rust宿主/公开JSON适配器运行，无Android环境前提 | T0.19/A1, T0.22/A1 | ADR-048取代原Application/加载器 |
| R04 / §2.3 | 每个核准JSON来源独立身份/revision、配置UUID稳定 | T0.20/A1, T0.23/A1 | ADR-048取代包→provider全集 |
| R05 / §2.4 | 原生媒体请求头/Referer与来源上下文保持、敏感字段留Rust | T0.7/A1, T0.8/A1, T0.21/A1, T0.25/A1 | ADR-048移除原插件interceptor语义 |
| R06 / §2.5,3,3.1 | 单Rust crate、类型分发、原生HTTP与共用媒体路径 | T0.3/A1, T0.4/A1, T0.5/A1, T0.9/A1, T0.22/A1 | ADR-048移除Android执行分支 |
| R07 / §3.2 | 核准Rust来源、内嵌视频、macOS arm64及真实数据交付 | T0.24/A1, T0.28/A1 | ADR-005/048；原包7/7与全集义务取代 |
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
| R30 / §10.1 | 系统WebView真实H264/AAC/HLS/MSE、平台依赖与安全传输 | T0.2/A1, T0.12/A1, T0.27/A1, T0.28/A1 | 按原规格完整保留 |
| R31 / §10.2 | 17invoke形状/权限/host_epoch/包装DTO/递归图片与事件卸载、旧cloudstream明确未支持 | T0.3/A1, T0.7/A1, T0.9/A1, T0.13/A1, T0.22/A1 | ADR-048保留线格式兼容，移除guest执行 |
| R32 / §10.3 | 一个video/player生命周期/250ms位置/旧epoch/自动播放提示/CORS/CSP | T0.2/A1, T0.12/A1, T0.16/A1, T0.28/A1 | 按原规格完整保留 |
| R33 / §10.4 | 锁文件、npm ci/type/build、Rustfmt/check/clippy/test与发行依赖 | T0.1/A1, T0.27/A1, T0.29/A1 | 按原规格完整保留 |
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
| R48 / §13 | 1080p30 H264AAC10分钟、20Mbps3分钟、50切集无泄漏与DPI125/150/200 | T0.2/A1（功能）, T0.12/A1, T0.25/A1（功能）, T0.26/A1, T0.26/A2（网关耐久）, T0.28/A1（播放器耐久） | ADR-044放宽 |
| R49 / §14.1 | 公开行为fixture/自有媒体，测试替身与正式构建隔离 | T0.2/A1, T0.4/A1, T0.5/A1, T0.19/A1, T0.22/A1, T0.25/A1, T0.28/A1 | ADR-048取代原DEX网络替身 |
| R50 / §14.2,14.3 | G0-Core及Rust来源G2、G1/G3/G4/G5实际桌面闭环 | T0.1/A1（历史）, T0.2/A1, T0.22/A1, T0.24/A1, T0.25/A1, T0.27/A1, T0.28/A1 | ADR-048取消G0-CS、取代原包G2 |
| R51 / §15 | 先机器契约、真命令/日志/报告、复现/回归/数据及发布材料 | T0.3/A1, T0.28/A1, T0.29/A1 | 按原规格完整保留 |
| R52 / §16.1,16.2 | 桌面后移动分期与Rust/DTO/页面/cfg复用 | T0.13/A1, T1.1/A1, T1.6/A1 | 按原规格完整保留 |
| R53 / §16.3 | Android本地Rust来源、单应用、生命周期/权限隔离 | T1.1/A1, T1.2/A1, T1.3/A1, T1.4/A1 | ADR-048；移动阶段另获授权 |
| R54 / §16.4 | iOS本地Rust来源/网关，HTTPS校验、会话隔离及断网恢复 | T1.1/A1, T1.5/A1, T1.6/A1 | ADR-048取代远程引擎/配对部署；移动另获授权 |
| R55 / §16.5 | 44px/safearea/360,390,768/playsinline/返回/音频/后台/网络 | T1.3/A1, T1.6/A1 | 后续移动另获授权 |
| R56 / §16.6 | MG0..MG3真机、安装签名与生命周期最终交付 | T1.1/A1, T1.4/A1, T1.6/A1 | 后续移动另获授权 |
| R57 / §17 | 全部固定事实/工程决定/待实测追溯来源与原始证据 | T0.1/A1, T0.28/A1 | 按原规格完整保留 |
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
| C-PATH / §用户路径确认/ADR-001/C1 | 唯一根、venv/原脚本/driver与全局锁/原证据绑定 | T0.1/A1, T0.29/A1 | 按原规格完整保留 |
| C-RESOURCE / §moc-init/C14 | 预算全命令覆盖、真实集群Docker/容器硬限额合计+宿主RSS/实际采样及自产清理 | T0.1/A1, T0.25/A1, T0.26/A1, T0.26/A2, T1.5/A1 | 按原规格完整保留 |
| REQ-T00 / replica-spec/AI_TASKS.md T00 | 本卡完整工作/通过/验证/停止条件原文见卡内，附可观察反证 | T0.1/A1 | 桌面首版 |
| REQ-T01 / replica-spec/AI_TASKS.md T01 | 开发功能与产品耐久分期，原时长/规模/指标保持 | T0.2/A1（功能）, T0.28/A1（耐久） | 桌面首版 |
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
| REQ-T23 / replica-spec/AI_TASKS.md T23 | ADR-048修订后的T23工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.27/A1 | 桌面首版 |
| REQ-T24 / replica-spec/AI_TASKS.md T24 | ADR-048修订后的T24工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.28/A1 | 桌面首版 |
| REQ-T25 / replica-spec/AI_TASKS.md T25 | ADR-048修订后的T25工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T0.29/A1 | 桌面首版 |
| REQ-M00 / replica-spec/AI_TASKS.md M00 | ADR-048修订后的M00工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.1/A1 | 后续移动 |
| REQ-M01 / replica-spec/AI_TASKS.md M01 | ADR-048修订后的Android本地Rust来源接入工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.2/A1 | 后续移动 |
| REQ-M02 / replica-spec/AI_TASKS.md M02 | ADR-048修订后的M02工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.3/A1 | 后续移动 |
| REQ-M03 / replica-spec/AI_TASKS.md M03 | ADR-048修订后的M03工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.4/A1 | 后续移动 |
| REQ-M04 / replica-spec/AI_TASKS.md M04 | ADR-048修订后的iOS本地Rust来源接入工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.5/A1 | 后续移动 |
| REQ-M05 / replica-spec/AI_TASKS.md M05 | ADR-048修订后的M05工作/验收，原始义务见归档；其余性能强度沿ADR-044 | T1.6/A1 | 后续移动 |

## 任务卡

## 开发与产品测试分期（当前用户决定）

开发阶段以功能实现为中心：每卡实际完成相关构建/类型检查、功能行为与正负例回归、必要的短时真实集成验证，验证通过后交接，并沿原完成提交与独立巡检流程收尾。长时间连续播放及本次列出的窗口耐久验收集中到产品测试阶段；原时长、规模、指标、资源与清理要求保持。其他开发卡现有的短时功能循环、错误/边界反例及即时资源清理继续按卡执行。

T0.2的MP4/HLS耐久及窗口开关移交T0.28/A1，按ADR-044为各600秒及5次；T0.25的网关播放由T0.26/A2在真实产品链路完成，按ADR-044为10分钟。T0.26为产品性能/故障测试，T0.28为产品独立验收与修复，保留原文件顺序及依赖。原始义务原文作为来源保留，执行阶段按本条及ADR-020、ADR-044解释；后续产品测试按当时稳定源码和构建重新实测。短时功能通过与长期验收待测分别记录。


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
- 需求：U04, R30, R33, R45, R50, REQ-T23 → A1（覆盖表含精确对应）
- 范围：`packaging/`, `src-tauri/`, `licenses/`, `docs/runtime.lock.json`, `package.json`, `package-lock.json`, `README.md`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t23.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "macOS arm64个人使用安装包干净系统安装/初始化/首播/升级/卸载；本地前端无运行CDN/开发绝对路径、实际依赖许可/版本锁对应；安装包不含原插件、引擎APK或SDK镜像。签名公证如实记录；缺Apple凭据产出未签名包并列待办。其他平台NOT RUN，个人使用范围及第三方来源限制清楚，未签名冒通过或依赖证据缺失时失败。",
    "argv": [
      "{python}",
      "-B",
      "tests/acceptance/t23.py",
      "--suite",
      "all",
      "--report",
      "evidence/acceptance/T23-all.json",
      "--junit",
      "evidence/acceptance/T23-all.xml"
    ],
    "kind": "test",
    "report": "junit:evidence/acceptance/T23-all.xml",
    "cases": [
      "U04",
      "R30",
      "R33",
      "R45",
      "R50",
      "REQ-T23"
    ],
    "timeout_seconds": 7200
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 工作与验收：Tauri构建、本地资源、实际依赖和版本锁/许可通知；移除Android运行时/原包发行材料要求及产物。干净机器安装/首播/升级/卸载，使用默认系统安全设置，无开发机绝对路径；缺签名凭据如实列未签名与发行待办，公开发行另获用户授权。

### T0.28 T24 独立验收与问题修复
- [ ] 完成
- 执行阶段：产品测试；承接T0.2两格式各600秒及5次窗口开关（ADR-044），按当前受验源码/构建重新执行；复用现有原生夹具、长播断言及资源清理，历史短测与失败只作历史证据。
- 依赖：T0.27
- 需求：U01, U04, R30, R32, R48, R49, REQ-T01, U06, U07, R01, R07, R50, R51, R57, REQ-T24 → A1（覆盖表含精确对应）
- 范围：`src/`, `src-tauri/`, `packaging/`, `docs/release-evidence/`, `tests/`, `docs/PROGRESS.md`
- 交付物：落实下列修订义务；提供`tests/acceptance/t24.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 独立验收：本卡必须由独立只读审阅者审查固定构建及受验提交，作者按报告修复后再次复审。审阅原始报告保存于`docs/reviews/T24/round-<N>.md`（随文档提交），正文写明所审提交SHA与源码摘要，末行`ACCEPTANCE-VERDICT: CLEAR`或`ACCEPTANCE-VERDICT: BLOCKING`；t24.py只读取N数值最大的轮次，提交/摘要与本次构建不一致、末行缺失或为BLOCKING时失败。 本卡先经原run执行同源码/构建/protected/generation的t24-endurance前置实跑并保存原始样本，payload在原日志输出证据manifest SHA以由收据log_hash绑定；独立只读报告复核本批原始证据及固定WIP构建，记录文件SHA，再执行A1完整重算同批所有长播/生命周期断言并核对原收据/日志/manifest/独立报告，拒绝仅passed摘要、缺失或改写证据及旧源码/基线。报告允许先于最终A1，绑定受验构建提交与源码；其后仅docs提交保持证据绑定。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "本产品测试阶段先执行T0.2移交的真实1080p30 MP4与HLS各600秒、5次完整窗口开关及帧/UI/音频/资源清理（ADR-044），以当前源码/构建实测；缺少、失败或沿用旧源码时失败。随后独立只读验收者逐修订U01..U09（U07首版结构）及G0-Core/G1..G5检查当前提交，复算Rust来源支持清单与平台真实运行安装媒体/数据/历史证据，行为复测高影响问题清零；作者修复后再次独立复审。核对当前仅个人使用，公开发行未获授权；缺独立结论/未解决阻断/复制旧报告时失败。",
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

### T0.29 T25 开发交付与最终说明
- [ ] 完成
- 依赖：T0.28
- 环境：macos-arm64 必需；macos-x86_64、windows-x86_64、linux-x11/wayland 记录为 NOT RUN（ADR-005/006）
- 需求：U06, R33, R51, C-PATH, REQ-T25 → A1（覆盖表含精确对应）
- 范围：`README.md`, `docs/release-evidence/`, `docs/runtime.lock.json`, `tests/`, `docs/PROGRESS.md`
- 移动阶段：本卡完成并独立巡检CLEAR后停止桌面监管，报告移动清单；T1.1的阶段门由控制器拦截，等待人类授权（ADR-007）。
- 交付物：落实下列修订义务；提供`tests/acceptance/t25.py`的真实行为断言与对应报告。复用已存在代码，保持C1..C16中与本卡相关契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "干净源码目录按最终README准确命令复现构建/运行/测试/初始化/诊断；macOS arm64安装包和源码/版本锁/许可证/报告可定位且摘要一致，状态与证据一一对应，列出真实限制、未运行平台和后续移动清单。不可复现步骤/遗失材料/宣称未跑平台通过时失败。",
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
  }
]
```
#### 修订义务（ADR-048取代归档原卡）

- 依赖：T24。
- 修改：application/README.md、docs/progress.md、release-evidence、最终版本锁。
- 工作：重写README围绕最终实现给出构建、运行、测试、依赖初始化、诊断及发布状态；列出实际保留限制。
- 通过：另一开发AI可从全新目录按说明复现；任务状态与证据对应；实际平台材料和Rust来源支持清单可追溯。
- 最终报告：Changed / Why / Verified / Remaining，附安装包、源码、版本锁和验收报告路径。

### T1.1 M00 Android / iOS 真机可行性与版本基线
- [ ] 完成
- 依赖：T0.29, T0.3, T0.12, T0.13
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

