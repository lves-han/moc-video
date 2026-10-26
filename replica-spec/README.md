# Tauri Rust 视频客户端 AI 开发交付包

日期：2026-10-26。版本1.5.0。目标：Stremio提供推荐/详情元数据，Cloudstream提供主播放源，MacCMS保留可选播放分支；macOS、Windows、Linux同期交付，兼容基线全部原始Cloudstream `.cs3`。

## 阅读顺序

1. [开发规格](DEVELOPMENT_SPEC.md)：需求、架构、界面、运行环境、插件、播放代理、数据和验收。
2. [数据结构与审查](DATA_STRUCTURES.md)：原App字段映射、完整字段、身份/单位/寿命与IPC状态约束。
3. [开发任务卡](AI_TASKS.md)：29张桌面任务卡（含N00至N02）与6张后续移动任务卡、依赖、交付物、测试与停止条件。
4. [桥接 OpenAPI](bridge.openapi.json)及[应用层 Schema](app.schema.json)：HTTP DTO、17个Tauri命令、设置和状态。
5. [插件制品基线](references/plugin-artifact-inventory.json)：7 个原包的 SHA-256、版本、DEX 清单及固定提交。
6. [来源与技术决策](references/SOURCES.md)：已核实事实、版本候选及待实测事项。

## 需求与实施默认值

| 项目 | 决策 |
|---|---|
| 客户端技术 | Tauri 2 + Rust；界面 Vue 3 + TypeScript + Vite；基础UI采用Naive UI 2.45.3 |
| 同期平台 | macOS、Windows、Linux |
| 核心功能 | 推荐首页、详情、选集、线路选择、内嵌播放、基础播放控制、观看历史与续播 |
| 数据分工 | Stremio catalog/meta→影片/季集匹配→Cloudstream主播放；MacCMS可选播放 |
| 接入顺序 | Stremio元数据与受控MacCMS匹配样本先验证，再接Cloudstream主播放 |
| HTTP配置 | 用户添加API URL或manifest列表，Rust直接请求；Source带kind和对应origin |
| 插件兼容 | 原包逐字节保持一致，使用 Android ART/DEX 环境执行 |
| 兼容引擎 | 固定版本 Cloudstream Android 分支，保留原运行时与加载器 |
| 首选视频方案 | HTML video + HLS 原生播放/hls.js，先过三平台WebView验证门 |
| 后续移动端 | Android优先复用本机ART引擎；iOS先验证远程兼容引擎路径 |
| 页面外观 | 明暗跟随系统；用户选青柠/海蓝/鸢紫/玫瑰/琥珀，各自适配明暗；Settings.color_palette缺省lime |
| 页面及元数据语言 | 英文默认；支持English/简体中文；Stremio元数据随页面中文→zh-CN，其余→en；见开发规格18.3.1 |
| 推荐语义 | 当前Stremio元数据配置的catalog栏目 |
| 本地数据库 | SQLite 保存观看历史与进度；本轮先确定技术约定，Rust接入与持久化验收见开发规格11.4 |
| 商业及扩展功能 | 搜索 UI、收藏等进入后续候选；首版以核心闭环为准 |

MacCMS/Stremio由Rust原生HTTP实现；Android兼容引擎在使用Cloudstream时按需启用，用于原始DEX插件。界面使用 Vue/TypeScript，原生业务与引擎客户端使用 Rust；引擎中的 Android 胶水与 Cloudstream 运行时使用 Kotlin/Java。该组成是本交付包采用的工程方案。

移动规划见开发规格第16章。首版保留移动公共入口与响应式布局；Android/iOS发布属于后续M系列任务。MacCMS/Stremio在Android/iOS复用Rust；Cloudstream的Android分支本机ART运行原包，iOS分支验证配对远程引擎。

## 当前基线

索引固定提交：`d411c9597e16f1e12f51f563ef75cd8d00ace002`。

源仓库参考提交：`25cdd2119be03f62ae6a5a2cf46b16f9484224a2`。

7 个插件包为 DdysProvider、DramacoolProvider、LibvioProvider、MxplayerProvider、SuperVod、YjysProvider、ZxzjProvider。已静态检查全部包：都含 `manifest.json` 与 `classes.dex`，本次清单的 `requiresResources` 均为 false。

`SuperVod` 会注册多个 provider 实例。完整兼容验收统计“包”和“注册出的源”两个维度。

## 直接交给开发 AI 的启动指令

