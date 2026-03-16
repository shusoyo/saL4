use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const TARGET_ARCH: &str = "riscv64gc-unknown-none-elf";
const ROOTSERVER_BASE_ADDRESS: u64 = 0x8040_0000;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LOG");
    write_linker();
    build_rootserver();
}

fn write_linker() {
    let ld = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(&ld, tg_linker::NOBIOS_SCRIPT)
        .unwrap_or_else(|err| panic!("failed to write linker script to {}: {}", ld.display(), err));
    println!("cargo:rustc-link-arg=-T{}", ld.display());
}

fn build_rootserver() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .expect("kernel crate should live under the workspace root");
    let rootserver_root = workspace_root.join("rootserver");
    let rootserver_manifest = rootserver_root.join("Cargo.toml");
    let rootserver_target_dir = workspace_root.join("target").join("rootserver-build");
    println!("cargo:rerun-if-changed={}", rootserver_manifest.display());
    println!(
        "cargo:rerun-if-changed={}",
        rootserver_root.join("build.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rootserver_root.join("src").display()
    );

    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            rootserver_manifest.to_string_lossy().as_ref(),
            "--target",
            TARGET_ARCH,
        ])
        .env("BASE_ADDRESS", ROOTSERVER_BASE_ADDRESS.to_string())
        .env("CARGO_TARGET_DIR", &rootserver_target_dir)
        .status()
        .expect("failed to execute cargo build for rootserver");
    if !status.success() {
        panic!("failed to build rootserver");
    }

    let elf = rootserver_target_dir
        .join(TARGET_ARCH)
        .join("debug")
        .join("rootserver");
    let bin = objcopy_to_bin(&elf);

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let app_asm = out_dir.join("app.asm");
    write_single_app_asm(&app_asm, ROOTSERVER_BASE_ADDRESS, &bin);
    println!("cargo:rustc-env=APP_ASM={}", app_asm.display());
}

fn objcopy_to_bin(elf: &Path) -> PathBuf {
    let bin = elf.with_extension("bin");
    let status = Command::new("rust-objcopy")
        .args([
            elf.to_string_lossy().as_ref(),
            "--strip-all",
            "-O",
            "binary",
            bin.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to execute rust-objcopy");
    if !status.success() {
        panic!("rust-objcopy failed for {}", elf.display());
    }
    bin
}

fn write_single_app_asm(path: &PathBuf, base: u64, bin: &PathBuf) {
    use std::io::Write;

    let mut asm = fs::File::create(path)
        .unwrap_or_else(|err| panic!("failed to create {}: {}", path.display(), err));

    writeln!(
        asm,
        "\
.global apps
.section .data
.align 3
apps:
    .quad {base:#x}
    .quad 0
    .quad 1
    .quad app_0_start
    .quad app_0_end
app_0_start:
    .incbin {bin:?}
app_0_end:"
    )
    .unwrap();
}
