# 来源与技术决策

研究日期：2026-10-23。用户已确定三平台同期与全部原始插件兼容。以下区分静态核实事实、工程选择和需要开发阶段取得的运行证据。

## 已核实事实

| 编号 | 来源 | 支持的事实 |
|---|---|---|
| S01 | [固定索引](https://raw.githubusercontent.com/821938089/cloudstream-extensions/d411c9597e16f1e12f51f563ef75cd8d00ace002/plugins.json) | 当前基线7个包及其版本、URL、语言和元数据 |
| S02 | [本次制品检查](plugin-artifact-inventory.json) | 全部原包ZIP均含manifest.json/classes.dex，原包长度与摘要及固定commit一致 |
| S03 | [加载器](https://github.com/recloudstream/cloudstream/blob/a7ac454af8fbd64574c96847a936905855911720/app/src/main/java/com/lagradost/cloudstream3/plugins/PluginManager.kt) | Android Context、PathClassLoader及真实插件加载路径 |
| S04 | [构建配置](https://github.com/821938089/cloudstream-extensions/blob/25cdd2119be03f62ae6a5a2cf46b16f9484224a2/build.gradle.kts) | 扩展工程应用Android library/Kotlin Android插件及Cloudstream API依赖 |
| S05 | [SuperVodPlugin](https://github.com/821938089/cloudstream-extensions/blob/25cdd2119be03f62ae6a5a2cf46b16f9484224a2/SuperVod/src/main/kotlin/com/horis/cloudstreamplugins/SuperVodPlugin.kt) | 一个插件在load中注册多个MainAPI实例 |
| S06 | [BaseVodProvider](https://github.com/821938089/cloudstream-extensions/blob/25cdd2119be03f62ae6a5a2cf46b16f9484224a2/SuperVod/src/main/kotlin/com/horis/cloudstreamplugins/BaseVodProvider.kt) | 部分内容/集数引用为序列化对象；源方法把该字符串用于详情/链接解析 |
| S07 | [YjysProvider](https://github.com/821938089/cloudstream-extensions/blob/25cdd2119be03f62ae6a5a2cf46b16f9484224a2/YjysProvider/src/main/kotlin/com/horis/cloudstreamplugins/YjysProvider.kt) | getVideoInterceptor对请求及特定媒体清单响应有处理 |
| S08 | [模拟器加速](https://developer.android.com/studio/run/emulator-acceleration) | 宿主CPU、系统镜像架构和平台虚拟化前提 |
| S09 | [命令行](https://developer.android.com/studio/run/emulator-commandline) | AVD启动、安装、窗口/音频相关选项及CLI入口 |
| S10 | [ADB命令参考](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/docs/user/adb.1.md) | forward与tcp:0动态端口 |
| S11 | [RFC 8216](https://www.rfc-editor.org/rfc/rfc8216) | HLS主/媒体清单、URI、Key、初始化片及byterange等语义 |
| S12 | [Tauri Rust命令](https://v2.tauri.app/develop/calling-rust/) | 前端invoke调用Rust命令、传参、返回及事件机制 |
| S13 | [hls.js](https://github.com/video-dev/hls.js) | 基于MSE的HLS播放及原生HLS能力选择；codec由平台支持决定 |
| S14 | [WebView版本](https://v2.tauri.app/reference/webview-versions/) | Tauri使用平台WebView：Windows WebView2、Android WebView、macOS/iOS WKWebView、Linux WebKitGTK |
| S15 | [Tauri capabilities](https://v2.tauri.app/security/capabilities/) | 按窗口/WebView与平台约束原生能力权限 |
| S16 | [Cloudstream许可证](https://github.com/recloudstream/cloudstream/blob/a7ac454af8fbd64574c96847a936905855911720/LICENSE) | 所参考上游附GPL版本3许可证；引擎分支保留其标识及源码对应信息 |
| S17 | [MainAPI](https://github.com/recloudstream/cloudstream/blob/a7ac454af8fbd64574c96847a936905855911720/library/src/commonMain/kotlin/com/lagradost/cloudstream3/MainAPI.kt) | 参考接口及getVideoInterceptor能力 |
| S18 | [AOSP镜像目录](https://dl.google.com/android/repository/sys-img/android/sys-img2-3.xml) | API35 default arm64-v8a和x86_64包标识存在；包标识列表见android-aosp-images.json（未含文件摘要，revision与摘要由T00固定） |
| S19 | [Tauri Vite配置](https://v2.tauri.app/start/frontend/vite/) | Vite与Tauri前端构建、开发服务器及移动开发配置 |
| S20 | [NanoHTTPD](https://github.com/NanoHttpd/nanohttpd) | 可嵌入式Java HTTP实现的选型参考 |
| S21 | [Tauri CSP](https://v2.tauri.app/security/csp/) | 为WebView配置内容安全策略与资源访问限制 |
| S22 | [Tauri移动插件](https://v2.tauri.app/develop/plugins/develop-mobile/) | Android Kotlin/Java、iOS Swift原生能力与Rust接入 |
| S23 | [Android绑定服务](https://developer.android.com/develop/background-work/services/bound-services) | 客户端绑定Service、IPC与绑定生命周期 |
| S24 | [原App字段静态证据](app-data-evidence.json) | 文件摘要、相对artifact_path、精确字节片段与字段消费语义；原位置path保留为溯源信息 |
| S25 | [本次REA模块视图](app-model-rea-evidence.json) | 选定4个JS文件静态分析摘要及Detail模块身份；partial覆盖 |
| S26 | [MacCMS Provide源码](https://github.com/magicblack/maccms10/blob/master/application/api/controller/Provide.php) | JSON/XML数据接口、列表/详情参数、播放组与资源字段；具体站点是否开放须实测 |
| S27 | [Stremio协议](https://stremio.github.io/stremio-addon-sdk/protocol.html) | HTTP资源路径与catalog/meta/stream/subtitles职责 |
| S28 | [Stremio manifest](https://stremio.github.io/stremio-addon-sdk/api/responses/manifest.html) | 资源字符串/对象、types/idPrefixes、catalogs和extra声明 |
| S29 | [Stremio Stream](https://stremio.github.io/stremio-addon-sdk/api/responses/stream.html) | URL/其他候选类型、proxyHeaders及字幕标识提示 |
| S33 | [TMDB Stremio add-on](https://github.com/mrcanelas/tmdb-addon) | 扩展项目说明提供元数据语言选择；具体中英文请求路径在N01实测 |
| S30 | [Stremio Meta](https://stremio.github.io/stremio-addon-sdk/api/responses/meta.html) | 元数据、videos及原video_id、inline streams |
| S31 | [OWASP SSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html) | 出站目标、DNS解析与重定向需要共同校验；具体连接绑定策略为本项目工程约定 |
| S34 | [Naive UI官方仓库](https://github.com/tusen-ai/naive-ui) | Vue3/TypeScript组件、内置主题与themeOverrides、MIT；2.45.3已接入当前预览并锁版 |
| S32 | [MediaError](https://developer.mozilla.org/en-US/docs/Web/API/MediaError) | 媒体错误暴露通用code与浏览器相关message；项目通过网关状态查询判断资源租约与上游错误 |

## 固定的研究版本

| 项目 | 研究基线 |
|---|---|
| 插件索引builds提交 | d411c9597e16f1e12f51f563ef75cd8d00ace002 |
| 插件源master提交 | 25cdd2119be03f62ae6a5a2cf46b16f9484224a2 |
| Cloudstream Android候选提交 | a7ac454af8fbd64574c96847a936905855911720 |

客户端采用Tauri 2；Vue 3、TypeScript、Vite、hls.js作为初始工程选择。精确版本在T00实际构建后写入package-lock.json、Cargo.lock和runtime.lock.json，移动工具链在M00固定。官方文档核对日期为2026-10-23。

## 工程决策

- Cloudstream原始DEX在Android ART运行；新增MacCMS/Stremio由Rust原生HTTP处理，使用AppSource联合选择后端。
- 使用固定Cloudstream分支加入引擎入口，避免脱离其初始化环境后重建整套stub。
- 专用AVD提供三平台统一客体；ADB只管理本项目serial/forward。
- Cloudstream媒体保留原interceptor/Cookie；原生HTTP媒体由Rust网关处理，同一套契约/fixture验证两处清单与资源生命周期。
- 客户端采用Tauri 2 + Rust；界面默认Vue 3 + TypeScript + Vite，页面命令经Rust进入引擎。
- HTML video与原生HLS/hls.js为初始播放路径；平台codec、CSP/CORS、loopback与cap资源授权经G1实测后固化。
- Android后续先采用同机独立引擎APK与签名权限绑定服务，保留上游初始化；iOS按受信HTTPS远程Android引擎路径先做可行性验证。
- 首页使用Stremio catalog；Cloudstream主播放、MacCMS可选播放，经18.6匹配形成推荐、详情、播放闭环。

## 开发前必须完成的实测

1. 候选Cloudstream runtime是否与7个实际DEX的二进制接口兼容。当前上游版本只作为初始候选；实际通过版本写入runtime.lock.json。
2. 多源注册全集与全部实例声明能力的契约测试，包含原始opaque字符串和interceptor。
3. Tauri系统WebView/video/hls.js在目标平台的构建、codec、音频、字幕、cap鉴权、CSP/CORS与loopback网络策略。
4. 官方SDK/system image的实际revision、虚拟化前提、资源占用、启动时间和引擎包签名。
5. 源站当前在线结果。本次仅读取GitHub/官方技术资料与原包ZIP元数据，源站页面与视频未实际请求。
6. Android移动独立组件绑定/进程回收、iOS配对远程引擎/真机播放，以及移动系统权限和生命周期。
7. 发行材料：引擎GPL来源、播放器/codec依赖、原插件获取/分发方式及对应许可记录；最终发行策略在T00/T23形成明确清单。

## 选择的成本

三平台完整原始插件兼容引入Android运行环境、系统WebView媒体依赖和代理维护。实际资源成本由G0/G1测量。优先复用一套兼容引擎、同一组DTO/业务命令与一套WebView播放控制。Android移动阶段额外维护组件安装/绑定；iOS远程模式额外维护配对、TLS与部署。具体成本在M00/M04测量并记录。

## 本次静态制品检查方法

读取用户指定JSON，逐一从原仓库获取七包到内存，核对长度、计算SHA-256并读取ZIP manifest/entry list；随后使用固定builds提交重新取得相同字节并验证摘要。原包未安装或执行。输出插件清单保留原URL、固定URL及完整摘要。


数据模型1.4.0接入：见上层DATA_STRUCTURES.md。原App字段用于参考展示与选择语义；bridge/app契约、字段默认值和ID公式为本项目设计。原App完整服务端响应、各provider实际数据及播放路径继续由开发期实测。默认契约检查独立运行，原样本字节复核通过--artifact-root显式启用。

HTTP来源的实际站点、配置路径兼容、媒体类型及播放尚待N00/N01/T20实测。当前只核对原始协议与项目源码；不预设公共影视服务默认地址。Rust宿主/客体隔离与任务排期为本项目工程设计。

架构修订1.3.1：会话资源所有权、配置提交世代、统一请求预算、完整manifest快照、warnings与资源状态查询为本项目工程约定；合成样例只验证契约。新增状态查询复用现有Rust/Android网关控制通道，原始媒体请求继续沿既有媒体通道。

1.4.0约定：同集换线路保留Resolution，候选显式释放；首页范围与index共同决定合并；Cloudstream变更提交前停止对应会话。Stremio meta支持videos内联streams的事实沿用S30；旧1.4.0曾据此计算resolve能力；1.5.0采用元数据角色，resolve固定false。上述模型向量与上下文反例只用于契约核对，真实行为由任务卡验收。

2026-10-26语言决策：英文为默认页面语言；支持en/zh-CN，Stremio元数据按中文区域变体→zh-CN、其余→en派生。18.3.1的英文基础字段、中文覆盖、缓存与竞态规则为本项目设计；S27/S28定义协议与配置，S33为具有语言选择能力的扩展示例。预览双语fixture与真实扩展运行验证分别记录。

2026-10-26前端整理：基础UI选择Naive UI 2.45.3，版本、实现与选型比较见application/README.md及package-lock.json。官方源码/已安装类型核对NConfigProvider、useOsTheme、Form.labelProps、Modal焦点与表单校验行为；浏览器预览验证与Tauri实机验证分开。PrimeVue当前5.x授权依据其发布包LICENSE.md与[官方社区条款](https://primeui.dev/licenses/community)，旧4.x仍为MIT；Reka UI为无样式基础组件，见[官方介绍](https://reka-ui.com/docs/overview/introduction)。

2026-10-26架构1.5.0：按用户最新设定，Stremio仅承担catalog/meta元数据，Cloudstream主播放，MacCMS可选播放。S27至S30记录上游具备的资源能力；本项目只采用其中元数据能力。MetadataIdentity、规范content_id、match_playback、手动确认、双来源状态与历史SQLite结构为本项目工程设计，合成契约可核对，真实匹配质量与持久化行为由T08/N02/T18/T20验收。Android原始provider搜索/首页/详情/解析能力继续做7包兼容测试。