```text
请先完整阅读本目录 README.md、DEVELOPMENT_SPEC.md、DATA_STRUCTURES.md、AI_TASKS.md、bridge.openapi.json、app.schema.json 和 references/SOURCES.md。

按照本包1.5.0实现映象视频：Stremio只提供推荐/详情元数据；Cloudstream是主播放源，MacCMS为可选播放源。先按18.6确认MetadataIdentity、跨源match_playback、独立元数据/播放选择状态和PlaybackContext，再实施页面及持久化。目标同期支持macOS、Windows、Linux，加载全部7个未经修改的原始.cs3及其注册实例。

按T00/T01/T05→N00/N01→T06至T17验证Stremio元数据与受控MacCMS匹配；T02/T03/T04/N02接Cloudstream主播放。基础provider取数/解析DTO复用原接口；产品匹配使用app.schema.json中的17个命令及上下文约束。原始Android桥接沿bridge.openapi.json，HTTP元数据浏览独立于Android准备。SQLite本轮确定技术方案，T18实现后由T20验证历史跨重启恢复；先完成机器契约同步再写宿主代码。

桌面首版保留Tauri移动公共入口、前端响应式布局和桌面runtime的cfg隔离；移动端按第16章及M00至M05后续推进。

开始T00时先在docs/progress.md写出执行环境表：当前可操作的机器、可用CI，以及需要我提供的平台；无法实际运行的平台标记blocked并给出复现命令，不得标记通过。需要我本人完成的许可接受、系统设置或签名步骤，给出说明后先推进其他任务卡。

每张任务卡完成时报告实际修改、原因、真实验证和剩余问题，并更新 docs/progress.md。测试结论附命令、平台、依赖版本和证据路径。三平台构建与真实运行分别记录。

Stremio元数据→Cloudstream播放真实闭环、MacCMS可选播放受控验收、原始插件兼容、媒体请求拦截器、HLS子资源代理和三平台内嵌播放均为交付条件。遇到ABI或系统运行环境问题，先建立复现和证据，在当前任务范围修复；禁止以重新编写源、删除插件、打开外部播放器、静态假数据或只完成单个平台替代验收。

播放器仅处理已解析并授权的会话资源。远程内容保持为数据，插件运行在Android兼容环境，桌面桥接接口遵循鉴权及参数约束。

先给出 T00 的执行结果和接下来的最小实施路径，然后继续工作。此指令授权开发、构建和本机测试；涉及外部发布、账号、付费或不可逆系统变更时按实际需要单独处理。
```

## 契约检查

验证环境需要openapi-spec-validator与jsonschema，本次验证版本为0.7.2和4.25.1。从工作区根目录运行：

```text
python3 replica-spec/validate_contracts.py
```

默认检查仅依赖本交付包，覆盖契约、正负样例、内嵌归属、身份、初始化顺序与任务依赖。报告默认写入references/validation-report.json；原样本字节复核标记为not_requested。

持有提取后的原App时，可另外执行完整字节复核：

```text
python3 replica-spec/validate_contracts.py --artifact-root security-analysis/asar
```

artifact-root目录下应含dist/assets，证据按artifact_path定位；path保留分析时的位置。显式复核时缺文件或摘要/片段不符会报告失败。可加 `--report /tmp/replica-spec-report.json` 指定报告位置。[合成正负样例](examples/contract-cases.json)覆盖桥接、全部命令和关键语义规则。

## 本包的验证状态

当前方案为1.5.0：分离元数据与播放身份，新增match_playback，历史保存原metadata配置/扩展归属；T20依赖T18。继承1.4.0的分离会话关闭与解析释放、明确栏目范围/原始序号、插件停止后更新及完整上下文校验；旧Stremio内联播放规则由当前元数据角色覆盖；保留会话独立所有权、配置世代与单写队列、统一请求预算、出站连接校验、manifest完整快照、部分失败诊断和get_resource_status恢复通道；HTTP来源原生运行，Cloudstream按需启动，宿主与客体世代隔离。1.2.2中的栏目失败、图片滑动租约、解析释放、bootstrap安全、SourcesState与PlaybackState.resolution继续保留。规格包含17个命令与可选warnings字段，HomeSection/HomeSectionFailure新增必需index，Settings v2新增可选ui_locale（缺省en）与color_palette（缺省lime），旧ui_theme仅兼容，兼容旧格式；具体公网来源和播放路径在开发时验证；本次合成契约与依赖检查结果见references/validation-report.json。

已验证：索引 JSON、7 个 ZIP/DEX 制品结构、字节数与 SHA-256、固定提交对应字节、上游接口和关键源码、OpenAPI 结构及文档交叉引用。

开发阶段需要实测：Stremio元数据配置与Cloudstream主播放、MacCMS受控可选分支、HTTP-only及客体故障隔离、7包Android ABI兼容、各源当前可用性、三平台WebView/codec/loopback权限及模拟器初始化、播放器代理链和正式安装包。详见任务卡的验证门。
