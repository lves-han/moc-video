# Round 41：T0.13 唯一第七会话授权独立文档复审

结论：ADR-042及对应RESOLVED准确落实真实人类授权。唯一一次6→7、仅原前端范围利用已有诊断定位最小修复、原RSS门不变及必要测试限制均明确。现有资源持有根因未定，文档准确保留该事实。未发现阻塞本次授权增量的文档问题。第七会话修复与新源业务复验均为NOT RUN。

## 受审基线

本轮基于round40，仅审查本次授权增量及相关失败义务。

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`8d75427a4a6ab03a452228e7703037cebb79ded0`
- 原摘要函数实际source：`198ec737482ec9a703d79bbd9c0429196af25d450de478e236a77b68dc580d72`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-042分段SHA256：`f65e8d98ede2c94d2b1f1af780a87d11dfe674ad39a32966d0e23ed563d20c3e`
- 工作树干净。固定提交仅追加DECISIONS 13行、PROGRESS 7行，两文件保留父提交完整字节前缀，前41份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `336443c9122eb82baa58252c89b7f18a9e30c7a1bb7fdb118961fb726bc3f161` |
| docs/PROGRESS.md | `54651c98eada5166a1b2127d81ab3058ea48112ba860acdb7c665f63beb4df40` |
| docs/TASKS.md | `1ab319bff84d3ee4a9f54544f93dbcf4f473785252ce7723f8a01d240f6ed396` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-40.md | `c5855255564a0ed25b1f29d6ad83c45c52a07c7b422a36ee58039e192a6cce24` |

## 真实授权与语义覆盖

沿用既有moc-init、AGENTS及PROMPT-DOC-REVIEW，本轮核对round40、原T13相关义务、ADR042/RESOLVED和当前原始证据。

root可信转述的用户明确选择为“仅批准 T0.13 追加一次（6→7），沿原前端范围用已有诊断定位最小修复，原门不变”。此前询问同时提供追加开发和只读复审测法选项，用户选择追加开发。ADR准确记录其选择，授权限于原前端src、tests和PROGRESS；未延伸为RSS门、测法、阶段或Rust修改授权。

本次工作直接服务原切集生命周期、CPU/内存与资源回收义务。先阅读已有50/51逐身份RSS、生命周期及8份renderer/GPU映射证据，确认相关资源或验收自身持有原因，再最小修复并做必要局部回归，路径可实施。缓存、解码资源或实际持有原因尚待实证，文档未预认根因。

原完整50次切集、51份aggregate CPU/RSS样本及原时点、十个五切窗口原趋势门、全部创建身份和WebView XPC预算保持。排除renderer、替换为detail/footprint/DOM计数、改负载/时点/阈值/适用阶段、抽样、warmup、强制GC及同源筛选均被明确禁止；真实修复须有因果证据及当前源实测。确需改变指标、测法、阶段或范围时须另取真实决定。

新增必要测试约束明确：每次新检查或重跑先说明具体改动、未解失败、验证点及现有证据不足；优先现成最小入口，复用有效结果，避免惯例批量前端/Rust检查和完整UI预检后重复A1。稳定源码运行一次准确A1，成功直接完整verify。该约束保持冻结必需门及完整规模，也约束无关和重复测试。

原14冻结cases、单AppState/Api/SettingsQueue/UiProvider等复用、来源与epoch守卫、首页合并、双语偏好/系统明暗/活动会话保持继续受验。20矩阵、4DPI、五首帧、4.5对比度、当前真实EOS/HistoryEntry完成、从0重播、删除清空确认与焦点、共享12秒及自然退出正负探针保持。

C9一次3秒查询、一次过期恢复、图片恢复、完整errors及重复诊断、autoplay、unknown、真实EOS、布局、正常stop/dispose及Host finish均要求新源准确A1完整执行。History仍为会话预览，SQLite后卡接入，移动业务另授权。Rust/lib/main/host/gateway、依赖、冻结契约及控制/全局配置未获本次修改授权，无需rebaseline。

