# 播放服务方案：独立服务 API、元数据与播放源统一、多源自动选线

- 整理日期：2026-11-11
- 状态：第 2 版（ADR-077、ADR-078）。用户 2026-11-11 授权按审查意见修订方案、任务卡与 ADR，并按个人娱乐应用收敛验收要求。任务卡为 T0.47（第一期）、T0.48（第二期）、T0.49（第三期），第四期记入 BACKLOG。本次仅修订与复审文档，开发等待用户另行指令。
- 取代：本文档并入并取代 [FOLLOWUP-AUTO-PLAYBACK-SOURCES.md](FOLLOWUP-AUTO-PLAYBACK-SOURCES.md)（第 3 版）。该文档已批准的 D1–D10（ADR-075、ADR-076）在本方案中保留其意图，执行位置按第 2.3 节调整。
- 需求来源（用户 2026-11-11 聊天）：
  1. 播放影片时查询所有播放源，用户选择或程序选择延迟最低的线路；设置中移除当前播放源选择。
  2. 元数据与播放源解耦，元数据全部使用 TMDB 或 Stremio，播放源暂用 MacCMS 格式；跨源搜索用应用语言，加入过滤并由程序再筛选。
  3. 支持 zyfun、TVBox、Stremio 等插件或扩展机制；做独立服务 API，App 向服务查询可用播放源和实际播放信息，服务缓存剧集可用源、统一多种机制；TMDB/Stremio 元数据也由服务提供，服务每天更新主页排名与分类，并代理 TMDB。
  4. 暂时放弃 CloudStream-3 和爬虫类机制，只保留接口形式。

## 1. 结论与范围

可行，整体难度中等，前三期粗估 5–8 个开发周（第 11 节，含基本联调，随实际接口适配调整）。来源以 HTTP 接口接入，服务用 Rust 单进程与 SQLite，复用 App 现有的 MacCMS 解析、TMDB 映射、片名匹配与网络安全代码。

本方案面向个人娱乐使用：优先保证能找片、选集、选线、播放、续播和保留当前数据。每期用代表性样本验证关键流程，具体要求见第 10 节。超时、并发、缓存期均为可调初值，按实际体验调整；验收关注正确行为，性能数值作为观察记录。高可用、多租户、压力测试、长时间耐久和其他平台验证留待实际需要时安排。

**纳入范围**

| 来源类型 | 说明 | 依据 |
|---|---|---|
| MacCMS JSON / XML | zyfun T1_JSON / T0_XML，TVBox type 1 / 0 | App 已实现 JSON；zyfun `t1Json.ts`、`t0Xml.ts` |
| T4 查询参数式 | zyfun T4_DRPYS（drpy-node）、T4_DRPYJS0；TVBox type 4 | zyfun `t4Drpys.ts`、`t4DrpyJs0.ts` |
| T4 REST 式 | zyfun T4_CATVOD（`/home`、`/category`、`/detail`、`/search`、`/play`） | zyfun `t4Catvod.ts` |
| Stremio 播放扩展 | `/stream/{type}/{id}.json`，按 IMDb 编号 | Stremio 扩展协议 |
| json 型解析接口 | `api + 平台页面地址` 返回最终地址 | zyfun `parse/analyze.ts` |
| TMDB 元数据 | 官方 API，服务端持有 Key | App `tmdb_api.rs`（T0.41） |
| Stremio 元数据扩展 | 用户指定的 manifest、catalog、meta，第一期支持 | App 现有 Stremio 客户端与扩展协议 |

**不纳入**：CloudStream-3；TVBox type 3 jar；zyfun T3_DRPY、T3_PY、T3_XBPQ、T3_XYQ、T3_CATOPEN 等脚本或规则爬虫；web 型解析（需无头浏览器嗅探）；Stremio 的 BT（`infoHash`）流。zyfun 的 T3_APPYSV2、T3_APPGET 属于特定 APP 后端的 HTTP 接口，列为第四期可选。

本地视频与直播（IPTV）不经过服务，保持现状。

## 2. 总体架构

```
┌──────────── App（Tauri 桌面） ────────────┐        ┌──────────────── 播放服务（Rust） ────────────────┐
│ WebView 前端：首页/详情/搜索/线路列表/播放器 │        │ 对外：Stremio 扩展协议 /stremio/{token}/…          │
│ Rust 宿主：                                 │ HTTPS  │       原生接口 /v1/…（Bearer 鉴权）                │
│  · Stremio 客户端（已有）→ 首页/详情/搜索   │───────▶│ 元数据：TMDB 适配 + 缓存 + 每日首页快照            │
│  · 线路轮次：查询服务、持有播放地址、测速   │        │ 播放：来源适配器（MacCMS/T4/Stremio流/解析）       │
│  · 媒体网关（已有）→ 播放                   │        │       匹配流水线（语言片名→过滤→筛选→确认）       │
│ 本地视频、直播（不变）                      │        │       对应关系缓存、线路成败统计、预取             │
└─────────────────────────────────────────────┘        │ 存储：SQLite；网络：沿用当前网络规则             │
                                                       └───────────────────────────────────────────────────┘
                     媒体请求：App 网关 ──直连──▶ 播放源 CDN（可选：经服务转发，见 7.4）
```

### 2.1 职责划分

