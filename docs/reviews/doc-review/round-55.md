# ADR-061 / T0.37 过渡文档独立审查报告

## 结论

CLEAR：仅确认新增卡、用户决定、交付依赖和两阶段过渡计划足以进入“提交文档并用原 task.py start 建立 T0.37 状态”这一步。当前 C5 原文仍有效；不授权开展 T0.37 实现或验收。拟定的 C5 补充须在取得用户对具体冻结修订与 rebaseline 的明确批准后落地，并对最终受审版本完成独立 CLEAR。

## 受审基线

- 项目根：`/Users/eason/jp/application`；Git HEAD：`d5cd545894050a9bd5359763aee98b428b5831f7`。受审文件有未提交修改，其原始 SHA-256 如下，不能仅以 HEAD 识别。
  - `docs/TASKS.md`：`6a543950c12da56f478ef11f6419cd7dbd60a365b59b2325bdf510e9a812b81f`
  - `docs/DECISIONS.md`：`6f8e830278432d420f5b88e36ea9a7b46bd7b93751ac295d9b422772b350bbce`
  - `docs/PROGRESS.md`：`a169d843329079f28d3e7591d17ea03202c113cf4f63d1bae4ae94169661c6a4`
  - `docs/ARCHITECTURE.md`：`602366bbc8488f24de278532921fa2d4e6579d87ff257cd6249bf04d745d8096`
  - `workflow.json`：`829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7`
- 当前卡 T0.35 已完成；工作树源码摘要与其 done 收据 source 均为 `aad5387ef76a3b5843edd180aa6ff246ffdbf1e824d7c57e5fdb9a94c12e8d4a`。此处的内容快照比较不替代清理工作树及原控制器的全部检查。
- 受审 protected 与全部 ADR 分段摘要见报告末尾的 DOC-REVIEW-BASELINE。

## 对需求和安全边界的核对

- 用户明确要求首集找到成功来源后，后续集优先使用，并确认关闭重开后保留。需求覆盖表将原话/ADR-061 映射到 T0.37/A1；新卡列出真实三线路、前两条失败、第三条真实解码推进、后续两集与重启、手动选择抢占、陈旧结果、来源修订、存储故障和敏感数据不落盘。九个具名 cases 包含需求汇总及各项行为，不能用单一汇总代替。
- 首次人工确认、当前会话实际推进、元数据影片/来源及修订/来源内容绑定和当前数据重取均已写入拟定 C5；无结构化季集字段时要求可审计的唯一标签与元数据交叉核对，重复、缺号、跨季、重排及同名歧义回确认。新增同 revision/同 vod_id 内容替换反例，禁止只凭旧 group_id、数组下标或标题相似推断。
- A1 覆盖生产 MacCMS 适配器、真实 Host/窗口与 vod_play_from/vod_play_url 原格式，包括 season/number 为 null 的正反例；同时有同源 MacCMS 换线、后续集、重启、历史 video 身份及旧会话回归。旧 match、Resolution、Session、Stream 和 URL 不得复用或持久化。
- 存储损坏、权限/配额或写入失败时不阻断当前播放及原匹配，不伪报保存成功，保留确认记录并提供可定位错误；恢复写入和迟到结果在 A1 范围内。
- T0.36 依赖 T0.37，保留原 A1/A2 内容及干净系统声音确认。交付须区分 T0.35 变更前固定构建证据与 T0.37 当前源码回归，不将旧报告冒称新源码全量验证。T0.28–T0.35 卡正文 spec 未变；本轮未扩大 T0.28–T0.34 验收。

## 控制器可行性与待决前提

当前 ARCHITECTURE 已恢复到 T0.35 受验内容，其源码摘要等于 T0.35 done 收据；TASKS、DECISIONS、PROGRESS 和 doc review 属原控制器动态文件。将过渡文件及本报告提交并使工作树干净后，按文件顺序与依赖应选择 T0.37；须实际执行 task.py next 验证。task.py start 建立卡状态，不等同于启动开发 CLI。

随后若用户明确批准具体 C5 修订及原 rebaseline，按现行规则记录 BLOCKED、真实 RESOLVED 与批准摘要，更新受保护基线，完成最终独立文档复审；该 CLEAR 之前不得开始实现或业务验收。具体 rebaseline 授权不由普通功能请求或本报告推定。若公开协议确需扩展，另明确最小范围与复审。

