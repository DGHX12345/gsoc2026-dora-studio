# M18 D3 fixture parity — dora validate vs Studio compat_engine (2026-08-20)

| Case | dora validate (1.0.0-rc.4) | Studio schema/check | Aligned |
|---|---|---|---|
| case1 UInt8→UInt32 拓宽 | All type annotations OK (无警告) | compatible / green | ✅ |
| case2 Image→Float64 不相关 | type mismatch warning | incompatible / red + 建议 | ✅ |
| case3 UInt8→String + type_rule | All type annotations OK (无警告) | rule / yellow, rule 回显 | ✅ |

Notes:
- 与 compat_engine 单测（11 个）一致；深度 3 边界（3 跳接受/4 跳拒绝）已
  在 Task 3.1 复审中用临时 parity 测试对真实 CLI 验证（见 commit 6d62a9b 评审）。
- 已知偏差（计划 Self-Review 记录）：normalize_field_type 对「不同 URN 但
  结构相同」的 struct 判定为不兼容（dora 深比较相等）——保守方向。
- fixture 文件保留在 out/m18-parity/（本地，不提交）。
