# 本阶段小结：从 Boot Skeleton 到 Rootserver Bring-up

这一阶段的目标，不是实现完整的微内核，也不是进入完整的虚拟地址空间管理，而是先把内核从“只有最早期启动骨架”的状态，推进到“能够构建、装载并启动一个最小 rootserver”的状态。

这一阶段完成后，系统已经具备如下能力：

- 内核可以完成最早期启动，进入 Rust 世界并执行 `bootstrap()`。
- boot 阶段可以分配最基本的内核对象，包括 `BootInfo`、root cnode、root tcb、IPC buffer 和 rootserver 栈。
- `rootserver` 被作为一个独立的用户态 crate 构建，并在 boot 时以固定地址装载。
- root tcb 的初始上下文可以被正确设置，并通过 `LocalContext::execute()` 首次进入 U-mode。
- rootserver 执行 `ecall` 后，内核可以接住 trap，恢复 `stvec`，并回到最小的 trap 处理逻辑。
- kernel 与 rootserver 之间已经建立了最小的启动 ABI：`a0 = BootInfo*`，`a1 = ipc_buffer`。

## 一、本阶段的主要目标

这一阶段聚焦在三个问题上：

1. 内核在 boot 阶段到底要准备哪些最小对象。
2. 初始用户任务 rootserver 应该如何被构建、装载并启动。
3. BootInfo 应该如何作为 boot-time contract 传给 rootserver。

在这个阶段里，我们有意不做以下事情：

- 不实现完整的 VSpace 与页表管理。
- 不实现完整的 syscall 子系统。
- 不实现成熟的调度器。
- 不把普通用户程序纳入内核直接启动的模型。

换句话说，这一阶段只解决：

`kernel boot -> rootserver image -> root tcb -> enter U-mode -> trap back`

这条最短主路径。

## 二、工程结构的调整

为了让后续阶段更清晰，项目被整理成了 workspace 结构：

- `kernel/`
  当前内核 crate，本体代码位于 `kernel/src/`。
- `rootserver/`
  初始用户任务 crate，由内核在 boot 阶段直接启动。
- `sal4-common/`
  kernel 与 rootserver 共享的 ABI 定义。

这样做的原因很简单：

- `kernel`、`rootserver`、共享 ABI 已经是三个不同职责的组件。
- 继续把内核放在仓库根目录 `src/` 下，会让职责边界越来越模糊。
- 改成 workspace 后，目录结构、依赖关系和后续扩展方向都更接近真正的系统工程组织。

## 三、Boot 阶段做了什么

当前 boot 流程位于 `kernel/src/boot/mod.rs`，它只负责串起主流程，不再把所有细节塞进一个文件里。

### 1. 物理内存探测与早期分配

物理内存相关逻辑被拆到了 `kernel/src/boot/mm.rs`：

- `probe_free_memory(rootserver)`
- `BootArena`
- `UntypedStream`

这里采用的是一个刻意简化的模型：

- rootserver 已经通过 `AppMeta` 在 boot 时拷贝到固定运行地址；
- free memory 从 `max(kernel_end, rootserver_end)` 开始；
- untyped 只从这之后的连续区间导出。

这样做的好处是：

- 不需要在当前阶段就引入复杂的“中间挖洞”逻辑；
- rootserver 运行区天然不会出现在 untyped 中；
- 启动模型足够简单，便于先把 rootserver 主路径跑通。

同时，当前实现还加了一个断言：

- rootserver 的固定运行区不能超出 early-boot memory window；
- 一旦这个简化假设被破坏，系统会在 boot 时明确失败，而不是静默进入错误状态。

### 2. Boot-time 对象分配

`bootstrap()` 现在会按顺序分配：

- `BootInfo`
- root cnode
- root tcb
- IPC buffer page
- rootserver stack page

这些对象都是通过 `BootArena` 从 boot-time free memory 中顺序切出来的。  
在当前阶段，这种顺序分配器已经足够，因为我们只需要可靠地把最小启动对象先搭起来。

### 3. 初始 capability 安装

当前 root cnode 中会安装一组固定的 boot-time capability：

- `NULL`
- `TCB`
- `CNode`
- `IRQControl`
- `BootInfo frame`
- `IPC buffer frame`

这些固定 slot 现在已经形成了一个明确的 boot contract。  
后续 rootserver 读取 `BootInfo` 时，可以稳定地依赖这套初始化布局。

## 四、Rootserver 的引入与启动

### 1. rootserver 不再是普通测试 app

这一阶段做了一个重要取舍：

- 不再把 tutorial user apps 当作“内核直接启动的一组程序”；
- 而是单独引入一个 `rootserver/` crate，作为 boot 时唯一直接处理的用户态镜像。

这更接近 seL4 的思路：

- kernel 只负责启动第一个特殊的初始任务；
- 普通用户程序将来由 rootserver 在用户态世界里处理。

### 2. rootserver 的构建方式

`kernel/build.rs` 会在构建内核时：

1. 构建 `rootserver`。
2. 使用 `rust-objcopy` 把它转成平坦二进制。
3. 通过 `AppMeta` 风格的元数据把它嵌进 kernel image。
4. 在 boot 时由 `tg_linker::AppMeta::locate().iter().next()` 触发 copy 到固定地址。

虽然这里仍然复用了 `AppMeta` 的装载逻辑，但在语义上，rootserver 已经不再被视为“普通 app 集合中的一个成员”，而是 boot 阶段唯一的初始用户镜像。

### 3. rootserver 启动上下文

rootserver 的上下文准备逻辑已经单独拆到了 `kernel/src/boot/rootserver.rs`：