## 当前失败及诊断证据

当前指针指向：

`evidence/drive/t13-sixth-validation-20261104T035656Z`

指针SHA256：`635e1c45cdc2b1e63e2477ab5f83fc50e23ee7109d43853ad2d579bfb73a8e52`。

`rss-failure-ui-complete/manifest.json`的SHA256为：

`5e9ed0484e9ca8eeb4bb648238743477e34a08f575c677acfd0572c0b10492d2`

167份实际副本均存在且符合记录SHA。缺1份可选周期`samples-1.jsonl`准确列明，原51份实际样本保留。

准确A1 `20261104T052925074681-A1`为finished、exit1、s6/generation3，source前后一致。报告128必需、128实跑、127通过、1失败；JUnit142项，137通过、5失败、0跳过。唯一原始case失败为十窗口RSS严格增长。

只读重算保存数据确认：

- 完整50条切集及51份aggregate样本存在。
- 十窗口中位数为`297041920、300122112、302415872、305856512、309592064、314064896、319832064、323928064、327532544、331481088` bytes，严格递增，原门拒绝。
- owned RSS首末为`289914880→340115456` bytes。
- 同角色原始样本重算变化：renderer增加51937280、application下降4980736、GPU增加2408448、network增加835584 bytes。
- 原执行树峰RSS为2050703360 bytes，低于4096MiB；预算通过与趋势门失败分别保留。
- 末次DOM84、track1、监听/会话/HTTP及scratch等已有计数稳定；旧轨弱引用末alive5、connected/enabled为0，仍不足确认RSS增长原因。
- native finally退出-15，lingering为空。

live、media-stopped、observer-released和view-disposed阶段的8份renderer/GPU vmmap日志实际存在且已归档。日志标注Physical footprint，该指标与aggregate RSS不同；其下降或波动未用于替代原门或确认缓存/泄漏。诊断stop/dispose观察与原正常清理验收分开记录。

主UI11项全部通过，20个矩阵组合及4DPI齐全，五个startup报告各自通过。历史原始证据显示当前身份trusted ended、正式phase=ended、HistoryEntry completed=true，完成观察耗时693ms，低于原12秒；重播位置0、删除/清空和焦点恢复通过。上述成功按本源历史保存，不覆盖RSS失败。

本轮RSS门后的nativeHLS/controls、expire、图片、errors、autoplay、unknown、EOS、layout、正常stop/dispose、Host finish及完整verify仍为NOT RUN。诊断清理未冒充这些未执行门。

## 预算、路线与完成门

实际严格首张未完成卡、current及原`next_card()`均为T0.13；T0.1至T0.12完成判定成立。当前s6/f9/generation3、paused=false、hard_sessions字段不存在；保存的`seventh-authorization/original-status.json`一致。第七许可未消费，第八未授权。

固定独立round41 CLEAR原文保存提交后，紧邻严格顺序、全部旧创建身份退出、两锁实得、clean/source/protected及s6核对，可通过唯一一次原`drive --once --max-sessions 7`落实累计额度。实际s7即消费，其他卡default2，失败、segments、rebaseline和全部历史保留。

原gpt-6.1-sol/workspace-write开发、read-only巡检、4096MiB macOS RSS采样、jobs2、线程1及全部期限保持。已知sysctl/sccache/Git限制按既有分工处理；稳定WIP退出后外层原run代验。GUI锁定时等待真实解锁。

新源准确A1成功后直接完整verify、原done、受验提交、新固定独立CLEAR及clean齐备才完成，后卡等待。未发现本次增量造成的新文档路线阻塞。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要与控制器只读函数、167份副本SHA256校验、原报告/收据及8份映射核对、RSS/矩阵/历史完成数据重算。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未测试、构建、启动CLI或取锁，未输出凭据。实时旧身份、两锁及GUI状态仍须启动前紧邻核对。

第七会话根因定位、最小修复及必要局部回归、新源完整准确A1、完整verify、done、受验提交及新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权和规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