| 职责 | 位置 | 理由 |
|---|---|---|
| 元数据、首页排行与分类、搜索 | 服务 | 统一来源、隐藏 TMDB Key、每日快照可缓存 |
| 播放源配置与管理 | 服务 | 多设备共用一份来源配置 |
| 跨源搜索、过滤、筛选、确认、对应关系缓存 | 服务 | 计算一次、所有集与设备复用；可预取热门影片 |
| 播放地址解析（含 T4 `play`、解析接口） | 服务 | 统一多种机制的差异 |
| 测速与选线 | App | 延迟必须从用户网络测量 |
| 播放与失败换线 | App | 现有网关与播放器 |
| 观看记录、收藏 | App | 现有 SQLite，不变 |

### 2.2 对外协议的选择

服务对外实现 **Stremio 元数据协议**（manifest、catalog、meta），复用 App 的首页、分类、详情和搜索流程。第一期同时调整自建服务的语言路由、目录和身份识别；现有客户端对已知 TMDB 域名的特殊处理须改为按已登记的元数据来源能力处理。

App 播放走原生接口 `/v1/lookups`，统一多源线路、确认和测速。向其他 Stremio 客户端输出 `stream` 属于第四期可选项，前三期的 manifest 仅声明已实现的 `catalog`、`meta`。接入第三方 Stremio 播放扩展仍在第三期。

### 2.3 原方案决定的去向

| 原决定（FOLLOWUP-AUTO-PLAYBACK-SOURCES） | 本方案 |
|---|---|
| D1 无外部 ID 时的自动确认门 | 服务端匹配流水线第 4 步（6.4） |
| D2 线路记忆只用于免重复确认 | 由服务端「对应关系」取代 App 的 `line-preference.ts`（6.5） |
| D3 MacCMS 详情/目录绑定自身来源 | 已由 D8 取代；本方案 App 不再浏览 MacCMS |
| D4 播放失败自动换下一条 | App（8.4） |
| D5 手动选择只在当前播放生效 | App；服务记录用户确认的对应关系（6.5） |
| D6 删除全局播放源与 `last_playback_source_key` | App（8.2） |
| D7 侧栏综艺改用 TMDB 真人秀/脱口秀，短剧、体育移除 | 服务首页分类（4.3） |
| D8 元数据只用 TMDB/Stremio | 服务 + App |
| D9 应用语言片名查询 | 服务（6.2） |
| D10 两级过滤筛选 | 服务（6.3、6.4） |
| 测速定义、并发、轮次与取消 | App（8.3） |

## 3. 服务端技术选型与部署

- **语言与框架**：Rust，`axum` + `tokio`，与 App 共用 `reqwest`、`rusqlite`、`serde_json`。
- **代码复用**：沿已批准的 Q3 建立 Cargo workspace 与共享 crate `moc-core`。按各期实际使用，迁入来源解析、匹配、TMDB 映射和 HTTP/HLS 的共用部分；单侧使用的逻辑留在对应侧，随调用方调整，避免一次搬迁全部模块。第一期保持现有播放可用，第二期替换 App 内匹配。
- **存储**：服务端 SQLite 单文件，表结构见第 7.1 节。
- **部署**：Docker 镜像，单容器，挂载数据目录；可放在家用 NAS、VPS 或本机。只供个人使用（ADR-048），不对外开放注册。
- **配置**：环境变量指定监听地址、数据目录、TMDB Key（或 v4 令牌）。访问令牌优先采用 `MOC_SERVICE_TOKEN`；未配置时从数据目录的私有令牌文件读取，首次缺失则生成并原子保存。重启复用原令牌；日志仅提示文件位置，用户从文件读取后填入 App。轮换后 App 重新填写令牌，已有内容身份保持稳定。
- **HTTPS**：服务自身监听 HTTP，由反向代理（Caddy/Nginx）提供 HTTPS；本机与局域网可直接用 HTTP，App 设置显示一次说明。远程连接使用 HTTPS，证书校验开启。
- **本机与局域网访问**：沿远端已采纳 ADR-068，来源及媒体允许公网、回环、内网和代理 Fake-IP，复用现有 HTTP 能力。App 的服务访问凭据限定到配置的 origin 与基础路径，跨 origin 重定向直接报错；配置的局域网服务域名可采用系统 DNS。共享来源/媒体传输继续使用现有固定 DNS，详见 7.3。

## 4. 元数据服务

### 4.1 TMDB 适配

- 复用 `tmdb_api.rs`：它已把 TMDB v3 接口转换为 Stremio 结构的 manifest（12 个目录）、catalog、meta，内容标识 `tmdb:<id>`，剧集分集优先 `<IMDb>:<季>:<集>`，缺少 IMDb 时沿当前实现使用 `tmdb:<id>:<季>:<集>`（T0.41、ADR-065）。服务端改为真实 HTTP 路由，并补上 4.3 的目录映射。
- 语言：服务统一使用 `zh-CN` / `en-US`；App 的 `en` 映射为 `en-US`。manifest 按语言区分，App 切换语言后更新路由；自建服务的路径约定由登记信息确认，与域名无关。
- 为匹配补充：meta 中增加 `original_title`（TMDB `original_title` / `original_name`）与 `aliases`（`/alternative_titles` 中与应用语言相同地区的名称，最多 3 个），作为服务内部字段，不影响 Stremio 客户端。

### 4.2 缓存

| 内容 | 缓存期 | 说明 |
|---|---|---|
| 首页快照（排行、分类首屏） | 每天 04:00（服务时区）重建 | 见 4.3 |
| catalog 分页 | 6 小时 | 键：元数据来源及版本 + 类型 + 语言 + 目录 + 筛选 + 页码 |
| meta（影片、剧集与分集） | 24 小时；正在连载的剧集 6 小时 | 键：元数据来源及版本 + 类型 + 语言 + 内容标识 |
| 图片 | 不缓存图片字节，返回 TMDB 图片地址 | App 网关已有图片缓存 |
| 搜索 | 1 小时 | 键：元数据来源及版本 + 类型 + 语言 + 目录 + 关键词 |