## 已执行检查与未验证

只读对比 Git 差异、C5、ADR-061、任务依赖及需求覆盖；核查 src/state.ts、生产 MacCMS 适配器和 Host/匹配实现、原控制器 completed/snapshot/start/rebaseline 语义；执行 task.py status/check、git diff --check、文件摘要及受验源码快照比较。task.py check 为 43 张卡。未修改项目文件或状态；T0.37 A1、完整 verify、真实窗口/重启行为和最终 C5 修订均 NOT RUN。本报告不重新证明所有历史需求。

DOC-REVIEW-BASELINE: {"adr":{"ADR-001":"31756257a1974108f9c1c8426c9bcb8f80128e2b867e9703f4b766b2e1754407","ADR-002":"d67dcfe2ddb2fd858acd1b1f4cb17440fa47dc15b4948811b1018d062b52e368","ADR-003":"26e4e12c4ae38b3ddcfbea18c04a1602799586a08610125f66567ae8dd516c5c","ADR-004":"a08db017049e19a1481fa314938b5eb88dfcb5598287af24e70bd93058c6eaae","ADR-005":"df0b0d68cd7a4595bd621ce153ba914da82496311c112089fd5b843d8685606d","ADR-006":"335827cabfced06c4cc2e2fc99978940e01818abbd7af6aedd12a66161b883c1","ADR-007":"028d7df4ecd18afa281cb45703aaf4e26857f114163be10b0c2d8860d718b227","ADR-008":"9c1b6bc1afa7ae46d9c714f2da6ff25775bb4f84fa2a894666823fabbc9f00cc","ADR-009":"2af85af3e9a0d33cd2fe5088a27bc3075ed73986e2e6da5b904129753f6e62c6","ADR-010":"2a8794076a5170988b290a41e762b733d1fca797f403b3ec2a497e32cbf4f45b","ADR-011":"48be2fa1e537f273c097ea8a1740ed0d93e8ea0dc3fa22abb8ea87dee831297b","ADR-012":"6b5c36ac582bfaea4cf97d42f35a2ea515abc903e9b835631469fca4200ed5c0","ADR-013":"d2c0c16b73cb276623fa0609ef43d8004ffbe78f4fe876c9211ccf7050294abf","ADR-014":"ef2a5363e382e5ddd1f085a1f7ae8671e9e420ed2eca8f565d6a5ef58ec78017","ADR-015":"22efa652b4d7491e68730f18d581764a679f8ed76baed2b673cd588ee93e7d48","ADR-016":"7bafb564f37f4c8f97dc2dd5984b1d566f621a0b96456e4c93fbf55f504862e8","ADR-017":"af981e1d604c68cb3971f88d8f8a404ee7e96c54a252946739f617b1a1776641","ADR-018":"3afac14679cd0259aa796032d3dc17c98a9915bd1eaadb2f80f2c19cd2ab22c4","ADR-019":"76f109d971d15c3c1b3f5377468a2e5a746c38f0caf3357442fd5d10745125bc","ADR-020":"4deb9a3d81c7033670e543644299bd456e0ec2eabfeeb25d96cb3e05b87fc9b3","ADR-021":"162b25bbf3de89b4a4c2fb1a6ed5ba2791fad963300807d7781f74e5e13025a9","ADR-022":"f5d9c3f48b001f688a18044d80dccbb75c98cf6a71b8627395e9acc2169b5c57","ADR-023":"f18853d93cb1d48819e9e84d9c3185f94d80bac46b42e99f9f6244692b277478","ADR-024":"038231a74a95bd84aafcedbe60ef46676b9f1fc63329bea5aff62a515c5a49cd","ADR-025":"4daf92c6b76cb0f449eed044f1ab2a6878df0ffc70a8653967a169780f1eb572","ADR-026":"a215bc09b327d0b295185c916805e5a2448f2b6b208591c66fab2c8bdad6052c","ADR-027":"428a4bc585fe0107c643716f76e6857e19534886ff647556d02477fe7495833d","ADR-028":"46c40a9b9cfbc2aed67690189d5f787e4a860d8c1c73a1649c07a5eecd41ee27","ADR-029":"8a8b072d7f01fb2418d7b3616d5f1697cdc50ebf6437d2bafc24ed272dbc2b5b","ADR-030":"cf95dc1bc8f9b096d1c4549bd0c8c5b6cbc58978973bd005d1e7b93d5b8d5549","ADR-031":"44ffce2c67b234c75cd80a8dce2846d57a8c71a70fcbb0c3e965f35922beff98","ADR-032":"c7417df2199f93cde98bd89d2bd3c61ec1ebb8473c35f3662cce2e5ecda1580e","ADR-033":"b7ac3a6796442e301c136ccb6421ca79c86ec43e261bc676504011cfe37fdde0","ADR-034":"37b10ad4e7301e28c2f69ad98212780deb59cdee2f991d205edb215fa7265d95","ADR-035":"6d5674a809870c75775f8b1f52d91cf098e1de30b3582c622f2547269a852946","ADR-036":"87bb8edf873c645c7e7228f365e78d6f0ea9ad1be7ca7a4679cbda73fb50087c","ADR-037":"268ce9b4aceff1c9fff59c8d8d7083212ef3dc9618224ecc55f8ffb9a386ef33","ADR-038":"e4a2efbfd5e847714bd91e065e18ed4346840f47228afa49f54d64b6801e9528","ADR-039":"6b8183fefb77f5018d28a45a1371973dc79e250b089598bc2e01007222e3546d","ADR-040":"2ab090ca8a1dc75d76515a25327906211bb4aede86574904728215312e14b171","ADR-041":"1b351c8fe55bdb85ae2dfa9971f49d97127bd1e61fdfadf28bc194c0028d373d","ADR-042":"f65e8d98ede2c94d2b1f1af780a87d11dfe674ad39a32966d0e23ed563d20c3e","ADR-043":"85de7df8d21029d6ef84b17b29db997fe318e95dba2187207c15cc3340b706f9","ADR-044":"f6e19d72435fb278213b117f08beae149c52695865a1c4bd45c36ebcbee44a44","ADR-045":"bb77b617437541b76a491b34ac823795879fba9a4a2912596f6919aaba7fbfa5","ADR-046":"42058f815bb3a0cda323014c430ac257157d9b8edd35c0cbf90956781aa2b446","ADR-047":"cab240e7dc9c1ab657fb02aaaf820d45cf69577a145679548624a1473512c012","ADR-048":"bdedb085c22082216d15421f21e2ea10e3afe61cce97080e9c9f180408b858c0","ADR-049":"af60be8e05e3968e605ccad0552d56ec96540b66e1529fc1337b94b43d54e601","ADR-050":"4f747a4e41435d2317e9ccbbfbcd0e4d12ff98a35c01b73c40d9c3b43829623a","ADR-051":"d3223c550c13f153b1a3ccd366898f693f12bcebf786349bc1352248cfeb5f06","ADR-052":"34dc063470d005834931ef0b845898560bdc78aa156e4ade27c7ee38579da852","ADR-053":"e462aa8d9adcc26125e5617a7ba7c0e9ceadb789fe4f3f403d5e79d260cc21e3","ADR-054":"68d96ac9338baccd6cfd30f9c6fcc5508ba3ab784431ce92546d992d5338bd3a","ADR-055":"4a141790900f750657098ef109ac0937efa8fb3263d5e5495dedf82ccd087533","ADR-056":"0a17f399c0ba0593cbe22684b337ad7af78ca5bcae4aaa90f02d00663c47f5db","ADR-057":"5b8dae50baade73ea4b9f6217ef21d8f42636df4f4a4512ca75fc478d1898d1a","ADR-058":"0b5d21df283072c98822e489c7e530a1250ccc25f7eb396999be40d1d3fba5c3","ADR-059":"9b1a90a73b659e0f4f65ce5b90f74c57ed63b9c0743fa5f4f84c8c555b374914","ADR-060":"d17c70909fcf4a8a389dbb7ee5277076f5482ed3a691895ebede8213f7c700a5","ADR-061":"6fa6079b1eba35234cd2aa93534150d6b484a00c460ce30a995101fee7a93278"},"protected":"c61e8148dd962b45e3564e81ef6d0d091927126b61e7f1d2523cd8f571c4e22b"}

DOC-REVIEW-VERDICT: CLEAR
