# Round 31：T0.12 唯一第三会话授权独立文档复审

结论：ADR-032及对应RESOLVED准确落实真实人类授权。追加范围、图片过期链路诊断、原规模与质量门、执行分工和完成条件明确，未发现阻塞本次授权增量的文档问题。第三会话实现及新源业务验收均为NOT RUN。

## 受审基线

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`8b415244b6b2b561672edb9c8f3048839b8523ca`
- 原摘要函数实际source：`1178265c43c21db0a639bb4dde16b6d12c278ed5b6c690528d6585f45e95cd3b`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-032分段SHA256：`c7417df2199f93cde98bd89d2bd3c61ec1ebb8473c35f3662cce2e5ecda1580e`
- 工作树干净。固定提交仅向DECISIONS追加9行、PROGRESS追加8行；两文件均保持父提交完整字节前缀，前31份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/PROMPT-DOC-REVIEW.md | `399371107d92e4172312182ba7e888fbe41f377a35c57d6109ccb852ef7382c0` |
| docs/DECISIONS.md | `8b17bc5b179b42ab864b2f943f9ebf7bd43444fd0e81cff59e140771c809d45b` |
| docs/PROGRESS.md | `dee8d4b1bbd2c389edff3b4d1e8d2f95cc695cfb7a493a7b131f61ba2949032f` |
| docs/TASKS.md | `438718e8b3dca5173a141b66b2df4688e26f5a7dc03e6c10d7eee4a21e2f12d8` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-30.md | `89697d0f8d34821724f5b952ad99ff72e449510fd60ef5cb2f09af486761362c` |

## 真实授权及需求覆盖

已读取moc-init、项目AGENTS、PROMPT-DOC-REVIEW、round30、T0.12完整任务卡及覆盖关系、SPEC10.3/10.5/11.3、C9、ADR010/020/032、对应PROGRESS、原控制器相关语义和当前失败证据。本轮针对既有审查基础上本次授权增量复审。

root可信转述的真实人类回复为“批准 T0.12 追加一次”，对应询问明确限定：本卡2→3、原CLI补真实图片请求与错误诊断、定位后最小修复并完整复验，原权限、50次规模、安全门、资源、时限和历史保持，后卡等待。ADR032与RESOLVED准确记录该来源和约束；第三许可未消费，未授权第四会话。

T12覆盖表与卡内需求保持一致：R08、R23、R30、R32、R34、R48、REQ-T12均进入准确A1的冻结cases，无新增skippable。图片LRU过期恢复属于SPEC10.5/11.3、C9及T12原始义务，诊断与修复可在原卡生产前端和tests范围内完成。

本次要求保留完整可观察链路：

- 实际4096-entry LRU淘汰后，确实发生过期图片的原生请求及load/error观测。
- 记录资源URL身份、阶段诊断及Host查询计数，区分缓存复用、DOM错误传递和生产身份回调。
- 正式页面原生img错误触发一次只读状态查询，仅刷新所属DTO一次，并确认新图片重新decode。
- 查询及返回校验request_id、host_epoch、source_revision、页面/播放epoch与target_id；旧结果、错误身份及重复恢复有反例。
- 保持video/img/track每加载尝试最多一次查询、3秒只读查询、播放一次ResumeSnapshot恢复、401重连、502上游、codec分类和查询失败诊断等原门。

上述要求能够拒绝直接调用error回调、mock替代真实图片链路、错误DTO刷新、重复恢复、旧查询污染或仅以无报错宣称成功的实现。

原50次真实切集、每轮红蓝帧和440/880非零PCM、Session与URL绑定、旧帧旧音频拒绝、单video/HLS/track/监听清理、HTTP与会话回收、CPU/RSS与无单调资源增长门均明确保留。原30秒边界和各命令时限保持，未借ADR020迁移或缩减本卡功能规模。

## 已确认失败与待验证根因

只读核对当前指针：

`evidence/drive/t12-current-validation-dir.txt`

其内容指向：

`evidence/drive/t12-second-validation-20261103T022445Z`

指针SHA256为`aa16ab351175fef04773da02a14c2a2636227ca88a65c701e0387850022a65bb`。

已核对该目录`image-expiry-failure/manifest.json`，SHA256为`44d23133648b568b03f873962a67d7791f1191885ea2109fceab51cd0951509c`。清单中35份副本均存在且全部符合SHA256，覆盖原A1 JSON/XML、日志、收据、Host/native结果及相关源码快照。

原报告的准确事实为：

- A1：5必需、5实跑、4通过、1失败；JUnit共12项，4通过、8失败、0跳过。
- 原始cycles实际50条；native-HLS、controls及播放expired恢复结果存在，controls保留Resolution，expired恢复布尔断言通过。
- 图片过期链路停于`Image error did not query/refresh its DTO`。
- 后续errors、autoplay、unknown duration、EOS、layout、正常stop/dispose、Host finish和完整verify未执行。
- 原native最终退出-15，lingering为空；周期samples缺失已在manifest如实记录。

静态定位线索进一步支持诊断必要性：`gateway.rs`实际图片响应设置`Cache-Control: private, max-age=60`；`t12_webview.ts`在LRU淘汰后移除src并立即重新赋同一URL，默认等待20秒。浏览器缓存复用具有代码依据，当前现场仍缺实际失效HTTP请求及原生load/error阶段证据，具体根因待验证。ADR032准确保留该不确定性，并要求先补真实观测再修实际原因，未提前宣称生产恢复函数有错，也未扩大到范围外Rust或改变生产缓存语义。

## 预算与路线可实施性

实际只读核对：

- 严格文件顺序首张未完成卡、current及原只读`next_card()`结果均为T0.12；T0.1至T0.11完成判定成立。
- T0.12：sessions2、failures4、generation0、paused=false、hard_sessions为空。
- T0.13：paused=true、sessions1、generation3；其未解除阻塞保持。
- 原保存的`third-authorization/original-status.json`亦为T0.12、s2/f4/g0及相同protected。
- workflow默认max_sessions仍为2；仅一次原`drive --once --max-sessions 3`，实际累计计数3即消费本次许可。
- A1定义超时3600秒，完整verify定义合计5040秒，共8640秒；原单会话10800秒未改变。完整运行及修复实际耗时仍须受原会话约束。
- 原workspace-write开发/read-only巡检、4096MiB macOS RSS采样、jobs2、线程1、原绝对共享重锁路径、driver锁及全部超时保持。

当前第一张待闭环卡为T0.12，本次授权提供原流程内可执行的定位与最小修复路径。启动前仍须紧邻复核顺序、计数、clean/source/protected、旧进程PID与创建时间以及两锁实际空闲；本审阅未获取锁或核验实时进程存活。

原CLI负责实施和相关真实自验证；已确认权限限制沿ADR010交接，外层仅在CLI退出、源码稳定及旧身份结束后，通过原执行器完成相关构建/类型/行为与真实WK准确A1。成功后直接完整verify、原done、受验完成提交、新固定独立CLEAR和干净树齐备，才可完成本卡并推进。GUI锁定时等待解锁，原门保持。

此次仅授权单卡额度及原范围内修复；实际protected保持，冻结定义和全局配置未改，无需rebaseline。未发现本次授权增量造成的新文档路线阻塞。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要函数、状态与原控制器只读函数、报告结构/计数读取及副本SHA256核对。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未运行构建/测试，未启动开发CLI，未获取共享锁，未输出cap凭据。

第三会话实现、图片请求与错误诊断、根因确认、最小修复及回归、新源50次完整真实链路、准确A1、完整verify、done、受验提交和新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权与规划充分，业务结果按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