缓存遵守 TMDB 使用条款：界面保留 TMDB 署名；不把缓存作为对外数据服务。

### 4.3 每日首页

定时任务按语言生成快照，每个目录取前 2 页：

首次启动或已有快照过期时立即在后台生成；服务重启加载最近成功快照。首次生成期间首页返回可重试的 `ENGINE_NOT_READY`，App 显示「正在准备首页」与重试入口，搜索和按 ID 查询详情照常可用。刷新成功后原子替换快照。时区默认 `Asia/Shanghai`，可通过 `TZ` 配置。

- 热门电影、热门剧集（`/trending/{movie|tv}/day`）
- 正在热映、即将上映（电影）、正在播出（剧集）
- 高分电影、高分剧集
- 分类：电影/剧集按 TMDB 类型；「综艺」= 剧集类型「真人秀」(10764) + 「脱口秀」(10767)（D7）；「动画」「纪录片」按类型
- 快照失败时保留上一份快照并在 `/v1/health` 报告

快照在 manifest 中以 Stremio catalog 形式出现，App 首页按现有方式加载。

### 4.4 Stremio 元数据扩展（第一期）

第一期提供 `/v1/metadata-sources` 的列表、新增、修改、删除与配置导入，登记用户指定的 Stremio manifest；服务读取 `resources`、`types`、`idPrefixes` 和 catalog 的 extra 声明，代理其 catalog、搜索、meta，按 4.2 缓存。可选 extra（例如 genre、skip）按原协议传递；缺少声明能力时向 App 返回对应状态。第三方目录按需读取，TMDB 每日首页独立刷新。

每个元数据来源有持久 `metadata_source_id`，TMDB 固定为 `tmdb`，其他来源使用 UUID。路由按来源隔离，保留上游的原始类型、内容 ID 和 video ID，避免两个扩展的同名 ID 冲突。第三方扩展的语言按其明确约定传递；缺少本地化能力时使用原始内容并提示语言状态。

App 的元数据设置保留来源选择、语言与扩展管理，管理操作转到服务。首次连接可上传当前格式的自定义 Stremio 配置并保存其来源身份；已上传配置按稳定 ID 更新。改服务地址、令牌、语言或扩展地址均沿用逻辑身份，具体记录规则见第 9 节。

## 5. 服务端 API

### 5.1 通用约定

- 路径前缀 `/v1`；JSON；UTF-8。
- 鉴权：原生接口使用 `Authorization: Bearer <token>`；Stremio 路由为兼容常见客户端，将令牌放在路径 `/stremio/{token}/…`。App、服务和反向代理统一隐藏该路径段与 Authorization；随部署提供关闭相关访问日志或脱敏的示例，见 7.5。
- 错误：`{"error":{"code":"…","message":"…","retryable":true|false}}`，code 沿用 App 的 `ApiError` 代码（`BAD_REQUEST`、`UNAUTHORIZED`、`UPSTREAM_FAILED`、`UPSTREAM_TIMEOUT`、`NOT_FOUND`、`RESOURCE_LIMIT` 等）。
- 语言：原生接口使用 `lang=zh-CN|en-US`；Stremio 路由使用同名路径段。
- 内容身份：请求包含 `metadata_source_id + type + meta_id + video_id`；服务核对目标 video 属于该内容，季集关系明确。类型和来源均参与缓存、对应关系及线路记录的键。
- 播放地址等敏感数据只出现在播放相关响应中，服务日志不记录完整地址与请求头。

### 5.2 Stremio 扩展路由

| 方法与路径 | 说明 |
|---|---|
| `GET /stremio/{token}/{metadata_source_id}/{lang}/manifest.json` | 前三期 resources：`catalog`、`meta`；types 按实际来源声明 |
| `GET /stremio/{token}/{metadata_source_id}/{lang}/catalog/{type}/{id}[/{extra}].json` | TMDB 首页、分类或指定扩展的目录；extra 按 Stremio 约定传 search、skip 等 |
| `GET /stremio/{token}/{metadata_source_id}/{lang}/meta/{type}/{id}.json` | 影片或剧集详情与分集 |
| `GET/POST /v1/metadata-sources`、`PUT/DELETE /v1/metadata-sources/{id}` | 登记元数据来源；POST 可携带从当前 App 导出的原始配置与逻辑身份，批量导入按稳定 ID 幂等更新 |

Stremio 路由的 extra 使用独立路径段并正确编码，各路由返回协议要求的 CORS 头。TMDB 的 manifest 保留 `tmdb-addon` 标识与内容 ID，App 为其登记固定的 TMDB 逻辑身份。

### 5.3 线路查找

`POST /v1/lookups`

```json
{ "metadata_source_id": "tmdb", "meta_id": "tmdb:12345", "type": "series", "video_id": "tt1234567:1:3",
  "lang": "zh-CN", "sources": null, "refresh": false }
```

- `sources`：限定来源 ID 列表，`null` 为全部启用来源。
- `refresh`：`true` 时忽略短期线路缓存（对应关系仍复用），缓存与标识生命周期见 5.8。

响应为 `application/x-ndjson` 流，每行一个事件，按发生顺序推送：

