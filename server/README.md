# 播放服务

在项目根目录准备可选配置，然后启动服务：

```sh
mkdir -p config
cp server/service.example.toml config/service.toml
# 编辑 config/service.toml，填写 [tmdb] 的 key
chmod 600 config/service.toml
docker compose -f packaging/playback-service.compose.yml up -d --build
```

服务配置位于主机 `config/service.toml`，该目录已加入 Git 忽略列表。首次启动会在数据卷内生成访问令牌；从容器 `/data/service-token` 读取后填入 App。默认仅监听 `127.0.0.1:8787`。

| TOML 配置 | 环境变量（优先） | 默认值 | 生效方式 |
| --- | --- | --- | --- |
| `service.listen` | `MOC_SERVICE_LISTEN` | `127.0.0.1:8787` | 重启 |
| `service.data_dir` | `MOC_SERVICE_DATA_DIR` | `./data` | 重启 |
| `service.token` | `MOC_SERVICE_TOKEN` | 自动生成 | 重启 |
| `tmdb.api_url` | `MOC_TMDB_API_URL` | `https://api.themoviedb.org/3` | 重启 |
| `tmdb.key` | `MOC_TMDB_KEY` | 空 | 重启 |
| `home.timezone` | `TZ` | `Asia/Shanghai` | 重启 |
| `home.poll_seconds` | `MOC_HOME_POLL_SECONDS` | 60 秒 | 重启 |
| `lookup.handle_ttl_seconds` | `MOC_HANDLE_TTL_SECONDS` | 900 秒 | 自动重载 |
| `lookup.line_cache_seconds` | `MOC_LINE_CACHE_SECONDS` | 60 秒 | 自动重载 |

容器镜像通过环境变量固定监听 `0.0.0.0:8787` 和数据目录 `/data`。在工作目录启动程序时默认读取 `service.toml`；可用 `MOC_SERVICE_CONFIG` 指定文件路径，指定文件必须存在。环境变量优先于 TOML，旧部署继续沿用现有变量。Compose 会在设置了 `MOC_TMDB_KEY` 时传入该值；未设置时使用 TOML。其他环境变量可通过 Compose 覆盖文件设置。

服务约每 5 秒检查配置文件；两项线路有效期的范围为 1 至 86400 秒，新值影响后续创建或续期的对象。配置无效或文件暂时缺失时保留当前值并记录日志。首页按配置的时区在当地时间 04:00 更新。请限制配置文件访问权限，备份时同时保存 `config/` 和数据卷；令牌与 TMDB 密钥不要提交到仓库。

需要远程访问时使用 HTTPS 反向代理；`packaging/playback-service.nginx.conf` 提供关闭访问日志的示例。

## 来源与线路

通过服务 API 管理播放源、扩展和解析器：`GET/POST /v1/sources`、`POST /v1/sources/import`、`GET/POST /v1/parsers` 和 `GET/POST /v1/metadata-sources`。导入支持 TVBox、zyfun 3.4.1+ 和旧版 zyfun JSON；相对地址使用 `base_url`。脚本、jar 和网页嗅探类型会在导入结果中标明跳过。

例如，将配置保存为 `sources.json`，并在本机环境中设置服务地址和令牌后执行：

```sh
curl -fsS -H "Authorization: Bearer $SERVICE_TOKEN" -H 'Content-Type: application/json' --data-binary @sources.json "$SERVICE_URL/v1/sources/import"
```

查找时服务端访问来源并检测线路，状态、清晰度、描述和大小保存到数据库，缓存期为一天。App 先取得展示信息，用户选择后调用 `POST /v1/lines/{id}/play` 获取播放地址及请求头。地址与请求头保留在短期内存中；可重新解析的缓存线路在选中时获取最新地址。缺少稳定身份的线路保留检测信息缓存，重查时重新访问来源以确保选中正确。

接口定义见 `protocol/service.openapi.json`；`/v1/parsers` 支持配置 JSON 解析接口的请求头、页面 URL 参数名及响应 URL 字段路径。

TMDB 地区选项取自其国家和地区配置，电影与剧集按 `with_origin_country` 筛选作品来源地。类型、年份、语言、地区可与热门或最新排序组合，服务会将条件一并传给 TMDB 查询。
