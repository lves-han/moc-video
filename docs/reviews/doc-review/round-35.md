# Round 35：T0.12 唯一第七会话授权独立文档复审

结论：ADR-036及对应RESOLVED准确落实真实人类授权。唯一一次6→7、原范围资源持有诊断、依据证据的最小修复、原aggregate RSS门及完整完成条件明确。未发现阻塞本次授权增量的文档问题。第七会话实现与新源业务复验均为NOT RUN。

## 受审基线

本轮基于round34，仅审查本次授权增量及相关证据。

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`cae589c36015a18b5b7565007bcd57ac5ef8b302`
- 原摘要函数实际source：`c2f88c950fce441fa4285441aed1c3fc745b49ad16948bada7c5f46d97325fb0`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-036分段SHA256：`87bb8edf873c645c7e7228f365e78d6f0ea9ad1be7ca7a4679cbda73fb50087c`
- 工作树干净。固定提交仅追加DECISIONS 10行、PROGRESS 8行，两文件保留父提交完整字节前缀，前35份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `b33ff797217e80eb6b761962917cb1e470dc18c343f1c7dd04bfcbc9de392f91` |
| docs/PROGRESS.md | `74da78a504116e0ec0e479c2ceec09e16a195a1eafabe40c2f31e2ac500baee4` |
| docs/TASKS.md | `438718e8b3dca5173a141b66b2df4688e26f5a7dc03e6c10d7eee4a21e2f12d8` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-34.md | `7a52fd044ec06218f4949e60978428a635fec8695dba73eef9c9e32be08f2650` |

## 真实授权、覆盖与可实施性

沿用前轮已读取的moc-init、AGENTS、PROMPT-DOC-REVIEW、T12完整原卡、SPEC10.3/10.5、C9/C14、ADR010与原控制器语义，本轮核对round34、ADR036/RESOLVED及当前原始证据。

root可信转述的真实用户回复为“批准 T0.12 再追加一次（6→7）”。此前询问明确说明第六会话耗尽、画面音频通过、原RSS趋势门再次失败且尚未运行到EOS，增长集中WebView renderer/GPU，已观察对象计数稳定但原因未定；仅追加一次继续原范围诊断、最小修复与完整复验，原50规模、RSS门、安全要求、权限及时限保持。受审文本准确记录该决定。

本次诊断直接服务T12原有50次切集、CPU/内存及媒体资源回收义务。T12的R08、R23、R30、R32、R34、R48、REQ-T12和冻结A1 cases保持。授权允许在原前端与tests范围定位renderer/GPU持有，依据真实数据区分浏览器缓存/解码资源、实际未释放对象和验收自身持有，再最小修复确认原因并增加正负例。现阶段根因尚待实证，未预先认定缓存或泄漏。

ADR036明确保持完整50次真实切集、51份aggregate CPU/RSS样本、十个五切窗口原趋势门、原单位阈值和每轮当前红蓝帧、440/880非零PCM、Session/URL绑定、旧音频旧帧拒绝及媒体/HTTP会话回收。稳定对象计数仅作为诊断证据，原RSS门继续独立判定。删除或弱化门、改阈值单位、抽样本、人为warmup或强制GC、同源反复筛选通过均被禁止；实质指标、门或测法变化须另取真实决定。

第六源EOS测试修正继续保持已播放直接seek、暂停先正式按钮resume再seek、原20秒总期限、真实trusted DOM ended、当前epoch/session及一次事件映射。原C9资源恢复、安全身份、查询次数与3秒预算、图片30秒链路、errors、autoplay、unknown、EOS及清理门均要求新源完整实测，旧源成功保留为历史。

实际严格首张未完成卡、current及原`next_card()`均为T0.12；T0.1至T0.11完成判定成立。T12为s6/f9/g0、hard_sessions字段不存在；保存的`seventh-authorization/original-status.json`一致，第七许可尚未消费。T13保持paused/s1/gen3。

原控制器累计上限及启动前递增语义配合唯一一次`drive --once --max-sessions 7`与紧邻顺序/身份/锁/clean/source/protected核对，可落实唯一第七会话。实际s7即消费，第八未授权，其他卡default2，失败和会话历史保留。原4096MiB macOS RSS采样、jobs2、线程1、权限与全部时限保持。

原CLI负责实施与相关真实自验；已知限制沿ADR010准确WIP稳定退出后交外层原run验收。新源准确A1成功后直接完整verify、原done、受验完成提交、新固定独立CLEAR与clean齐备才完成。GUI锁定时等待解锁，后卡严格等待。冻结定义、控制脚本和全局配置未改，protected保持，无需rebaseline。未发现本次增量造成的新路线阻塞。

## 当前失败事实与不确定性

当前指针指向：

`evidence/drive/t12-sixth-validation-20261103T054643Z`

指针SHA256：`9e18c9d70deb195c9bcda70768840f80cda40458be6a3d4d737796fd12fad66d`。

`rss-recurrence-failure/manifest.json`的SHA256：

`4ab99fd41b16654eca55674314dd7d1d79bf3a96def12ade36180b362b937e17`

清单中39份副本均存在且全部符合记录SHA256。

最新准确A1为`evidence/runs/20261103T054657730910-A1`，退出1、s6/g0、source前后一致。报告5必需、5实跑、4通过、1失败；JUnit共12项，4通过、8失败、0跳过。保存的oracle日志为23项通过，行为测试64项通过，类型及native check/clippy收据退出0。

只读核对原始保存数据确认：

- 50次切集按1至50完整排列，现有逐轮帧、非零PCM、Session绑定、单播放器与回收条件满足。
- 51份样本完整。十窗口中位数重算为`288751616、291094528、294174720、298631168、301137920、311427072、312688640、316358656、318521344、318668800` bytes，严格递增，原门拒绝。
- 首末为相同PID及创建身份。重算RSS变化：application下降5341184 bytes、renderer增加44515328 bytes、GPU增加20299776 bytes、network增加1163264 bytes。
- DOM71、text track1、监听注册9、Session1、HTTP0、scratch24576 bytes及1 canvas保持；旧轨弱引用末次alive6，connected/enabled均0，resource timing entries达到250。
- native finally退出-15，lingering为空，合并峰RSS为637648896 bytes。

上述数据支持“renderer/GPU RSS增长且已观测活跃对象数量稳定”。具体缓存、解码资源或实际持有根因仍未确认；原RSS趋势失败保持，未据计数稳定宣称资源门通过。

本轮在RSS门停止，新源nativeHLS、controls、expire、图片、errors、autoplay、unknown、真实EOS、layout、正常stop/dispose、Host finish与完整verify均为NOT RUN。EOS helper及相关测试通过尚待本源真实EOS验证，旧第五源后续成功仅保留历史。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要及控制器只读函数、保存数据重算、日志/收据核对和39份副本SHA256校验。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未构建测试、启动CLI或取锁，未输出cap凭据。旧进程身份及两锁空闲仍须启动前紧邻核对。

第七会话诊断、根因确认、最小修复与回归、新源完整准确A1、完整verify、done、受验提交和新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权与规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
