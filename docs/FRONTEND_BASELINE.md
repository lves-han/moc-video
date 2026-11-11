# 墨客视频界面

基于 Vue 3、TypeScript、Vite、Naive UI 2.45.3 的可操作前端预览，采用 al-design 技能制作。视觉沿用 replica-spec 的深色影院方向，以青柠色标记主操作，使用原创电影画面、轻量侧栏和清晰的海报网格。窄屏切换到底部导航。

## 运行

```sh
cd /Users/eason/jp/application
npm ci
npm run dev
```

打开 http://127.0.0.1:5173/ 。`npm run build` 执行 Vue/TypeScript 检查并输出 `dist/`；`npm run preview` 预览构建结果。

## 已实现的界面

- 发现页：精选切换、影片题材筛选、分类导航、展开列表、空态及切源加载态。
- 详情页：海报、简介、演职员、选集、相关影片。
- 播放页：真实 HTML video、本地 H.264/AAC 30 秒片段、播放/暂停、进度拖动、音量/静音及百分比（桌面与窄屏均在播放器内调整）、倍速、WebVTT 字幕、全屏、选集和自动下一集。
- 观看历史：侧栏与移动底栏入口，按影片合并最近集数、来源和进度；按今天/昨天/更早分组，支持继续观看、看完重播、单条删除与清空确认。当前预览记录保存在页面会话内，刷新重置；SQLite 为后续宿主的持久化约定。
- 来源管理：三类演示来源、选择与停用、添加配置表单及输入错误提示。新增配置仅在当前页面会话内保留。
- 外观：明暗模式实时跟随系统；设置页提供青柠、海蓝、鸢紫、玫瑰、琥珀五种色系，默认青柠。每种色系均包含明暗配色，偏好保存在本机。顶部调色板图标进入设置，色系变更保留页面、筛选与播放状态。
- 中英文：英文为基础和首次启动默认语言，顶部与设置页可切换 English / 简体中文；语言偏好保存在本机，刷新后保留。
- 播放设置：本次页面会话内的倍速、自动下一集；音量统一在播放页调整。
- 键盘：Space 播放/暂停，左右方向键快进/后退五秒，M 静音，F 全屏；输入控件和弹层优先处理键盘。

主要地址为 `#/home`、`#/kind/movie`、`#/kind/series`、`#/kind/anime`、`#/film/midnight`、`#/watch/midnight/4`、`#/history`、`#/sources` 和 `#/settings`，浏览器返回可用。

## 数据与素材

影片名称、演职员、简介均为虚构示意内容，集中在 `src/data/demo.ts`。五幅画面由内置 imagegen 原创生成；最终图片放在 `public/artwork/`，生成提示与来源记录见 [artwork.json](docs/artwork.json)。图标与标识为本项目 SVG，字体使用系统字体。

`public/demo.mp4` 由旷野画面缓慢平移生成，包含本地合成的低音量环境音；中英字幕为 `public/demo.vtt` 与 `public/demo.en.vtt`。所有影片与线路在预览中使用这一相同片段，界面明确标识其演示性质。运行时无 CDN、在线字体或远程媒体依赖。

## 后续接入

本次交付为界面工程。Rust/Tauri 宿主、真实 MacCMS/Stremio 网络请求和 Cloudstream Android 引擎尚待开发。当前演示视图模型用于验证交互；接入时按 replica-spec 的机器契约生成 DTO，通过 Tauri 命令替换数据入口，并保留已有组件和视觉样式。

后续接入真实来源时同步完成配置持久化、请求取消、宿主/客体世代、解析与会话释放、HLS 管线和跨平台 WebView 验收。当前未声明任何 Tauri 构建、真实片库或三平台发布验证通过。

## 语言与 Stremio 元数据

英文文案直接作为 `src/i18n.ts` 的消息键，中文词典覆盖对应键。路由、影片 ID、题材值与接口字段保持英文键；虚构影片的中英文展示数据在 `src/data/demo.ts`。切换语言保留当前路由、筛选、集数、播放进度与播放偏好。

Stremio 继续作为元数据接入方向。`getStremioLanguage()` 从页面语言计算元数据语言：`zh` 及中文区域变体 → `zh-CN`；其余 → `en`。当前选项为英文、简体中文，默认英文；原始电影音轨语言与页面语言分别处理。演示字幕跟随页面语言，正式字幕选择保留用户控制。

当前设置页显示该映射，影片仍为本地双语 fixture。真实 Stremio catalog/meta 请求接入时按 replica-spec 第18.3.1节完成已验证扩展的语言配置、英文回退、分语言缓存与旧响应丢弃。Stremio 扩展采用各自的语言配置方式，具体请求路径由该扩展验证结果确定。现阶段无真实元数据网络调用。

执行 `npm test` 验证语言映射、英文默认值、翻译占位符、偏好保存失败行为、色系启动恢复和五种色系在两种模式下的文字对比度；`npm run build` 执行类型检查与构建。

色系偏好由根元素的 `data-palette` 恢复，统一配置位于 `src/ui/theme.ts`。Naive UI 的 `useOsTheme` 读取系统明暗，`NConfigProvider` 在内置 `lightTheme` / `darkTheme` 上应用同一份品牌色覆盖；页面与库组件从该配置派生颜色。启动前恢复已保存色系，旧的手动明暗偏好保持忽略。视频和海报维持画面上的高对比度，播放器高亮使用所选色系的明亮配色。后续Tauri宿主将色系纳入Settings.color_palette，沿用已有设置写队列；ui_theme作为旧文件兼容字段保留。

