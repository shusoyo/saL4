use tg_kernel_context::LocalContext;

pub struct Tcb {
    pub id: usize,
    pub alive: bool,
    pub ctx: LocalContext,
    pub cspace: crate::cap::CSpace,
    pub write_badge: usize,
}

impl Tcb {
    pub fn new(id: usize, entry: usize, sp: usize, badge: usize) -> Self {
        let mut ctx = LocalContext::user(entry);
        *ctx.sp_mut() = sp;

        Self {
            id,
            alive: true,
            ctx,
            cspace: crate::boot::BootManager::bootstrap_cspace_for_task(badge),
            write_badge: badge,
        }
    }
}
