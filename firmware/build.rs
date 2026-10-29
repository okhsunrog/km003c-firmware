use std::{env, fs, path::PathBuf};

fn main() {
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer),
    )
    .unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for name in ["memory.x", "metadata.x"] {
        fs::copy(name, out.join(name)).unwrap();
        println!("cargo:rerun-if-changed={name}");
    }
    println!("cargo:rustc-link-search={}", out.display());
}
