# Round 36：T0.12 唯一第八会话授权独立文档复审

结论：ADR-037及对应RESOLVED准确落实真实人类授权。唯一一次7→8、查询完成后重复错误的诊断与动作保留、相关正负例和真实WK回归、完整重新验收及新完成门明确。原完成勾选已纠正，旧证据与新反例如实保留。未发现阻塞本次授权增量的文档问题。第八会话修复与新源业务复验均为NOT RUN。

## 受审基线

本轮基于round35，仅审查本次授权增量及相关证据。

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`52d5625ac6f256a751f58e9afd39e9affa461fd7`
- 原摘要函数实际source：`83f9e7c1d6184e73aba11485402a33fa6ce77cec07108a1c735cfeecd26b0f1c`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-037分段SHA256：`268ce9b4aceff1c9fff59c8d8d7083212ef3dc9618224ecc55f8ffb9a386ef33`
- 工作树干净。固定提交仅追加DECISIONS 10行、PROGRESS 8行，两文件保留父提交完整字节前缀，前36份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `3f0c24ec25bb6e336d43ed969fe1d7bc83ddf7686b775ab5da07063660ec1a47` |
| docs/PROGRESS.md | `51a9c5382ac4dd7ffe804319089712c1769724ec8361680e36643b3b384e2f2c` |
| docs/TASKS.md | `438718e8b3dca5173a141b66b2df4688e26f5a7dc03e6c10d7eee4a21e2f12d8` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-35.md | `ceae511ee0e262950925fd8654f5e1fb2f25e9a3f28f139d6ab66b1decb28894` |

TASKS中的T12实际为`- [ ] 完成`；上表记录文件实际字节摘要，protected沿原函数规范化完成勾选。

## 真实授权与原需求覆盖

沿用前轮已读取的moc-init、AGENTS、PROMPT-DOC-REVIEW、T12完整原卡及原控制器语义，本轮核对round35、ADR037/RESOLVED、SPEC10.5、C9、相关生产路径、独立巡检和原run反例。

root可信转述的真实用户回复为“批准 T0.12 再追加一次（7→8）”。此前询问明确说明独立巡检发现并复现生产缺陷：查询完成后同资源重复错误覆盖已确认分类并隐藏retry，第七额度耗尽；仅追加一次，由原CLI最小保留诊断和动作、补回归及真实页面验证，再完整复验，原一次查询、安全、资源与全部验收门保持。受审文本准确记录该决定。

该修复直接服务T12原R34/REQ-T12及SPEC10.5/C9的业务error.code分类与重试入口。授权收窄至原state.ts及tests中的同加载尝试、资源、epoch/session已完成诊断保留，要求覆盖查询完成前后重复、错误资源、旧身份和新加载等正负例及真实WK相关回归。

每加载尝试最多一次3秒只读status保持；重复错误不得增加查询。资源成功清错、其他资源独立、旧epoch及新load身份、目标资源、Host/source_revision/request校验均继续受验。原401/502/codec类别、生产鉴权及缓存语义保持。该规划能够拒绝覆盖网关分类、隐藏重试、重复查询、跨资源污染或旧身份恢复等错误实现。

原50次真实切集、51份aggregate CPU/RSS样本、十个五切窗口原趋势门、每轮当前红蓝帧与440/880非零PCM、Session/URL绑定、旧音频旧帧拒绝及媒体/HTTP回收保持。图片原生恢复、errors、autoplay、unknown、真实EOS、布局、正常stop/dispose及Host finish均要求新源完整复验。指标、门或测法实质变化仍须另取真实决定。

## 已确认缺口及完成勾选纠正

原独立巡检`20261103T062248404993-T0.12`绑定固定提交`5f651e838d288cc589e6242c99aa18e6c34934fb`，真实ended、exit0、verdict CLEAR；输出同时明确记录C9/SPEC10.5的P2重复错误问题。ADR037如实记录该组合事实，原报告保持原样。

原run `evidence/runs/20261103T063038571417-t12-post-query-repeat-error`为finished、exit1、s7/g0、相同source。原始输出确认：

- 查询完成后：`UPSTREAM_FAILED`、action=`retry`。
- 同资源重复MediaError4后：`MEDIA_UNSUPPORTED`、action为空。
- 断言“重复错误保留已完成网关诊断”失败。

静态因果链吻合：`src/state.ts`的receive先执行`fail(event.error)`；随后diagnose因`queried.has(key)`返回，已确认网关诊断被原MediaError覆盖。原巡检指出已有测试覆盖查询完成前连续错误，完成后重复时序存在缺口。真实WK该时序尚为NOT RUN。

WIP提交`79f9e71`实际撤销T12完成勾选，当前严格首张未完成卡、current及原`next_card()`均为T0.12，后卡保持等待。原done、A1、verify、受验提交、CLEAR及新反例全部保留；新增决定明确要求新源验收、新受验提交和新固定独立CLEAR，未使用accept-review或改写旧报告消除缺口。

## 证据与路线可实施性

当前指针指向：

`evidence/drive/t12-seventh-validation-20261103T061101Z`

指针SHA256：`e1bd2d2b9b7cff215f9c574613b24c6528b8ed46f37ec98413403a97ff587dde`。

两份归档清单均已核对：

- `repeat-diagnosis-gap/manifest.json`：SHA256 `797ff6a81e009c7a19e5b86c33621d49df22a1e7a084a2a3b55d5c990e5434cb`，12份副本全部符合SHA。
- `completion-evidence/manifest.json`：SHA256 `32b8469f7e5fab0f48f5a0aaec2a05876ee424e7a37d353174a70c8c9d131020`，50份副本全部符合SHA。

旧A1为5必需全通过、JUnit12项，50切集与51份样本、全部原生恢复/EOS/布局/清理及Host结果存在，native退出0、lingering为空。旧direct verify收据为finished、exit0、六门齐全；保存日志含26项oracle、workflow 128项中7 skipped、native108及16项通过。可选`samples-1.jsonl`缺失已在清单明确保留，逐命令JSON/results及报告中的51份样本仍归档。上述旧成功与后发现反例分别保存，当前缺口仍待修复。

实际T12为s7/f10/g0，hard_sessions字段不存在，第八许可未消费；保存的`eighth-authorization/original-status.json`一致。T0.1至T0.11完成判定成立，T13保持paused/s1/gen3。

原控制器累计上限及启动递增语义，配合唯一一次`drive --once --max-sessions 8`与紧邻首卡/current/next、身份、锁、clean/source/protected核对，可落实唯一第八会话。实际s8即消费，第九未授权，其他卡default2，历史不重置。

原CLI实施与相关真实自验，限制沿ADR010稳定WIP退出后交外层原run；新准确A1成功后直接完整verify、原done、新受验提交、新固定独立CLEAR和clean齐备才完成。原权限、4096MiB macOS RSS采样、jobs2、线程1、30秒及全部时限保持。冻结定义、控制脚本和全局配置未改，protected保持，无需rebaseline。未发现本次授权增量造成的新文档路线阻塞。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要及控制器只读函数、归档报告/日志/收据和生产因果链核对、12份与50份副本SHA256校验。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未构建测试、启动CLI或取锁，未输出cap凭据。实时旧身份及两锁空闲仍须启动前紧邻核对。

第八会话最小修复、相关正负例、真实WK完成后重复错误回归、新源完整准确A1、完整verify、原done、新受验提交与新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权与规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
