# 交接：从 T0.24 起由 Codex 接手

更新于 2026-11-06。本文件说明当前状态、下一步和必须沿用的做法；权威来源仍是 `docs/TASKS.md`、`docs/DECISIONS.md`、`docs/PROGRESS.md` 和 `scripts/task.py status`，有出入以它们为准。

## 当前状态

- 已完成并通过独立巡检（CLEAR）：T0.19、T0.20、T0.21、T0.22、T0.18、T0.23。最新提交 `da4a099`，工作树干净。
- `task.py next` 返回 T0.24。之后依次为 T0.25、T0.26、T0.27、T0.28、T0.29。M 系列（移动端）要等桌面版稳定上线并取得用户另行授权，现在不做。
- 8 个支持的 JSON 来源：优质、量子、红牛、无尽、光速、极速、豪华、淘片（`docs/compatibility-report.json`）。卧龙域名在出售，不做预置。其余 6 个插件站点按 ADR-048 暂缓。
- 本轮新增的关键决定：ADR-050、ADR-051（兼容性和来源可用性分开判定）、ADR-052（同一 UUID 且同一端点重加视为同一来源）。

## 下一步：T0.24 需要先由用户决定

T0.24 要求从真实的 Stremio 元数据扩展出发，经推荐、详情、匹配确认，到 JSON 来源播放和切集。规格只写“用户填写 manifest 完整 URL”，没有指定扩展。8 个来源都是中文片名，元数据扩展必须能给出中文片名才匹配得上。Cinemeta 只有英文片名，基本匹配不上。

已向用户提出两个选项，尚未得到答复：

1. 先找一个公开、能返回中文片名、无需账号或密钥的 TMDB 扩展，只发普通请求核实可用性，再请用户确认。
2. 由用户直接提供 manifest 地址。

在用户答复前不要自行选定扩展，也不要使用需要凭据的实例。

## T0.24 要一并处理的遗留项

- BACKLOG「T0.18巡检保留的历史展示P2」：原生历史条目把集号向上取整，`episode_total` 等于当前集。剧集第 1 集会显示成 “Feature”，2.5 集显示成第 3 集。海报固定为占位图，没有用 `poster_cache_key` 或当前元数据补齐。代码在 `src/data/history.ts` 的 `entryFromRecord` 和 `src/views/HistoryView.vue`。
- T0.23 巡检建议补两项测试：一项是事务撤销资源后发布失败的恢复，若确有可达路径才补；另一项是成功同步且 `changed=false` 时，原 Session 和 cap 继续有效。
- 窗口位置和尺寸恢复目前只测到 Host 读取，没有在真实重启中观察过。非 UUID 的 `match_id` 只测到 Session 绑定和 schema。磁盘写满只按错误码映射，没有实测。
- 多轮巡检都指出报告引用的 `samples-1.jsonl` 缺失，原始 RSS 采样无法复核。
- 新增依赖 `rusqlite`（bundled SQLite）的许可清单归 T0.27。

## 每张卡的固定流程

1. `.venv/bin/python scripts/task.py check`，再执行 `task.py start <卡>`。
2. 在卡片范围内实现，先跑相关离线测试：`run.py diag-... -- cargo test ...`、`node --experimental-strip-types --test ...`、`npx vue-tsc --noEmit`、`cargo clippy --offline --all-targets -- -D warnings`。
3. 用 WIP 提交固定源码，例如 `T0.24: WIP ...`；提交信息末尾加 `Co-Authored-By` 行。
4. 按卡内 argv 原样执行 A1：`.venv/bin/python -B scripts/run.py A1 -- '{python}' -B tests/acceptance/<小写原卡号>.py --suite all --report evidence/acceptance/<原卡号>-all.json --junit evidence/acceptance/<原卡号>-all.xml`。
5. 执行 `scripts/verify.py all`，六门全部 exit 0。
6. 勾选 TASKS 里本卡的完成框，在 PROGRESS 追加 DONE 记录（写明 A1 日志、verify 收据、断言摘要和 Remaining），执行 `task.py done <卡>`，再提交 `<卡>: <摘要>`。
7. `.venv/bin/python -B scripts/drive.py --review-only` 发起独立巡检。CLEAR 才进入下一张卡。

