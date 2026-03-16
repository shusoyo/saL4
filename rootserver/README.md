# rootserver

This crate holds the first minimal root server task used to bring up user-mode
execution in the kernel.

The initial version is intentionally tiny:

- it does not depend on the tutorial POSIX-style user library
- it only enters `_start`
- it executes a single `ecall`
- it spins forever if control unexpectedly returns

This keeps the first kernel/user bring-up path focused on:

1. loading the root task image
2. entering U-mode
3. handling the first trap back into the kernel
