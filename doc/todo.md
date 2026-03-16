# TODO

## Goal

Make the kernel boot far enough to create, map, start, and return from a first root task in user mode.

## Near-Term Goals

These items are the current implementation path and should stay at the top of the queue.

### 1. Finish Early Boot Accounting

- [ ] Split physical memory into `kernel reserved`, `boot reserved`, `user image`, and `free/untyped` regions.
- [ ] Define and document the ownership rules for every boot-time allocation so later retype/map code does not overlap bootstrap memory.
- [ ] Add assertions that `BootInfo`, root cnode storage, root tcb, IPC buffer, and future page tables do not overlap.
- [ ] Add a debug dump path so boot can print the final `BootInfo` content for validation.

### 2. Complete BootInfo Contract

- [ ] Finalize the `BootInfo` layout that the root task will consume.
- [ ] Record the physical and virtual location of the IPC buffer in `BootInfo`.
- [ ] Record root task image frame slots in `BootInfo` instead of leaving `user_image_frames` empty.
- [ ] Record any extra boot payloads that the root task needs, such as FDT or initrd, if applicable.

### 3. Build Initial Address Space

- [ ] Introduce VSpace/page-table capability types instead of leaving `vspace_root` as `Null`.
- [ ] Allocate the root task top-level page table during bootstrap.
- [ ] Implement page-table object creation for the target RISC-V paging mode.
- [ ] Map the kernel-required trampoline/trap entry pieces needed for user/kernel transitions.
- [ ] Map the root task IPC buffer into user virtual memory at a defined address.
- [ ] Choose and document the user virtual address layout: image base, stack, IPC buffer, boot info, guard pages.
- [ ] Add helpers for mapping frames into a VSpace with permission bits and alignment checks.

### 4. Load Root Task Image

- [ ] Decide the root task binary source and format path, most likely ELF.
- [ ] Parse the root task image and extract loadable segments.
- [ ] Allocate/map frames for each loadable segment into the root task VSpace.
- [ ] Copy segment contents into mapped frames and zero any `.bss` tail.
- [ ] Populate `user_image_frames` capability slots for every frame backing the root task image.
- [ ] Allocate and map a user stack for the root task.
- [ ] Define the initial user entry PC and SP from the loaded image.

### 5. Materialize Runnable Thread State

- [ ] Extend `Tcb` so it can hold the data needed for scheduling, fault handling, and IPC bookkeeping.
- [ ] Initialize the root task `Tcb` with both `cspace_root` and `vspace_root`.
- [ ] Build the initial user register context in `LocalContext`: `pc`, `sp`, argument registers, status bits.
- [ ] Pass `BootInfo` and IPC buffer addresses to the root task using the chosen ABI.
- [ ] Move the root task state from `Inactive` to a runnable state only after its address space is valid.

### 6. Enter User Mode

- [ ] Add a scheduler-ready structure, even if there is only one runnable thread initially.
- [ ] Implement the first context-switch/restore path from kernel bootstrap into user mode.
- [ ] Set up `stvec`, trap frame save/restore, and the return path from traps back to the current thread.
- [ ] Verify that the root task can execute its first instruction and trap back cleanly.

### 7. Minimal Trap and Syscall Path

- [ ] Decode trap causes and distinguish interrupts, exceptions, and syscalls.
- [ ] Implement a minimal syscall surface sufficient for the first user task to print, yield, or exit.
- [ ] Wire capability lookup into syscall dispatch using the existing `resolve_cptr` path.
- [ ] Add fault reporting for bad capability lookup, page faults, and illegal instructions.
- [ ] Define what happens when the root task exits or faults fatally.

## Long-Term Goals

These items matter, but they can stay behind the main boot-to-user-mode path.

### 8. Capability Operations Beyond Static Boot

- [ ] Introduce capability types for page tables, endpoints, notifications, and any other objects required by the ABI.
- [ ] Implement untyped retype so boot-created untyped memory can produce kernel objects dynamically.
- [ ] Add slot mutation primitives: insert, move, mint/copy, revoke/delete.
- [ ] Replace placeholder MDB data with real derivation metadata or explicitly document the temporary model.
- [ ] Validate guard/radix semantics in `resolve_cptr` against the intended cspace design.

### 9. Validation

- [ ] Add unit tests for cspace lookup edge cases: guard mismatch, invalid path, empty slot, too deep.
- [ ] Add boot assertions for object alignment, slot bounds, and untyped accounting.
- [ ] Add an integration path that boots under QEMU and confirms the root task reaches user mode.
- [ ] Add a smoke test where the root task performs one syscall and the kernel handles it correctly.
- [ ] Add logging checkpoints around image load, page-table setup, first switch, and first trap.

### 10. Platform Generalization

- [ ] Replace the fixed early-boot memory window with a real physical memory description source.
- [ ] Generalize boot assumptions that are currently hard-coded for the QEMU tutorial platform.

## Suggested Execution Order

1. Finish `BootInfo` and physical memory accounting.
2. Implement `VSpace` objects and page-table mapping helpers.
3. Load and map the root task image plus user stack.
4. Initialize the root task `Tcb` and user register context.
5. Enter user mode and get a first trap/syscall back.
6. Fill in capability mutation and retype after the static boot path works.