```json
{"event":"started","lookup_id":"…","sources":[{"source_id":"…","name":"量子","kind":"maccms_json"}]}
{"event":"source","source_id":"…","status":"searching"}
{"event":"confirm","source_id":"…","candidates":[{"candidate_id":"…","title":"示例剧集特别版","year":2026,"kind":"series","episodes":12,"mismatch":["title"]}]}
{"event":"line","line":{"line_id":"…","source_id":"…","source_name":"量子","line_name":"lzm3u8","episode_label":"第3集","kind":"hls","quality":"1080P","play":{"url":"https://…","headers":{"Referer":"…"}},"expires_at":1762900000,"history":{"ok":12,"failed":1,"last_ok_at":1762800000}}}
{"event":"source","source_id":"…","status":"done","lines":3}
{"event":"source","source_id":"…","status":"not_found"}
{"event":"done"}
```

来源状态：`searching`、`done`、`needs_confirmation`、`not_found`、`no_episode`、`unsupported`（如不支持搜索）、`failed`（附 `error`）、`timeout`。

线路 `play` 字段：

| 情况 | 返回 |
|---|---|
| 直链（MacCMS 地址、T4 `parse=0`、Stremio `url`、json 解析结果） | `{"url","headers"}` |
| 需要 web 解析或嗅探（T4 `parse=1` 且无 json 解析接口可用） | 不返回 `play`，`kind:"unsupported"`，附 `reason:"needs_sniffing"` |

服务端整轮上限 30 秒（初始值，实测后调整），每个来源 15 秒；超时来源报 `timeout`，不影响其他来源。

`started` 携带 `lease_expires_at`（Unix 秒）；后续事件属于同一 lookup。收到 `done` 表示来源查询结束，App 继续处理待测线路；流中途异常须标明未完成来源，避免误报全部完成。超时、并发及每轮待测数量的初值由实现集中配置，按体验调整。

### 5.4 确认与对应关系

| 方法与路径 | 说明 |
|---|---|
| `POST /v1/mappings` | 体 `{lookup_id, source_id, candidate_id}`，其余内容与目标集从 lookup 取得。核对有效期、候选归属及来源版本后保存，响应为本次目标集线路（NDJSON，事件同 5.3） |
| `GET /v1/mappings?metadata_source_id=&type=&meta_id=` | 该影片在各播放来源的对应关系 |
| `DELETE /v1/mappings/{mapping_id}` | 撤销列表返回的对应关系，并记录撤销状态 |
| `DELETE /v1/lookups/{lookup_id}` | 取消查找、在途确认和解析，释放候选及线路记录；已开始的播放会话独立持有地址 |

### 5.5 线路刷新与结果上报

| 方法与路径 | 说明 |
|---|---|
| `POST /v1/lines/{line_id}/refresh` | 在记录有效期内，用保存的来源、目标集和解析参数重新解析，返回同一 `line_id` 的新 `play`、`expires_at`；不改变线路身份 |
| `POST /v1/reports` | 体 `{line_id, outcome:"playing"|"failed", latency_ms, error_code}`。App 在实际开始播放或失败时上报，服务据此更新线路成败统计，用于下次排序参考和预取 |

### 5.6 来源管理

| 方法与路径 | 说明 |
|---|---|
| `GET /v1/sources` | 来源列表与状态（就绪、检查中、失败、禁用） |
| `POST /v1/sources` | 新增：`{name, kind, api, ext, headers, enabled, parse_ids}` |
| `PUT /v1/sources/{id}`、`DELETE /v1/sources/{id}` | 修改、删除 |
| `POST /v1/sources/{id}/check` | 按 kind 做一次搜索与详情样本检查，返回结果与耗时 |
| `POST /v1/sources/import` | 导入 TVBox / zyfun 配置 JSON，接收本期已实现类型，其余列入跳过清单；App 配置上传携带本地 `config_id` 用于幂等导入 |
| `GET/POST/PUT/DELETE /v1/parsers` | json 型解析接口：`{name, api, flags, headers}` |

`kind` 取值：`maccms_json`、`maccms_xml`、`t4_query`（drpy-node、TVBox type 4）、`t4_drpyjs0`、`t4_rest`（CatVod）、`stremio_stream`、（第四期）`app_ysv2`、`app_get`。

### 5.7 其他

| 方法与路径 | 说明 |
|---|---|
| `GET /v1/health` | 服务版本、TMDB 状态、首页快照时间、来源统计 |
| `GET /v1/media/{ticket}`（第四期，可选） | 媒体转发，见 7.4 |

### 5.8 查找、候选与线路缓存

- lookup、candidate、line 记录保存在服务进程内，默认自查询结束或最近一次确认后保留 15 分钟；记录总量和字节数有上限，按需清理。ID 为不透明随机值，候选和线路绑定对应 lookup、元数据来源、类型、影片、目标集及播放来源版本。
- 查询流正常结束后保留这些记录，允许用户阅读后确认。确认前与写入对应关系前再次核对 lookup 仍有效；取消后的确认返回 `CANCELLED`，候选串用或类型不符返回 `BAD_REQUEST`。取消前已完成的用户确认保留，可通过撤销接口更正。
- 短期线路缓存键为上述内容身份、播放来源版本、解析配置版本和线路标识。仅缓存当前配置上下文的结果；每次 lookup 仍签发本轮 line ID。URL 已知有效期时取其有效期与本地短期缓存期的较早值；未知时默认只复用 60 秒。`expires_at` 表示本地重新解析时点，实际可用性以播放为准。
- 播放地址和请求头只存内存；SQLite 中的对应关系与统计单独持久化。来源或解析配置修改、删除、禁用时清除受影响的短期结果与句柄。
- 记录过期、提前清理或服务重启后，确认与刷新返回 HTTP 410 / `SESSION_EXPIRED`；App 提示后为原影片和目标集重新查找，恢复进度。上报失败仅影响统计，播放继续。
- App 取消时中断 NDJSON、确认与测速，并尽力调用 DELETE；若尚未收到 lookup ID，收到迟到的 `started` 后补发取消。服务检测断连也取消在途查找。App 的轮次 ID 校验始终负责丢弃迟到结果。

