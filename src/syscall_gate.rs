#![allow(dead_code)]

use crate::cap::{Capability, threads::Tcb};
use tg_syscall::{Caller, SyscallId};

use riscv::register::scause::{self, Exception, Trap};

pub enum TrapAction {
    Continue,
    Exit,
    Killed,
}

fn cap_fault(task: &mut Tcb, slot_index: usize) {
    println!("[Kernel] Capability Fault at Slot {}", slot_index);
    task.alive = false;
    task.ctx.move_next();
}

fn dispatch_user_call(task: &mut Tcb) -> TrapAction {
    let syscall_raw = task.ctx.a(7);

    let id: SyscallId = syscall_raw.into();

    let required_slot = match id {
        SyscallId::WRITE => Some(1),
        SyscallId::EXIT => Some(2),
        _ => None,
    };

    if let Some(slot_index) = required_slot {
        let cap = match task.cspace.lookup(slot_index) {
            Some(cap) => cap,
            None => {
                cap_fault(task, slot_index);
                return TrapAction::Killed;
            }
        };

        match id {
            SyscallId::WRITE => match cap {
                Capability::Endpoint(ep) if ep.badge == task.write_badge => {}
                _ => {
                    cap_fault(task, slot_index);
                    return TrapAction::Killed;
                }
            },
            SyscallId::EXIT => match cap {
                Capability::Tcb => {}
                _ => {
                    cap_fault(task, slot_index);
                    return TrapAction::Killed;
                }
            },
            _ => unreachable!(),
        }
    }

    let args = [
        task.ctx.a(0),
        task.ctx.a(1),
        task.ctx.a(2),
        task.ctx.a(3),
        task.ctx.a(4),
        task.ctx.a(5),
    ];
    let caller = Caller {
        entity: task.id,
        flow: task.write_badge,
    };

    match tg_syscall::handle(caller, id, args) {
        tg_syscall::SyscallResult::Done(ret) => {
            *task.ctx.a_mut(0) = ret as usize;
            task.ctx.move_next();
            if id == SyscallId::EXIT {
                println!("[Kernel] Task {} requested exit", task.id);
                task.alive = false;
                return TrapAction::Exit;
            }
        }
        tg_syscall::SyscallResult::Unsupported(unsupported) => {
            println!(
                "[Kernel] Unknown syscall {} from task {}",
                unsupported.0, task.id
            );
            task.alive = false;
            task.ctx.move_next();
            return TrapAction::Killed;
        }
    }

    TrapAction::Continue
}

pub fn handle_trap(task: &mut Tcb) -> TrapAction {
    match scause::read().cause() {
        Trap::Exception(Exception::UserEnvCall) => dispatch_user_call(task),
        trap => {
            println!("[Kernel] task {} killed by trap {:?}", task.id, trap);
            task.alive = false;
            TrapAction::Killed
        }
    }
}
