use crate::cap::CSpace;

pub struct BootManager;

impl BootManager {
    // Stage-1 null-kernel bootstrap: inject a minimal fixed CSpace.
    pub const fn bootstrap_cspace_for_task(task_badge: usize) -> CSpace {
        CSpace::for_null_kernel(task_badge)
    }
}