## 6. 匹配流水线（服务端）

对每个来源，在一次查找中执行：

### 6.1 对应关系优先

若该影片在该来源已有对应关系（6.5）且仍有效，直接取来源内容详情定位目标集，跳过 6.2–6.4。`stremio_stream` 来源按 6.7 的已核实 ID 请求。

### 6.2 查询片名（D9）

最多两个，规范化后去重：

1. 应用语言片名（服务从本次选定的元数据来源取得；扩展缺少本地化时采用其返回片名）。
2. 原始片名；与第 1 项相同或缺失时用英文片名，再缺时用 `aliases` 第一个。

不做 T0.45 式的放宽（去续集号、截前缀）。

### 6.3 第一级：片名过滤（D10）

- 请求来源搜索，读取首页全部结果。
- 规范化（全角转半角、去标点、小写）后，候选标题包含查询片名，或查询片名包含候选标题（候选至少 2 字），才保留。即 App 现有 `fuzzy::related`。
- 每个查询保留至多 10 条，两次查询合并去重后至多 20 条。为空记 `not_found`。

依据：2026-11-11 实测 8 个默认 MacCMS 源，多数对 `wd` 做全文检索（量子用「The Odyssey」返回 2000 条，首页仅 1 条同名），只有优质、淘片是片名子串匹配。

### 6.4 第二级：程序筛选与确认（D1、D10）

1. 排除衍生内容：分类含「解说」「预告」「花絮」等的条目在 MacCMS 解析时已滤掉；再按标题标记（解说、预告、花絮、片段、彩蛋、幕后）排除。
2. 类型：元数据为电影时排除明确的剧集候选，为剧集时排除明确的电影候选。动漫、其他或类型缺失的候选保留供用户确认；只有电影/剧集类型已核实一致时参与自动确认。
3. 年份：核对结构化年份，以及标题末尾可明确识别的四位年份；其中出现与元数据冲突的年份即排除。标题尾年只在与元数据年份一致后去掉再比片名。缺年份时沿 D1 按其余条件判断，附 `year_missing` 提示。
4. 对剩余候选取详情，目标集无法在分组中唯一对应的排除（集对应规则统一为一套，移植自 App `episodeCoordinates`/`correspondingEpisode`）。电影要求分组恰好一集。
5. 自动确认：片名规范化后一致、电影/剧集类型已核实一致、已有年份证据无冲突、季集唯一对应，且该来源恰好一个候选满足上述全部条件时，保存对应关系（`confirmed_by=auto`）。其他候选进入人工确认。
6. 否则推送 `confirm` 事件，候选附 `mismatch`（片名不完全一致、缺年份等）。

### 6.5 对应关系

- 键：`(metadata_source_id, type, meta_id, source_id)`；值：来源内容 ID、来源版本、确认方式（`auto` / `user`）、稳定证据（片名、年份、类型、季标识）、时间。
- 每次使用时核对当前详情身份和目标集。新增剧集或播放线路属于正常更新，继续复用对应关系；片名、类型、季标识等身份冲突时重新匹配。映射只确认内容身份，具体线路每集重新选取。
- 用户确认（`user`）优先于自动确认；用户撤销后该来源不再自动确认同一内容。
- 不保存播放地址。

### 6.6 解析播放地址

- MacCMS：分组中的地址即播放地址（`$` 分隔）；非 http(s) 地址标为不支持。
- T4：调用 `play`，按 `parse`、`jx`、`url`（字符串或「名称, 地址」交替数组）、`header`/`headers` 解释；`jx=1` 或 `parse=1` 时，用与 `flag` 匹配的 json 解析接口解析，无可用接口则 `unsupported`。
- Stremio：取带 `url` 的流，`behaviorHints.proxyHeaders.request` 作为请求头；`infoHash` 流忽略。
- 解析接口：`GET api + 页面地址`，取 JSON 中的 `url`（字段名以实际接口为准，导入时可配置）。

### 6.7 Stremio 播放 ID 与格式（第三期）

- 登记 manifest 后按 `resources`、`types`、`idPrefixes` 判断是否适用于该影片。配置 URL 和实际声明的能力均保留，按声明请求流。
- TMDB 内容通过其 external IDs 获取 IMDb ID；电影请求 `tt…`，剧集按已核实的季集请求 `tt…:season:episode`。第三方元数据只采用已核实的 IMDb 外部 ID 或符合其已登记命名空间的原始 video ID。
- 来源要求 IMDb 而内容缺失时，返回 `unsupported` 与 `imdb_id_missing`；ID 前缀或类型不支持时返回对应原因，继续其他来源。用户输入的任意字符串须经过内容归属校验。
- 只将本项目播放器支持的 HTTP(S) HLS / 普通文件作为可播放线路；DASH、网络 MKV、外部跳转等按实际支持情况显示原因。Torrent 流按既定范围忽略。请求头沿原生网关传递，媒体协议和格式均校验。

## 7. 服务端内部

### 7.1 数据表

