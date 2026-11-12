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

需要远程访问时使用 HTTPS 反向代理；`packaging/playback-service.nginx.conf` 提供关闭访问日志的示例。
