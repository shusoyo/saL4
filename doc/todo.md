# TODO

## 当前主线

### 1. 建立 capability invocation 的最小入口

- 不直接跳进 `UntypedRetype`，先建立一个最小 invocation ABI。
- 在 [sal4-common/src/sal4_syscall.rs](/Users/suspen/retest/os/salos/sal4-common/src/sal4_syscall.rs) 中新增：
  - `CAP_INVOKE = 16`
- 第一版调用约定：
  - `a7 = CAP_INVOKE`
  - `a0 = service cptr`
  - `a1 = invocation label`
  - `a2..a5 = invocation arguments`
- 目标：
  - 让 rootserver 后续所有“调用某个 capability”的行为都走这一条入口。

### 2. 在 kernel 中拆分 debug syscall 和 capability invocation

- 修改 [kernel/src/syscall/mod.rs](/Users/suspen/retest/os/salos/kernel/src/syscall/mod.rs)。
- 把 syscall 处理分成两类：
  - `handle_debug_syscall(...)`
  - `handle_cap_invoke(...)`
- 保留当前两个 debug syscall：
  - `DEBUG_PUT_CHAR`
  - `DEBUG_SHUTDOWN`
- 让 `CAP_INVOKE` 单独进入 capability 调用分发逻辑。

### 3. 让 CPtr lookup 返回真实 slot

- 当前 [kernel/src/cap/utils.rs](/Users/suspen/retest/os/salos/kernel/src/cap/utils.rs) 中的 `resolve_cptr()` 返回的是 `Slot` 的值拷贝。
- 新增一个接口，例如：
  - `resolve_cptr_slot(...) -> Result<NonNull<Slot>, LookupError>`
- 目标：
  - capability invocation 操作真实 slot，而不是只读副本。
- 这一步是后续 `retype` 的必要前置条件。

### 4. 建立 invocation skeleton

- 新增一个 capability invocation 模块，例如：
  - [kernel/src/cap/invocation.rs](/Users/suspen/retest/os/salos/kernel/src/cap/invocation.rs)
- 第一版只做框架：
  1. 从 `root_tcb.ctx` 读取 `a0/a1/a2...`
  2. 从 `root_tcb.cspace_root` 取得 root cnode
  3. 用 `resolve_cptr_slot()` 找到 service slot
  4. 根据 `slot.cap` 分发
- 第一版只需要支持：
  - `Capability::Untyped(_)`
  - 其他类型统一返回 `UnsupportedInvocation`

### 5. 为 Untyped 定义最小 invocation label

- 新增共享的 invocation label 定义，例如：
  - [sal4-common/src/invocation.rs](/Users/suspen/retest/os/salos/sal4-common/src/invocation.rs)
- 第一版先定义：
  - `UNTYPED_RETYPE = 1`
- 最小参数约定建议：
  - `a0 = untyped service cptr`
  - `a1 = UNTYPED_RETYPE`
  - `a2 = object type`
  - `a3 = destination slot`
  - `a4 = size_bits`
- 当前阶段不追求完整 seL4 风格 message，只够用即可。

### 6. 让 Untyped 成为第一个真实 capability operation

- `UntypedRetype` 作为第一个真实 invocation。
- 第一版范围压到最小：
  - 一次只创建一个对象
  - 只写一个 destination slot
  - destination slot 必须为空
- 推荐第一种支持的对象：
  - `FrameCap`
- 原因：
  - [kernel/src/cap/mod.rs](/Users/suspen/retest/os/salos/kernel/src/cap/mod.rs) 中已经有 `FrameCap`
  - 不需要马上引入更复杂的内核对象元数据。

### 7. 给 UntypedCap 增加最小可消费状态

- 当前 [kernel/src/cap/mod.rs](/Users/suspen/retest/os/salos/kernel/src/cap/mod.rs) 中的 `UntypedCap` 只有：
  - `paddr`
  - `size_bits`
  - `is_device`
- 为了支持最小 `retype`，需要补一个最小状态字段，例如：
  - `free_offset`
  - 或 `watermark`
- 第一版只支持简单顺序分配，不做完整回收和派生树。

### 8. 让 rootserver 发起第一次 capability invocation

- 在 [rootserver/src/main.rs](/Users/suspen/retest/os/salos/rootserver/src/main.rs) 中加入一个最小 helper，例如：

```rust
fn cap_invoke(service: usize, label: usize, arg0: usize, arg1: usize, arg2: usize) -> usize
```

- rootserver 第一版只做一件事：
  - 从 `bootinfo.untyped.start` 取第一个 untyped slot
  - 从 `bootinfo.empty.start` 取第一个空 slot
  - 发起一次 `UntypedRetype`
  - 根据返回值打印成功或失败

## 建议的提交顺序

1. `~. cap/syscall: add invocation syscall shell`
2. `~. cap: return slot references from cptr lookup`
3. `~. untyped: add minimal retype path`
4. `~. rootserver: request first untyped retype`

## 阶段完成标志

- rootserver 启动
- 读取 `BootInfo`
- 取第一个 untyped slot
- 取第一个 empty slot
- 发一次 `CAP_INVOKE(UNTYPED_RETYPE, ...)`
- kernel 成功在目标 slot 写入一个 `FrameCap`
- rootserver 打印 `retype ok`