| 表 | 主要字段 |
|---|---|
| `sources` | id、client_config_id、name、kind、api、ext、headers、enabled、parse_ids、revision、status、checked_at |
| `parsers` | id、name、api、flags、headers、enabled、revision |
| `metadata_sources` | id、kind、manifest_url、逻辑来源身份、enabled、revision |
| `mappings` | id、metadata_source_id、type、meta_id、source_id、source_revision、content_id、confirmed_by、evidence、updated_at、revoked |
| `line_stats` | source_id、content_id、group_name、ok_count、fail_count、last_ok_at、last_latency_ms |
| `meta_cache`、`catalog_cache` | key、lang、payload、expires_at |
| `home_snapshots` | lang、built_at、payload |

第 5.8 节的查找记录和含播放地址的短期缓存为有界内存状态，重启后重新查找。

### 7.2 适配器接口

所有来源实现同一组操作，内部统一为 MacCMS 风格的 `vod_*` 模型（MacCMS 与三种 T4 的详情本来就使用这套字段与 `$$$` / `#` / `$` 线路格式）：

- `search(query, page) -> Vec<Card>`
- `detail(content_id) -> Vod`（含分组与集）
- `play(flag, episode_id) -> Play`（直链 / 需解析 / 不支持）
- `stremio_stream` 另有 `streams(type, video_id)`。

### 7.3 网络与资源

- 来源、图片、媒体、字幕及 HLS 子资源沿 ADR-068 允许公网、内网、回环和代理 Fake-IP。共享 HTTP 保留固定公共 DNS、HTTP(S) 协议检查、TLS 证书校验、HTTPS 降级限制、跨 origin 敏感头清除、跳转和响应上限。App 到已配置服务的令牌请求绑定 origin 和基础路径；局域网服务域名可通过该专用客户端使用系统 DNS，避免公共 DNS 无法解析本地名称。访问服务端点与导入来源均由用户配置，服务使用单用户鉴权。
- 每个来源并发 2、全局并发 16；单来源请求超时 8 秒（初始值）。
- 遇验证码、Cloudflare 挑战页即判定该来源失败（沿用 `maccms.rs` 的识别），不尝试绕过。
- 预取（第四期）：每日首页快照后，对热门前 50 部影片预先建立对应关系，不解析播放地址。

### 7.4 媒体转发（第四期，可选）

部分播放地址绑定请求方 IP，App 直连会失败。可选提供 `/v1/media/{ticket}`：服务按短时会话票据请求媒体并转发，票据在会话内支持多次 Range 与 HLS 子资源请求。代价是服务带宽，默认关闭，按来源开启；实施前再细化协议。

### 7.5 安全

- 只有持令牌的设备可访问；令牌可在服务端轮换。
- 不提供任意地址代理接口；转发票据只对服务自己解析出的地址签发，短时有效。
- 来源返回的数据按不可信输入处理：响应大小上限、字段校验、非 http(s) 地址拒绝。
- App、服务日志与诊断统一隐藏路径令牌、Authorization、TMDB Key 和含凭据的上游 URL。反向代理示例默认关闭 Stremio 路由访问日志；用户开启日志时须使用脱敏格式。第一期以样例请求检查输出一次即可。
- SQLite 和自动生成的令牌文件放在持久数据目录，限制文件访问权限；导入和配置更新成功后再移除本地配置，失败保留原数据。采用单用户令牌与本地文件即可。

## 8. App 改动

### 8.1 设置

- 新增「播放服务」：服务地址、访问令牌（随设置文件保存，沿 T0.41 对 TMDB Key 的处理）、连接检查。
- 「播放源」页改为显示并管理服务端的来源（调用 5.6），支持导入 TVBox / zyfun 配置。
- 「元数据」：第一期保留 TMDB 与自定义 Stremio 来源的选择、语言和增删改入口，通过服务管理。原内置 TMDB 在第一期未连接服务时继续工作，第二期结束后移除其直连接口；已有来源逻辑身份和记录按第 9 节保留。
- 移除「当前播放源」选择（D6）。

### 8.2 移除

| 内容 | 位置 |
|---|---|
| MacCMS 作为元数据：目录页、MacCMS 详情、搜索页 MacCMS 结果、`maccms` 元数据命名空间与同源直通 | `MacCmsCatalog.vue`、`App.vue`、`SearchView.vue`、`MediaRow.vue`、`WatchlistRow.vue`、`state.ts`、`host.rs` 的 `match_maccms` |
| 全局播放源 | `selected_playback_source`、`last_playback_source_key`、`selectSource(…, 'playback')`、`manualSource` |
| App 内跨源匹配 | `match_playback`、`MatchStore` 的跨源部分、`matching.rs` 中仅服务 App 的部分、`line-preference.ts` |
| App 内 MacCMS 来源同步与解析 | `maccms.rs` 中的来源同步、目录；共享部分移入 `moc-core` |
| 内置 TMDB 虚拟扩展 | `tmdb_api.rs` 移入 `moc-core` 供服务使用（Q5） |

### 8.3 新增宿主命令

| 命令 | 说明 |
|---|---|
| `start_line_round {round_id, metadata_source_id, meta_id, type, video_id}` | 宿主核对当前元数据身份后调用 `POST /v1/lookups`，读取 NDJSON 流，把播放地址与请求头保存在宿主内（不返回 WebView），按线路逐个测速 |
| `get_line_round {round_id}` | 返回轮次快照：阶段、来源状态、线路（不含地址）、测速结果、需确认候选。前端每 300 ms 轮询；不引入新的推送机制 |
| `confirm_line_candidate {round_id, source_id, candidate_id}` | 宿主调用 `POST /v1/mappings`，新线路并入本轮并测速 |
| `cancel_line_round {round_id}` | 中止查找、确认和测速，按 5.8 通知服务取消；释放本轮地址，已播放会话持有自己的地址 |
| `play_line {round_id, line_id}` | 用该线路地址与请求头登记 Resolution（复用现有 `resolution.rs`），之后照旧 `open_playback` |
| `refresh_line {round_id, line_id}` | 调用 `/v1/lines/{id}/refresh` |

