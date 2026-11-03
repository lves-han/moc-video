# Round 32：T0.12 唯一第四会话授权独立文档复审

结论：ADR-033及对应RESOLVED准确落实本次真实人类授权。一次3→4的会话增量、RSS资源持有诊断、依据证据的最小修复、原规模和质量门、受限执行分工及完整完成条件明确。未发现阻塞本次授权增量的文档问题。第四会话实现与新源业务复验均为NOT RUN。

## 受审基线

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`dca9c384cb171be8cf297dc59ac13e6572bbda10`
- 原摘要函数实际source：`0bf54dfbc4159459f1d83f3eaa44c9e7f133b759102739118fa8f0952edc4fbe`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-033分段SHA256：`b7ac3a6796442e301c136ccb6421ca79c86ec43e261bc676504011cfe37fdde0`
- 工作树干净。固定提交仅追加DECISIONS 10行、PROGRESS 9行；两文件均保持父提交完整字节前缀，前32份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/PROMPT-DOC-REVIEW.md | `399371107d92e4172312182ba7e888fbe41f377a35c57d6109ccb852ef7382c0` |
| docs/DECISIONS.md | `1da52e4f577a08cbbf8aa23f11771777ed64a9e5f4075ea67151d671ea790bbd` |
| docs/PROGRESS.md | `7cab74912d512dc9375e38ba3bb656ca8ce43878a431e67721c1c43b9374e45d` |
| docs/TASKS.md | `438718e8b3dca5173a141b66b2df4688e26f5a7dc03e6c10d7eee4a21e2f12d8` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-31.md | `666bed2ec580c7c75bcddb97d80917586bd5ca8e3b1f1f5fd6bc9bd9e6fe3efc` |

## 授权增量与需求覆盖

已读取moc-init、项目AGENTS、PROMPT-DOC-REVIEW、round31、T0.12完整原卡及覆盖关系、SPEC10.3/10.5/11.3、C9/C14、ADR010/020/033、对应PROGRESS、原控制器相关语义及当前失败证据。本轮基于round31实施授权增量复审。

root可信转述的真实人类回复为“批准 T0.12 再追加一次”，对应询问明确说明第三会话已耗尽、50次切集画面音频通过、十窗口内存趋势门失败且原因待定位、图片及后续恢复未执行；仅追加一次3→4，由原CLI补资源持有诊断、最小修复并完整复验，原规模、安全门、权限、资源及时限保持。ADR033与RESOLVED准确记录该来源。

T12的R08、R23、R30、R32、R34、R48、REQ-T12继续进入原准确A1的冻结cases，无新增skippable。新增诊断围绕原卡已有CPU/内存、媒体资源回收与50切集义务，覆盖创建身份进程RSS、媒体/DOM/轨道/监听、Session/HTTP及测试观测持有量；允许依据真实数据区分缓存暖机、实际保留与验收自身持有，再修复确认原因并增加相关正负例。原范围足以开展该定位，根因尚待实证。

ADR033明确保留完整50次真实切集、51份CPU/RSS样本、原十个五切窗口无单调资源增长门，以及每轮当前红蓝帧、440/880非零PCM、Session与URL绑定、旧帧旧音频拒绝、单video/HLS/track/监听及HTTP会话回收。删除门、修改阈值或单位、抽掉样本、人为warmup或强制GC、同源重跑筛选通过均被明确禁止；指标、门或测法实质变化须另取真实决定。

图片与资源恢复继续满足SPEC10.5/11.3及C9：真实LRU淘汰后原生410与trusted error，生产一次3秒只读status，校验request/host/revision/epoch/target，所属DTO一次刷新并真实trusted load/decode；播放过期一次ResumeSnapshot、401/502/codec分类、查询失败与旧查询丢弃均保持。直接invoke错误回调或mock替代真实原生链路会被拒绝。

## 当前失败证据核对

当前指针`evidence/drive/t12-current-validation-dir.txt`指向：

