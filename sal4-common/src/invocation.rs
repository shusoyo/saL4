/// Return value placed in `a0` when an invocation completes successfully.
pub const OK: isize = 0;

/// Invocation failed because the target capability type does not support
/// invocation in the current phase.
pub const ERR_UNSUPPORTED_CAP: usize = usize::MAX;

/// Invocation failed because the capability lookup did not resolve.
pub const ERR_LOOKUP: usize = usize::MAX - 1;

/// Invocation failed because the label is unknown for the selected capability.
pub const ERR_UNSUPPORTED_LABEL: usize = usize::MAX - 2;

/// Invocation failed because the requested object type is not supported.
pub const ERR_UNSUPPORTED_OBJECT: usize = usize::MAX - 3;

/// Invocation failed because the destination slot was invalid or already used.
pub const ERR_DESTINATION: usize = usize::MAX - 4;

/// Invocation failed because the requested size is invalid or the untyped region
/// does not have enough remaining space.
pub const ERR_UNTYPED_SPACE: usize = usize::MAX - 5;

/// Minimal untyped invocation label: request a future retype operation.
pub const UNTYPED_RETYPE: usize = 1;

/// Minimal object type supported by the first `UntypedRetype` implementation.
pub const OBJECT_FRAME: usize = 1;
