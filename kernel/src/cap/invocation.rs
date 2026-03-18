use crate::{
    cap::{
        self, Capability, FrameCap, MDBNode, Slot,
        threads::Tcb,
        utils::{CPtr, align_up, lookup_empty_slot, lookup_source_slot},
    },
    config,
};

use sal4_common::invocation;
use tg_console::log;

pub fn handle_cap_invoke(tcb: &mut Tcb) {
    let service = CPtr::from(tcb.ctx.a(0));
    let label = tcb.ctx.a(1);
    let args = [tcb.ctx.a(2), tcb.ctx.a(3), tcb.ctx.a(4), tcb.ctx.a(5)];

    let root_cnode = match tcb.cspace_root {
        Capability::CNode(root) => root,
        _ => {
            log::error!("[invoke] current thread has no root cnode");
            *tcb.ctx.a_mut(0) = invocation::ERR_UNSUPPORTED_CAP;
            return;
        }
    };

    let mut slot = match lookup_source_slot(&root_cnode, service, usize::BITS) {
        Ok(slot) => slot,
        Err(err) => {
            log::error!(
                "[invoke] cptr lookup failed: service={}, label={}, err={err:?}",
                service.raw(),
                label,
            );
            *tcb.ctx.a_mut(0) = invocation::ERR_LOOKUP;
            return;
        }
    };

    match slot.cap_mut() {
        Capability::Untyped(untyped) => {
            handle_untyped_invoke(tcb, &root_cnode, service, untyped, label, args)
        }
        _ => {
            log::error!(
                "[invoke] unsupported cap type for service={}, label={}",
                service.raw(),
                label,
            );
            *tcb.ctx.a_mut(0) = invocation::ERR_UNSUPPORTED_CAP;
        }
    }
}

fn handle_untyped_invoke(
    tcb: &mut Tcb,
    root_cnode: &cap::CNodeCap,
    service: CPtr,
    untyped: &mut cap::UntypedCap,
    label: usize,
    args: [usize; 4],
) {
    match label {
        invocation::UNTYPED_RETYPE => {
            let object_type = args[0];
            let destination = CPtr::from(args[1]);
            let size_bits = args[2];

            let result = untyped_retype(
                root_cnode,
                untyped,
                service,
                object_type,
                destination,
                size_bits,
            );
            *tcb.ctx.a_mut(0) = match result {
                Ok(()) => invocation::OK as usize,
                Err(err) => err.into(),
            };
        }
        _ => {
            log::error!(
                "[invoke] unsupported untyped label: service={}, label={}, args={args:?}",
                service.raw(),
                label,
            );
            *tcb.ctx.a_mut(0) = invocation::ERR_UNSUPPORTED_LABEL;
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum InvokeError {
    UnsupportedObject,
    UntypedSpace,
    Destination,
}

impl From<InvokeError> for usize {
    fn from(value: InvokeError) -> Self {
        match value {
            InvokeError::UnsupportedObject => invocation::ERR_UNSUPPORTED_OBJECT,
            InvokeError::UntypedSpace => invocation::ERR_UNTYPED_SPACE,
            InvokeError::Destination => invocation::ERR_DESTINATION,
        }
    }
}

fn untyped_retype(
    root_cnode: &cap::CNodeCap,
    untyped: &mut cap::UntypedCap,
    service: CPtr,
    object_type: usize,
    destination: CPtr,
    size_bits: usize,
) -> Result<(), InvokeError> {
    if object_type != invocation::OBJECT_FRAME {
        log::error!(
            "[invoke] unsupported object type for untyped retype: service={}, object_type={}",
            service.raw(),
            object_type,
        );
        return Err(InvokeError::UnsupportedObject);
    }

    if size_bits != config::MIN_UNTYPED_SIZE_BITS as usize {
        log::error!(
            "[invoke] unsupported size_bits for frame retype: service={}, size_bits={}",
            service.raw(),
            size_bits,
        );
        return Err(InvokeError::UntypedSpace);
    }

    let destination_slot =
        lookup_empty_slot(root_cnode, destination, usize::BITS).map_err(|err| {
            log::error!(
                "[invoke] destination slot lookup failed or not empty: service={}, dest={}, err={err:?}",
                service.raw(),
                destination.raw(),
            );
            InvokeError::Destination
        })?;

    let base = untyped.paddr;
    let end = base + (1usize << untyped.size_bits);
    let alloc_start = align_up(base + untyped.free_offset, 1usize << size_bits);

    let alloc_end = alloc_start
        .checked_add(1usize << size_bits)
        .ok_or(InvokeError::UntypedSpace)?;

    if alloc_end > end {
        log::error!(
            "[invoke] untyped exhausted: service={}, base={:#x}, free_offset={:#x}, requested_bits={}",
            service.raw(),
            base,
            untyped.free_offset,
            size_bits,
        );
        return Err(InvokeError::UntypedSpace);
    }

    unsafe {
        core::ptr::write_bytes(alloc_start as *mut u8, 0, 1usize << size_bits);
    }

    untyped.free_offset = alloc_end - base;
    destination_slot.write(Slot {
        cap: Capability::Frame(FrameCap {
            paddr: alloc_start,
            size_bits: size_bits as u8,
        }),
        mdb: MDBNode::empty(),
    });

    log::info!(
        "[invoke] untyped retype ok: service={}, dest={}, frame=[{:#x}, {:#x})",
        service.raw(),
        destination.raw(),
        alloc_start,
        alloc_end,
    );

    Ok(())
}
