use std::{
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

const TARGET_ARCH: &str = "riscv64gc-unknown-none-elf";
const ROOTSERVER_BASE_ADDRESS: u64 = 0x8040_0000;

struct BuildContext {
    workspace_root: PathBuf,
    out_dir: PathBuf,
    rootserver_target_dir: PathBuf,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LOG");
    let ctx = build_context();
    track_embedded_rootserver_inputs(&ctx);
    write_linker();
    let (rootserver_bin, image_stamp) = build_rootserver(&ctx);
    write_embedded_rootserver(&ctx, &rootserver_bin, image_stamp);
}

fn build_context() -> BuildContext {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .expect("kernel crate should live under the workspace root")
        .to_path_buf();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    BuildContext {
        rootserver_target_dir: workspace_root.join("target").join("rootserver-build"),
        workspace_root,
        out_dir,
    }
}

fn write_linker() {
    let ld = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(&ld, tg_linker::NOBIOS_SCRIPT)
        .unwrap_or_else(|err| panic!("failed to write linker script to {}: {}", ld.display(), err));
    println!("cargo:rustc-link-arg=-T{}", ld.display());
}

fn track_embedded_rootserver_inputs(ctx: &BuildContext) {
    track_tree(&ctx.workspace_root.join("rootserver"));
    track_tree(&ctx.workspace_root.join("sal4-common"));
}

fn track_tree(path: &Path) {
    track_path(path);
    let entries = fs::read_dir(path)
        .unwrap_or_else(|err| panic!("failed to read directory {}: {}", path.display(), err));
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|err| panic!("failed to walk {}: {}", path.display(), err));
        let entry_path = entry.path();
        if entry_path.is_dir() {
            track_tree(&entry_path);
        } else {
            track_path(&entry_path);
        }
    }
}

fn track_path(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

fn build_rootserver(ctx: &BuildContext) -> (PathBuf, u64) {
    let rootserver_root = ctx.workspace_root.join("rootserver");
    let rootserver_manifest = rootserver_root.join("Cargo.toml");

    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            rootserver_manifest.to_string_lossy().as_ref(),
            "--target",
            TARGET_ARCH,
        ])
        .env("BASE_ADDRESS", ROOTSERVER_BASE_ADDRESS.to_string())
        .env("CARGO_TARGET_DIR", &ctx.rootserver_target_dir)
        .status()
        .expect("failed to execute cargo build for rootserver");
    if !status.success() {
        panic!("failed to build rootserver");
    }

    let elf = ctx
        .rootserver_target_dir
        .join(TARGET_ARCH)
        .join("debug")
        .join("rootserver");
    let bin = objcopy_to_bin(&elf);
    let stamp = hash_file(&bin);
    (bin, stamp)
}

fn write_embedded_rootserver(ctx: &BuildContext, bin: &Path, image_stamp: u64) {
    let app_asm = ctx.out_dir.join("app.asm");
    write_single_app_asm(&app_asm, ROOTSERVER_BASE_ADDRESS, bin, image_stamp);
    println!("cargo:rustc-env=APP_ASM={}", app_asm.display());
    println!("cargo:rustc-env=ROOTSERVER_IMAGE_STAMP={image_stamp:016x}");
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

fn hash_file(path: &Path) -> u64 {
    let bytes = fs::read(path)
        .unwrap_or_else(|err| panic!("failed to read {} for hashing: {}", path.display(), err));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn write_single_app_asm(path: &Path, base: u64, bin: &Path, image_stamp: u64) {
    use std::io::Write;

    let mut asm = fs::File::create(path)
        .unwrap_or_else(|err| panic!("failed to create {}: {}", path.display(), err));

    writeln!(
        asm,
        "\
# rootserver image stamp: {image_stamp:016x}
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