- `locate_rootserver()`
- `prepare_rootserver_context()`
- `enter_rootserver()`

当前阶段里，root tcb 上下文至少会设置：

- `pc = rootserver.start`
- `sp = rootserver_stack_top`
- `a0 = BootInfo*`
- `a1 = ipc_buffer`

这样 rootserver 入口已经不是一个孤立的“只会 ecall 的函数”，而是开始具备真正的 boot handoff 语义。

### 4. 首次进入用户态与 trap 返回

目前通过 `LocalContext::execute()` 直接完成：

- 从 S-mode 进入 U-mode；
- rootserver 执行第一条 `ecall`；
- trap 回到内核；
- 恢复 `stvec`；
- 做最小的 trap 分类。

当前 trap 处理只覆盖最小必要路径：

- `UserEnvCall`
- 其他异常
- 中断

这足够证明“进入用户态并 trap 返回”这条闭环已经打通。

## 五、BootInfo 的整理与 ABI 化

这一阶段的另一个关键工作，是把 `BootInfo` 从 kernel 私有结构，提升成 kernel 与 rootserver 共享的 boot ABI。

### 1. 共享 ABI crate

共享定义被抽到了 `sal4-common/` 中，目前至少包括：

- `CPtr`
- `SlotRegion`
- `BootInfo`
- `MAX_UNTYPED_OBJECTS`

这样做的意义在于：

- kernel 写入的 `BootInfo`
- rootserver 读取的 `BootInfo`

来自同一份定义，不再需要手抄两份结构体，避免 ABI 漂移。

这正是类似 seL4 的做法：

- 内核通过固定 ABI 传入 `BootInfo*`
- 用户态通过共享定义解释它

### 2. 当前 BootInfo 包含什么

这一阶段的 `BootInfo` 只保留当前真正必要的字段：

- `ipc_buffer`
- `init_thread_cnode_size_bits`
- `empty`
- `user_image_frames`
- `untyped`
- `untyped_paddr_list`
- `untyped_size_bits_list`

这个选择是刻意收敛过的。  
它反映的是“初始资源地图”，而不是未来完整系统的所有状态。

### 3. user_image_frames 的意义

`user_image_frames` 现在已经不再是空壳：

- rootserver image 的页范围会被逐页写成 `FrameCap`
- 对应的 slot 区间会记录在 `BootInfo.user_image_frames`

这意味着 rootserver 已经可以从 `BootInfo` 的视角看到：

- 哪些页构成了自己的初始镜像

### 4. empty / untyped 的边界

这一阶段还明确了 `BootInfo` 中各区间的语义：

- `empty`：
  boot 后尚未使用的 root cnode slot 区间
- `untyped`：
  导出的 untyped capability 区间
- `user_image_frames`：
  仅表示 boot-time rootserver image pages

并且 `untyped` 明确不包括：

- rootserver image
- BootInfo frame
- IPC buffer frame
- rootserver stack

这让当前 boot contract 已经具备了稳定的解释空间。

## 六、目前的系统处于什么状态

到了这一阶段结束时，系统已经可以被判断为：

- 已经越过了“null kernel / hello world kernel”阶段；
- 已经建立了 seL4 风格的 boot-time capability 与初始资源交接框架；
- 已经能够把 rootserver 当成 boot 阶段唯一的初始任务拉起来；
- 已经具备最小的 `BootInfo` handoff ABI；
- 但仍然没有进入完整的地址空间、syscall、调度器和用户态服务体系。

换句话说，当前系统更准确的状态是：

**一个能够启动并交接给最小 rootserver 的早期 capability 微内核原型。**

## 七、本阶段的技术取舍

为了尽快形成最短闭环，这一阶段做了若干有意识的简化：

### 1. 只面向 QEMU 和当前教程平台

- 不解析 DTB/FDT
- 不做通用平台内存发现
- 不做复杂保留区管理

### 2. 不引入 VSpace

- rootserver 通过固定地址装载
- 暂时不处理真正的虚拟地址空间与页表对象

### 3. 只启动一个 rootserver

- kernel 不负责批量启动普通 app
- 普通 app 以后交给 rootserver 在用户态管理

### 4. 先做最小 trap 闭环

- 只验证 `U-mode -> ecall -> kernel`
- 还不进入完整 syscall 框架

这些取舍的目的都很一致：

先把“第一个用户态任务能跑起来”这件事做成，再进入更复杂的系统层次。

## 八、下一阶段最自然的方向

这一阶段收尾之后，下一阶段最自然的工作有三类：

1. 让 rootserver 真正消费 `BootInfo`
   例如读取 `ipc_buffer`、`user_image_frames`、`untyped`。

2. 定义最小 syscall / invocation 语义
   不需要一次做完整，但需要让 `ecall` 有明确的内核处理路径。

3. 继续细化 boot contract
   例如在保留当前简化模型的前提下，逐步靠近更完整的 capability handoff。

当前不适合立刻优先做的，是：

- 完整 VSpace
- 完整调度器
- 普通 user apps 的批量支持
- 通用平台支持

## 九、总结

这一阶段最重要的成果，不是“代码量增加了多少”，而是系统状态发生了实质变化：

- 从单纯的 boot skeleton
- 变成了能够构建、装载、进入并从 trap 返回一个初始用户任务的 capability 微内核雏形

同时，这个雏形已经具备了两个非常关键的长期基础：

- 清晰的 workspace 与模块职责划分
- 共享的 boot ABI（`BootInfo`）

这意味着后续不管你是继续往 seL4 风格的 rootserver 路线走，还是进一步细化 capability / syscall / VSpace，这一阶段的工作都不会是一次性的临时代码，而是后续演进的稳定起点。
