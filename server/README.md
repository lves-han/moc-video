# 播放服务

在项目根目录执行 `docker compose -f packaging/playback-service.compose.yml up -d --build`。
默认仅监听本机 `127.0.0.1:8787`；容器的数据卷保存 SQLite 和自动生成的访问令牌。
从容器的数据目录 `/data/service-token` 读取令牌后填入 App，切勿将其写入日志。
启动前设置 `MOC_TMDB_KEY` 环境变量，Compose 会传递给服务。
其他变量可在 Compose 覆盖文件中设置，或直接传给 `docker run`。

- `MOC_SERVICE_LISTEN`：监听地址，容器默认为 `0.0.0.0:8787`。
- `MOC_SERVICE_DATA_DIR`：持久数据目录，容器默认为 `/data`。
- `MOC_SERVICE_TOKEN`：可选访问令牌；设置时优先于令牌文件。
- `MOC_TMDB_KEY`：TMDB v3 API key 或 v4 读取令牌，由服务持有。
- `MOC_TMDB_API_URL`：可选 TMDB v3 接口地址，默认 `https://api.themoviedb.org/3`。
- `TZ`：首页每日更新时区，默认 `Asia/Shanghai`；在当地时间 04:00 更新。
- `MOC_HOME_POLL_SECONDS`：检查更新的间隔，默认 60 秒。

需要远程访问时使用 HTTPS 反向代理；`packaging/playback-service.nginx.conf` 提供关闭访问日志的示例。

## 来源与线路

通过服务 API 管理播放源、扩展和解析器：`GET/POST /v1/sources`、`POST /v1/sources/import`、`GET/POST /v1/parsers` 和 `GET/POST /v1/metadata-sources`。导入支持 TVBox、zyfun 3.4.1+ 和旧版 zyfun JSON；相对地址使用 `base_url`。脚本、jar 和网页嗅探类型会在导入结果中标明跳过。

例如，将配置保存为 `sources.json`，并在本机环境中设置服务地址和令牌后执行：

```sh
curl -fsS -H "Authorization: Bearer $SERVICE_TOKEN" -H 'Content-Type: application/json' --data-binary @sources.json "$SERVICE_URL/v1/sources/import"
```

查找时服务端访问来源并检测线路，状态、清晰度、描述和大小保存到数据库，缓存期为一天。App 先取得展示信息，用户选择后调用 `POST /v1/lines/{id}/play` 获取播放地址及请求头。地址与请求头保留在短期内存中；可重新解析的缓存线路在选中时获取最新地址。缺少稳定身份的线路保留检测信息缓存，重查时重新访问来源以确保选中正确。

接口定义见 `protocol/service.openapi.json`；`/v1/parsers` 支持配置 JSON 解析接口的请求头、页面 URL 参数名及响应 URL 字段路径。

TMDB 地区选项取自其国家和地区配置，电影与剧集按 `with_origin_country` 筛选作品来源地。地区可与类型、年份和语言组合；地区筛选下可使用当前栏目支持的热门或最新排序。
