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

## 添加与导入播放源

在 App 的元数据设置中填写服务地址和令牌，连接后进入播放源设置。可单独添加 MacCMS JSON/XML、T4 Query、T4 DRPYJS0、T4 REST（CatVod）或 Stremio Stream；T4 扩展参数使用 JSON，Stremio 地址填写 `manifest.json`。

选择“导入配置”，粘贴 JSON 并选择 TVBox、zyfun 3.4.1 及以上或 zyfun 旧版；需要解析相对地址时填写配置文件的完整 URL。逐项结果显示成功、跳过或失败；重复导入根据来源身份更新已有配置。解析器按来源的关联及线路 flag 选择，空 flag 匹配全部线路。影片详情页的“选择线路”和“刷新”可重新查询当前影片的线路。脚本、jar、网页嗅探等格式会显示跳过。

服务接口见 `protocol/service.openapi.json`。`/v1/parsers` 支持配置 JSON 解析接口的请求头、页面 URL 参数名及响应 URL 字段路径；这些请求头只用于访问解析接口，媒体请求头由来源或解析结果提供。
