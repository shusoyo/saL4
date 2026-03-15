use tg_kernel_context::LocalContext;

use crate::cap::Capability;

#[repr(C)]
pub struct Tcb {
    pub ctx: LocalContext,
    pub cspace_root: Capability,
    pub vspace_root: Capability,
    pub state: ThreadState,
}

impl Tcb {
    pub const fn empty() -> Self {
        Self {
            ctx: LocalContext::empty(),
            cspace_root: Capability::Null,
            vspace_root: Capability::Null,
            state: ThreadState::Inactive,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ThreadState {
    Inactive,
    Running,
    BlockedOnReceive,
    BlockedOnSend,
}