测速沿用原方案定义：普通文件 `Range: bytes=0-65535` 取首批数据；HLS 取清单、必要时取变体清单、再取首个分片首批数据；走 `maccms::Http` 安全规则，绕过媒体缓存，不开会话；来源并发 4、单条 5 秒（初始值）。

轮次快照分别记录「来源查询结束」和「待测数量」。自动选择等待查询结束及本轮待测线路全部完成或超时；整轮默认预算 45 秒，到期把尚未完成和排队的检测标记超时，再从成功结果中选择。该预算用于避免无限等待，可按实际体验调整。用户可随时选用已测通线路；手动选择后，本轮迟到结果只更新列表。

宿主在 `open_playback` 后实际推进时调用 `/v1/reports`（`playing`），失败时上报 `failed`。

### 8.4 前端

- 详情页「播放」：按 8.3 等待查询和测速结束或整轮到期，再选择当前设备支持且测速成功的最快线路；仅有需确认候选时打开确认列表。「选择线路」允许用户提前选择已测通线路。
- 播放器「线路」入口：同一列表，换线保留进度。
- 进度提示：「已查询 5 / 8 个来源，发现 12 条线路」「正在检测 12 条线路」；结果逐条出现。
- 列表字段：来源、线路名、清晰度、格式、延迟或状态、服务端统计（如「近期成功 12 次」）。
- 需确认来源：显示「需确认」，展开候选（片名、年份、类型、集数、不一致字段），确认后加入列表。
- 播放失败（D4）：同一轮内每条线路先刷新重试一次，再按测速顺序尝试下一条并提示，已失败的线路本轮不再循环尝试；全部失败显示原因、重试和手动选择。句柄 410 时重新查找原影片和集并保留进度。
- 切集：新轮次；对应关系由服务复用，通常无需再次确认。
- 观看记录续播：按记录中的影片与集启动轮次，记录里的来源仅用于标注「上次播放」。
- 侧栏：综艺由服务首页分类提供（D7）；短剧、体育移除。
- 服务不可达：首页、详情、播放显示明确错误与重试；本地视频、直播照常。

### 8.5 协议

- `protocol/app.schema.json`：新增 8.3 的命令与预算；删除全局播放源字段、`maccms` 元数据命名空间、App 内匹配命令；`bridge.openapi.json`、`src/models.ts`、`commands.rs`、Tauri capabilities 同步。
- 新增 `protocol/service.openapi.json` 描述第 5 节服务端接口，作为服务与 App 的共同契约，契约测试双方共用。

## 9. 数据与兼容

- 沿 ADR-064/065 不做旧版本兼容：引用 `maccms` 元数据命名空间的观看记录与收藏不再读取；旧的线路记忆存储（`moc-video.lines.v1`）不再使用。
- 当前格式的 TMDB 观看记录与收藏保留。服务 TMDB 继续使用固定 `TMDB_CONFIG_ID`、原 addon key、`tmdb`/`imdb` 命名空间、内容 ID 和 video ID；App 按该逻辑来源登记服务端点，内容身份与服务地址、令牌和语言解耦。
- 当前自定义 Stremio 配置迁入服务时携带原配置 ID、addon key 和内容命名空间，并持久映射到 `metadata_source_id`。来源地址或令牌变更仅更新传输配置。已有收藏与历史按原逻辑身份解析到服务来源；续播通过新一轮查找选择播放源，原播放源 ID 只用于展示。
- 第一期开通服务即处理这些身份与记录的衔接，验收包含接入前创建的 TMDB 历史和收藏，以及一条已有自定义 Stremio 内容；第二期检查新选线流程的续播。元数据扩展被删除或明确改变内容身份时展示来源不可用提示，保留原记录。
- App 原有 MacCMS 配置在第二期可一键上传，按本地配置 ID 幂等导入并返回成功清单；仅在对应项保存成功后移除本地副本，失败项保留。第一期继续使用本地播放配置。已有自定义 Stremio 配置按相同原则导入，服务缺失时显示设置入口。

## 10. 测试与验收（个人娱乐应用）

每卡保留一组相关自动检查和一次本机关键操作检查；同一场景可覆盖多个要求，已有有效检查直接复用。真实服务仍按 Q1 在本机 Docker 运行，App 在当前 macOS arm64 检查，沿用项目资源执行器并清理本次创建的容器和临时数据。

| 范围 | 最小有效检查 |
|---|---|
| 第一期 | 本机/配置的局域网服务可连接；TMDB 与一个自定义 Stremio 元数据源的首页/目录、搜索、详情、语言；缓存命中与首次无快照后恢复；重启令牌复用、错误鉴权、日志脱敏；现有 TMDB 历史/收藏及自定义来源身份保留 |
| 第二期 | 少量 JSON/XML 夹具覆盖精确/全文搜索、类型和标题年份冲突、多候选确认；至少两个来源、一条失败来源；查询后延迟确认、撤销与过期、缓存命中和刷新；App 完成自动/手动选线、换线续播、切集后旧结果丢弃与记录续播 |
| 第三期 | 每个新适配类型一个代表性请求样本，补必要错误样本；Stremio 的电影/剧集 ID、缺 ID 与前缀不匹配；App 实际播放一条 T4 与一条 Stremio HTTP 线路 |
| 契约与安全 | OpenAPI 记录本卡接口和关键字段，用上述场景核对 App/服务一致；代表性检查覆盖配置端点与普通来源的访问规则、凭据脱敏和迟到结果处理 |
| 真实外部来源 | 有条件时检查实际 TMDB、播放接口的表现；外部临时故障单独报告，并保留受控样本的功能结果 |

