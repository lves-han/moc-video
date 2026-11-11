# 独立文档复审 round-13

本轮审查仅T0.2追加一次长播诊断修复会话的授权、计数、原参数可执行性及后续质量门。审阅者全程只读，未修改项目、运行drive/run/verify或重命令，未启动子代理。

## 受审基线

- 项目：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`a4bb7b01b361aaacf40d57db6fad468e0e458f90`。
- source：`d7142e7e25a78adb6f4baeb604fd73e78d7bcd6b508aec0562d05e9e25b84277`。
- protected：`5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481`。
- 最早未完成卡/current/next均为T0.2。
- sessions=4、失败7条、generation=1，hard_sessions为空；workflow默认max_sessions=2保持。
- 工作树干净；原task check、scope均退出0，check_frozen通过。
- 同一heartbeat任务`automation`为PAUSED，等待本轮复审。

关键文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| docs/DECISIONS.md | `64611e0e29f790338f06bb4443d5abe1bb81f25a50c285adbf5ae2198a48cb26` |
| docs/PROGRESS.md | `800c117a8fcda4dd2f25e90a89532e1cc3003dd6eb66b58bc73ce6e2eed43dfb` |
| docs/reviews/doc-review/round-12.md | `eae220f44e956a84e4658807a06399195a21394ba7e74ec317e2965006e98313` |
| tests/acceptance/t01.py | `f22a05694b154bece1c6467211e5808c5dd16da1bc800b051b6b886987378f0f` |
| tests/acceptance/test_t01.py | `14aab5622c9f987bfc4b21d61e61ac6692b16a4a3c5c0a22c8df72a07620bf18` |
| evidence/task-state/T0.2/handoff.md | `b4955f5854511784ea8d2c01a6948ab02d57c9f697199472b025adc9c894dd34` |

新增ADR-017分段摘要：

`af981e1d604c68cb3971f88d8f8a404ee7e96c54a252946739f617b1a1776641`

AGENTS、SPEC、ARCHITECTURE、TASKS和原控制器定义保持，protected与round-12相同；本次通过追加ADR、真实RESOLVED及handoff记录预算例外。

## 授权与原参数结论

ADR-017保留了完整询问语境：仅T0.2追加一次长播诊断修复会话，累计上限4→5，原脚本实施、外层复验，权限和资源保持。人类针对该问题明确回答：

> 批准 T0.2 追加一次

该回答具有明确卡号、一次数量及原询问中的预算语境。RESOLVED和handoff与之相符，授权未扩大到其他卡。

原drive的局部maximum覆盖及`--once`语义保持。当前sessions=4，在紧邻启动确认earliest/current/next=T0.2、旧写入者及重命令退出后，唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 5
```

可沿原计数启动第5会话。该调用只启动一个开发会话，默认配置仍为2；原4会话、7失败、区段及generation=1保持。protected未变，本次无需rebaseline。

## 失败证据与诊断范围

已核对以下原收据及日志/报告摘要：

- `20261027T080719704187-t01-build`：exit0。
- `20261027T080809906822-t01-negatives`：exit0，27个回归通过记录保留。
- `20261027T080839687237-A1`：exit1，有效JUnit为13通过、7失败、4跳过；source/after_source、protected、generation均与当前一致。

旧窗口/inset及JUnit修复的成功证据保留。新A1的直接失败为`Playback or UI stalled`；原23条样本存在，最后正常样本包含frames=3527、raf=7103、paused=false、media_error=null及audio_state=running。失败瞬间具体触发字段缺失，当前证据尚未确定根因。

新增会话先补失败瞬间可观察信息，再定位真实WebView/播放/UI状态并作最小修复及回归，符合当前实际问题。原正常前序样本保持诊断作用，完整长播通过仍需真实执行。

各收据记录的执行器及payload身份均已退出；原清理事实、forced_cleanup、fixture释放结果及失败历史保留。

## 质量门、顺序与运行约束

修复后仍须在稳定源码上重新取得：

1. 原build及回归成功。
2. 准确A1：真实MP4/HLS各1800秒、20窗口生命周期，以及原帧/UI/音频、安全、布局和清理断言。
3. 直接完整verify。
4. 原done、受验完成提交及固定提交独立CLEAR。

ADR-010开发/验收分工、ADR-015严格顺序、ADR-013项目专属锁继续。当前仅恢复T0.2，后卡等待完整质量门。

gpt-6.1-sol、workspace-write/read-only、4096MiB RSS采样、jobs2、线程1、原单次时长和命令超时保持。T0.4的host.rs范围批准及多在途冻结基线迁移约束继续按round-12保留，本次未扩写控制器变更授权。

已读取当前同ID监管提示词。提示词保留现行规则、真实未消费的第五会话许可、严格顺序、当前阻塞及后续T0.4路线；明确每卡完整完成后整体重写，移除已完成操作、已消费额度指令和重复历史，历史继续保存于项目原证据。此操作维护与原任务状态分离。复审CLEAR后恢复同ID时，应同步更新当前PAUSED状态说明。

## 实际检查与未验证项

本轮实际完成原task check、scope、check_frozen、最早未完成卡/current/next、预算字段、原收据及日志/报告摘要、23样本、退出身份和当前监管提示词核对。

本轮未执行诊断修复、构建、回归、准确A1或完整verify。当前A1仍为FAIL，完整MP4/HLS长播及20生命周期尚未完成；根因定位与最终修复效果待原开发会话和外层真实复验。

未发现此次仅T0.2预算例外、原状态接续及质量门之间的阻断问题。

DOC-REVIEW-VERDICT: CLEAR