`evidence/drive/t12-third-validation-20261103T030541Z`

指针SHA256：`33e446993bd97b9e7e5e09b557c393eec80108d67a7d175e012065ae35ac8f3b`。

其`rss-growth-failure/manifest.json`的SHA256为：

`4d28802c5099170732fcea82ecd2e17c1a2c03e3b225504298eb059205f22b06`

清单中41份副本均存在且全部符合记录SHA256，覆盖原报告、每轮结果、CPU/RSS样本、Host/native记录、源码及原收据日志。

最新准确A1为`evidence/runs/20261103T031024087377-A1`，实际退出1，source前后均为本轮受审source，session3/generation0。报告5项必需、5实跑、4通过、1失败；JUnit共12项，4通过、8失败、0跳过。

只读重算已保存数据确认：

- 50条实际切集结果按1至50完整排列，逐轮帧、非零PCM、Session绑定、单播放器、旧轨道与HTTP会话断言满足现有验证条件。
- CPU/RSS样本51份。
- 十个五切窗口RSS中位数为`290193408、294912000、296091648、296976384、298008576、298303488、301694976、301776896、302366720、302579712` bytes，严格递增。
- 首末owned RSS为`283164672→304742400` bytes。
- native合并峰RSS为`664846336` bytes；原A1外层收据峰RSS为`1929838592` bytes，均低于4096MiB。
- 原native finally退出-15，lingering为空。

趋势门失败事实明确；现有证据尚未区分缓存暖机与实际资源保留，ADR033准确保留该不确定性，未宣称已确认泄漏。

原执行顺序在趋势门处失败。native-HLS、controls、expire、图片新链路、errors、autoplay、unknown duration、EOS、layout、正常stop/dispose、Host finish与完整verify均为NOT RUN，未借用旧源成功结果补足。

## 预算与路线可实施性

实际只读核对：

- 严格文件顺序首张未完成卡、current及原`next_card()`均为T0.12；T0.1至T0.11完成判定成立。
- T0.12为sessions3、累计failures6，generation字段缺省按原语义为0，hard_sessions字段不存在；第四许可尚未消费。
- 保存的`fourth-authorization/original-status.json`与上述计数及protected一致。
- T0.13保持paused=true、sessions1、generation3。
- workflow默认max_sessions仍为2。原控制器将`--max-sessions 4`解释为本次驱动使用的累计上限，并在启动前递增sessions；配合`--once`及紧邻首卡/current/next核对，可实现仅T0.12唯一第四会话，实际sessions4即消费许可。第五会话未授权，其他卡仍用默认2。
- A1超时3600秒，完整verify定义合计5040秒，共8640秒；原单会话10800秒保持。实际诊断修复与完整运行仍受原时限约束。
- 原gpt-6.1-sol/workspace-write开发、read-only巡检、4096MiB macOS RSS采样、jobs2、线程1、原绝对共享重锁路径及driver锁保持。

当前第一张待闭环卡为T0.12，本次授权提供原流程内可执行的定位和最小修复路径。启动前仍须紧邻核对clean/source/protected、s3、旧进程PID与创建时间及两锁实际空闲；本审阅未获取锁或验证实时进程存活。

原CLI负责实施与相关真实自验证。已知权限限制按ADR010稳定WIP退出，由外层在源码稳定且旧身份结束后，通过原run执行相关构建、类型、行为及准确A1。成功后直接完整verify、原done、受验完成提交、新固定独立CLEAR及clean齐备，才满足完整完成门。GUI锁定时按原规则等待解锁，严格后卡等待。

本次仅追加单卡额度及原范围内诊断修复授权，protected保持；冻结定义、控制脚本、权限和全局配置未改，无需rebaseline。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要及控制器只读函数、已保存报告数据核对与41份副本SHA256校验。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未运行构建/测试，未启动开发CLI，未获取锁，未输出cap凭据。

第四会话诊断、根因确认、实际修复及回归、新源完整准确A1、完整verify、done、受验提交与新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权及规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