本机曾用的脚本 `/private/tmp/.../scratchpad/finish.sh` 只是把第 4 至 7 步串起来，不在仓库里，Codex 需按上面的步骤自己执行。

## 已踩过的坑

- **锁屏：** 打包 WKWebView 验收要求屏幕解锁。每次跑 A1 前先检查 `t01.native_gui_session()["screen_locked"]`，为 True 时必须中止，只打印出来不算。显示器 10 分钟会休眠。
- **失败计数：** `run.py` 按日志末尾计算失败特征，不同原因的失败常被算成同一种。同一特征累计 3 次出现 STUCK，5 次出现 STUCK-HARD，当前会话会被禁止继续重跑。T0.18 就因一次锁屏失败到了 5 次，后来经用户明确批准，才把会话计数从 0 改为 1（见 PROGRESS「T0.18 STUCK-HARD 恢复决定」）。计数到 3 以后，先把整个场景对照实现核对清楚再跑；不得自行改状态文件。
- 任何失败运行之后，重跑前必须先写 `task.py note`，说明新的假设和改动。
- 巡检 BLOCKING 必须修复。重开已勾选的卡时，先把 TASKS 的完成框改回 `- [ ]`，修完后按完整流程重走。绝不能自行 `drive.py accept-review`。
- 巡检偶尔因模型满载以 `nonzero_exit` 失败，这与代码无关，直接重新执行 `--review-only` 即可。
- 修改冻结内容（TASKS 验收、ADR 等）需要用户决定：在 PROGRESS 写一条 BLOCKED，再写一条带 `- 决定：` 和 `- 批准摘要：<protected_hash>` 的 RESOLVED，然后执行 `task.py rebaseline <卡> --protected <hash>`。T0.20 的 ADR-052 是范例。

## 验收脚本的可复用部件

- `tests/acceptance/t14.py` 中的 `packaged(...)` 负责打包窗口场景，支持 `command_timeout` 和 `after_close`（真实关窗后，再从 Host 核对数据）。
- `tests/fixtures/n02_host.rs` 是真实 Host 的桥接，支持 `restart`、`lock_history`、`unlock_history` 操作；`tests/acceptance/n02.py` 中的 `Bridge` 会核对重启次数。
- WebView 场景写在 `tests/fixtures/<卡>_webview.ts`，在 `tests/fixtures/t01/automation.ts` 中按 op 名注册，参考 `n02_webview.ts`、`t18_webview.ts`。
- 新的原生测试名需要加入 `t02.NATIVE` 的扩展集合，否则测试名会被日志脱敏吃掉，参考 `t03.py`、`t18.py`。
- 公开来源取证用 `t02_capture_public_json`（ignored 测试），由 `run.py diag-...` 执行。

## 用户的约束

- 用中文回答，直接说结论；遵守 `AGENTS.md` 的句式限制。每次结束按 Changed / Why / Verified / Remaining 汇报，没验证的如实写明。
- 项目已整体交给开发方：按卡片顺序推进，不必每张卡都询问。只有用户才能做的事才打断用户：解锁屏幕、提供凭据或设备、授权进入受控阶段、修改冻结内容。
- 验收标准按娱乐软件定位（ADR-044），不要加严；安全、鉴权、数据完整性和功能正确性不放宽。
- 仅供个人使用，没有公开发行授权。不得绕过验证码、Cloudflare 或访问控制，遇到就停止该来源并报告。TLS 校验保持开启。凭据和带 capability 的 URL 不得进入代码、日志或报告。
- 来源能否播放、有没有内容属于来源自身状态，不能判为程序缺陷；程序负责兼容来源的公开响应，只做最小兼容。
- Git 不推送、不改写历史。`PROGRESS.md`、`DECISIONS.md`、`BACKLOG.md` 只追加。
