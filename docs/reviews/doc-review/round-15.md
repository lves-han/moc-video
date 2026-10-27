# 独立文档复审 round-15

本轮审查用户将T0.2交由当前agent持续完成、取消本卡任务额度限制的授权及原工作流接续路线。审阅者全程只读，未修改项目、执行重命令或启动子代理。

## 受审基线

- 项目：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`5c05d17b5db68b1ad863666cfc19442ed415526f`。
- source：`f50dd34310109ea50feb51298b0441e0ab219e8015e8cd017cfdb7cae716cbd5`。
- protected：`5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481`。
- earliest/current/next均为T0.2。
- 原sessions=6、失败9条、generation=1及区段保留；hard_sessions为空。
- workflow默认max_sessions=2保持；工作树干净，task check、scope及check_frozen通过。
- ADR-019分段SHA-256：`76f109d971d15c3c1b3f5377468a2e5a746c38f0caf3357442fd5d10745125bc`。

关键文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| docs/DECISIONS.md | `e5fcea409de7bf67b6940badc9f8a2365f6173a43a52e37c8ef7daa8c1e1512a` |
| docs/PROGRESS.md | `6d4333ebec67cd984bccef0340000fee809633deb87904840eec94397ea8ab18` |
| docs/reviews/doc-review/round-14.md | `158e915a56866f722256d827fc3d2da9b5bd8845b6ef7a5e4f828f3901814633` |
| tests/acceptance/t01.py | `499b7e675966bbb6d0d18cb81aacc97b76066eae04f7431e5445cbe67ded1294` |
| tests/acceptance/test_t01.py | `accfb7f8112fbb0b1c191f1009e8f37d5e12ac3670fc8cb43d03e423caf27703` |
| tests/fixtures/t01/automation.ts | `26dc2e9a10f3264227e90e40eb02509e6e457477018b6fc66d97097d937d5891` |
| src-tauri/src/automation.rs | `4beff4d37f9d9fe9346964b9c9226ace7491be5439df37e7758d85dd01c68045` |

## 授权与最小可执行路线

真实人类原话：

> 你来负责T0.2任务的完成，不做额度限制

ADR-019及真实RESOLVED准确将该指令限定于T0.2：当前agent接手实现、诊断、修复、实际验证和收尾，本卡后续开发会话次数及累计开发工时不设上限；其他卡保持原分工和默认预算。

当前agent直接在原T0.2范围内实施，是落实该新分工的最小路线。原run、verify和done核验当前卡、冻结/源码摘要、generation、验收及完成证据，无需为直接修改源码另启动开发CLI。原sessions6继续记录已发生的CLI历史，保持原值即可；当前路线不依赖修改默认max_sessions或新增控制状态。

未来重用开发CLI时，仍须核对原控制器实际支持和当前授权。当前直接实施路线已足以继续本卡，不需要逐次为T0.2累计额度重复询问。

## 保持的限制与完成门

本次授权取消的是T0.2任务额度，以下保持：

- 4096MiB macOS RSS采样、jobs2、测试线程1和项目专属重任务锁。
- 原CLI/独立巡检权限、访问与冻结范围。
- 单条构建、验收、verify及巡检命令超时。
- 准确A1的真实MP4/HLS各1800秒、20生命周期，以及帧/UI/音频、安全、布局/DPI和资源清理断言。
- 全部冻结cases、完整verify、原done、受验提交、固定提交独立read-only CLEAR及干净工作树。
- ADR-015严格顺序，T0.2完整完成后才处理后卡。

当前agent即使同时承担实现和复验，最终独立只读巡检仍保持。原`--review-only`路线不增加开发会话，可在完成提交后沿原流程取得独立结论。

T0.4已批准范围及多暂停卡冻结迁移约束继续保留；本次未扩展其他卡或控制器变更授权。

## 当前实现与验证事实

第6开发会话WIP`e726bfa`保留了require更名、测试专用前台激活及新增回归。此前真实诊断已显示视频帧和媒体时钟前进、RAF停计，并同时记录hidden/focused=false；新实现的正确性仍由回归和真实长播证明。

本轮末已读取并通过原只读verify_evidence核对：

- `evidence/runs/20261027T100502286761-t01-build/receipt.json`：finished、exit0。
- `evidence/runs/20261027T100639820423-t01-negatives/receipt.json`：finished、exit0。

两者source、protected及generation=1一致，支撑证据有效。审阅者未重新运行测试。准确A1、两格式完整长播、20生命周期及完整verify仍须取得当前源码的有效证据。

当前同ID监管提示词已更新为ADR-019直接接手模式，保持原资源/单条超时及其他卡默认值，移除逐次追加本卡额度的启动要求；发生实际变化后继续整体维护当前事项，历史保留于原证据。

## 实际检查与结论

本轮实际核对了原task check、scope、check_frozen、顺序/current/next、原计数与generation、配置资源字段、ADR-019、RESOLVED、关键摘要、监管提示词及最新build/回归支撑证据。

受保护定义和原控制代码保持，无需rebaseline。当前agent可在本卡范围持续执行“实际构建/测试→依据失败修复→重新复验”，以全部原质量门成立为结束条件。

附更正：round-14摘要表的旧handoff SHA出现转录误字。当轮实际工具输出为`6925b2c59aa22161e1c38042177a16ba2a7c3bd564cc70a48940a6d4691a82b4`；原报告保持，后续引用采用本更正。该动态旧文件现已更新，此更正不影响当前protected或授权结论。

未发现本次T0.2直接接手及不限任务额度授权与原资源、权限、范围和完成质量门之间的阻断问题。

DOC-REVIEW-VERDICT: CLEAR
