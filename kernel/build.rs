use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let current_dir = env::current_dir().unwrap();

    let font_obj = Path::new(&out_dir).join("shinonome_halfwidth.o");

    Command::new("objcopy")
        .current_dir(Path::new(&current_dir).join("resources"))
        .args(&["-I", "binary"])
        .args(&["-O", "elf64-x86-64"])
        .args(&["-B", "i386:x86-64"])
        .arg("shinonome_halfwidth.bin")
        .arg(&font_obj)
        .status()
        .unwrap();

    println!(
        "cargo:rustc-link-arg={}",
        font_obj.as_path().to_str().unwrap()
    )
}
