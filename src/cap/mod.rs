pub mod threads;

use crate::config;

#[derive(Debug, Copy, Clone)]
pub struct EndpointCap {
    pub badge: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum Capability {
    Null,
    Endpoint(EndpointCap),
    Tcb,
}

#[derive(Debug, Copy, Clone)]
pub struct CSpace {
    pub slots: [Capability; config::NULL_CSPACE_SLOTS],
}

impl CSpace {
    pub const fn empty() -> Self {
        Self {
            slots: [Capability::Null; config::NULL_CSPACE_SLOTS],
        }
    }

    pub const fn for_null_kernel(badge: usize) -> Self {
        let mut cspace = Self::empty();
        cspace.slots[1] = Capability::Endpoint(EndpointCap { badge });
        cspace.slots[2] = Capability::Tcb;
        cspace
    }

    pub fn lookup(&self, slot_index: usize) -> Option<Capability> {
        if slot_index >= self.slots.len() {
            None
        } else {
            Some(self.slots[slot_index])
        }
    }
}