## UI 库选型与代码组织

核对日期：2026-10-26。当前项目采用 **Naive UI 2.45.3**，版本与间接依赖由 package-lock.json 固定。候选按现有 Vue 3/TypeScript、系统明暗、多色系、桌面/移动响应式、授权与维护工作量比较。

| 候选 | 核实能力及代价 | 本项目选择 |
|---|---|---|
| [Naive UI](https://github.com/tusen-ai/naive-ui) | Vue 3、TypeScript、内置明暗主题、类型化 themeOverrides、按组件构建、MIT；无需额外的 CSS 框架 | 采用，与现有栈直接衔接，基础控件和主题统一消费 |
| [PrimeVue](https://primevue.dev/theming/styled/) | 控件及主题体系完整；当前最新版5.0.4及themes3.1.0发布包使用PrimeUI授权，社区许可含资格、密钥与年度续期；4及更早版本仍为MIT，见[官方条款](https://primeui.dev/licenses/community) | 当前项目优先MIT依赖的持续维护路径 |
| [Reka UI](https://reka-ui.com/docs/overview/introduction) | Vue无样式无障碍基础组件，适合自建完整视觉系统；主题和控件外观需自行组合 | 本次以减少手写基础控件及主题样式为目标，采用已有成套组件 |

组件直接从 `naive-ui` 导入。基础组件采用 `NConfigProvider`、`NGlobalStyle`、`NMessageProvider`、`NButton`、`NModal`、`NForm`、`NFormItem`、`NInput`、`NSwitch`、`NRadioGroup`、`NRadioButton`、`NTag`、`NAlert`。表单规则交给库校验，错误摘要与字段提示共同呈现，弹层使用库的焦点圈定与返回能力。短列表（页面语言、倍速、来源类型及顶部来源选择）沿用原生select，直接使用系统选择菜单和原生键盘语义；媒体进度及音量继续使用原生range。此分工保留移动端原生交互，并缩减额外的下拉框状态与样式。

| 文件 | 责任 |
|---|---|
| src/ui/UiProvider.vue | 单一库配置、系统主题、组件语言、全局消息及页面CSS变量桥接 |
| src/ui/theme.ts | 五种品牌色覆盖、持久化色系、Naive内置主题与页面语义变量的同源派生 |
| src/views/SettingsView.vue | 外观、语言与播放偏好UI，通过v-model更新共享播放偏好 |
| src/views/SourcesView.vue | 来源列表、添加表单、字段校验与弹层；通过事件交给App修改共享来源 |
| src/views/HistoryView.vue | 历史分组、进度、续播、删除/清空确认及加载/错误状态 |
| src/data/history.ts | 会话内历史预览数据入口，后续对接宿主 SQLite |
| src/data/history-model.ts | 预览历史字段、进度校验与时间格式 |
| src/App.vue | 页面导航、共享来源与播放状态、首页/详情及现有播放器交互 |
| src/style.css | 影院布局、海报/播放器、响应式和原生短列表轻量样式 |

主题色值集中在theme.ts，常规组件外观优先修改themeOverrides。后续业务页直接复用库组件及现有主题，保持单一UiProvider；按出现的实际复用再拆组件。当前首页、详情与播放器继续共用既有状态，接入Rust时按原规格中的生命周期边界组织。

本次相较引入UI库前，App.vue减少约470行，CSS减少约620行；Naive的通用控件带来约100kB的gzip JS增量，CSS gzip减少约2.5kB，具体数值见实际构建输出。库与素材随安装包本地分发，运行时沿用离线资源。浏览器预览与三平台Tauri/移动WebView实机验证分别记录。

## 观看历史与 SQLite 约定（2026-10-26）

用户确认 SQLite 为技术约定，本轮先写入方案。当前工程保持 Vue 前端预览，历史数据使用会话内 Map；刷新后从空列表开始。播放进度大约每5秒更新，暂停、seek、切集、切源与离页时记录当前进度。历史中的影片身份与播放来源分开保存；语言切换沿用同一影片记录。看完标记针对当前集，重新播放从该集起点开始。

正式宿主按 [开发规格11.4](../replica-spec/DEVELOPMENT_SPEC.md#114-观看历史与-sqlite) 使用 SQLite，保存位置为应用数据目录；由 Rust 负责迁移和参数化读写。接入时替换 `src/data/history.ts`，继续使用已有历史视图与播放事件入口。当前 `demo:<kind>:<id>` 身份服务于虚构预览；真实数据使用 Stremio 的稳定内容/video身份，重新匹配当前 Cloudstream/MacCMS 播放引用后续播。

`npm test` 验证按影片更新、来源独立、排序、删除/清空、无效进度与临时会话字段校验；`npm run build` 执行类型检查和打包。SQLite 跨重启持久化、迁移、写入失败与三平台运行在宿主接入阶段验收。

本轮前端验证（2026-10-26）：构建和8项测试通过；浏览器验证桌面/390px无横向溢出、空态、按影片更新集数、原来源恢复、进度续播、当前集看完与从头重播、删除/清空确认取消。实际SQLite读写与三平台宿主验证留待T18/T20。构建主JS约519kB（gzip约158kB），Vite报告单包体积提示。

文档方案已更新至1.5.0：Stremio元数据→match_playback→Cloudstream主播放，MacCMS可选；元数据与播放源分别选择。当前界面仍使用本地fixture与会话内历史，真实匹配、双来源状态和SQLite宿主接入按replica-spec实施。