验收以短时播放能推进、进度能恢复、关键失败能解释为准。长播、压力/并发容量、全平台安装、完整故障矩阵、重复全量 verify 和固定延迟/RSS 指标作为按需专项，前三期仅运行直接相关检查。报告简记命令或操作、结果和剩余问题；测试脚本沿用 JSON/JUnit 接口以兼容当前执行器，无须为凑用例数拆分场景。尚未运行的业务验证统一记 NOT RUN。

## 11. 分期与工作量

| 期 | 内容 | 难度 | 量级 |
|---|---|---|---|
| 一：元数据服务 | 按需抽共享代码；服务骨架、持久令牌、SQLite；TMDB 与自定义 Stremio 元数据；缓存、首次与每日首页；本机接入、身份与现有记录保留 | 中 | 2–3 周 |
| 二：播放服务与 App 选线 | MacCMS JSON/XML 适配器；匹配流水线与对应关系；`/v1/lookups`、`/v1/mappings`、刷新与上报；来源管理与导入；App 宿主轮次、测速、`play_line`；前端线路列表、确认、换线；移除全局播放源与 MacCMS 元数据路径 | 中 | 2–3 周 |
| 三：接口类来源 | T4 查询式、DRPYJS0、REST 式；TVBox type 4；Stremio 播放扩展与 ID 适配；json 解析接口 | 中 | 1–2 周 |
| 四：可选 | APP 后端接口（YsV2、AppGet）；媒体转发；热门预取；向其他 Stremio 客户端提供 stream | 待需求明确后估算 | 按需 |

第一期完成后 App 即可从服务获取首页与详情，独立可验证；第二期完成后替代现有 App 内匹配。

## 12. 决定（ADR-077；本版由 ADR-078 补充）

| 编号 | 问题 | 决定 |
|---|---|---|
| Q1 | 服务部署在哪里 | 先在本机 Docker 运行，稳定后迁到常开设备（NAS 或 VPS） |
| Q2 | 服务不可用时，App 是否保留本地直连 MacCMS 的后备 | 不保留，避免两套匹配实现；服务不可达时明确报错 |
| Q3 | 共享代码方式 | 按已批准的 Cargo workspace，分期迁入实际共用代码 |
| Q4 | Stremio 路由令牌放在路径中 | 接受，App、服务及反向代理均隐藏该段，令牌持久化 |
| Q5 | 未连接服务时是否保留 App 内置 TMDB | 保留到第二期结束，之后移除 |
| Q6 | T0.46 的处理 | 以本方案新卡取代 T0.46（T0.46 未开发）；第一级片名过滤（已在 App 内修复、未提交）先保留，第二期移到服务端 |
| Q7 | 媒体转发是否做 | 第四期按需，默认关闭 |
| Q8 | 第四期 APP 后端接口 | 先不做，等前三期稳定后再评估 |

本次用户已授权九项审查修订及验收简化，细化结果见第 3–10 节。方案和任务卡获准修订；开发启动仍等待用户另行明确指令。

## 13. 风险

1. **播放地址绑定请求方**：服务只返回地址与请求头，由 App 直连；绑定 IP 的地址需要媒体转发（7.4）才可用。
2. **T4 `parse=1` 网页地址**：没有 json 解析接口时只能标为不支持；web 解析需要无头浏览器，本方案不做。
3. **服务器 IP 被封**：机房 IP 更容易触发验证码；家用网络部署可缓解。
4. **第三方接口不稳定**：T4 服务与解析接口变化快，来源检查与状态上报用于尽早发现。
5. **依赖服务在线**：服务不可达时网络影片不可用（Q2）。
6. **合规**：仅个人使用（ADR-048）；TMDB 署名与缓存遵守其条款；服务不对外开放。

## 附：依据

- zyfun 源码（2026-11-11 读取）：`src/shared/config/film.ts`（类型编号：T0_XML=0、T1_JSON=1、T4_DRPYJS0=2、T4_DRPYS=6、T3_DRPY=7、T4_CATVOD=8 …）；`routes/v1/film/cms/adapter/t4Drpys.ts`（首页 `filter=true`、分类 `ac=videolist&t&pg&ext=base64({src})`、详情 `ac=detail&ids`、搜索 `wd&pg`、播放 `flag&play` → `parse`、`jx`、`url`、`header`、`parse_extra`，均带 `extend`）；`t4DrpyJs0.ts`；`t4Catvod.ts`（`/init`、`/home`、`/category`、`/detail`、`/search`、`/play`、`/action`）；`t3AppYsV2.ts`、`t3AppGet.ts`；`routes/v1/parse/analyze.ts`（解析：`api + 页面地址`，类型 web / json）。
- TVBox 类型编号：TVBoxOS 源码注释「0:xml 1:json 3:jar 4:remote」；type 4 的参数沿社区惯例，实现前用真实接口核对。
- 本项目：ADR-047/048（放弃 Android 兼容层、个人使用）、ADR-049/068（固定 DNS、放宽 IP 范围）、ADR-061（线路记忆）、ADR-064/065（不做兼容、TMDB 两种方式）、ADR-075/076（多源选线、解耦与两级筛选）、T0.41（`tmdb_api.rs`）。
