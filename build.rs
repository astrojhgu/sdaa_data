use std::{
    env::var,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn main() {
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // 获取 crate 根路径
    let crate_dir = var("CARGO_MANIFEST_DIR").unwrap();
    let include_dir = Path::new(&crate_dir).join("include");
    if !include_dir.exists() {
        fs::create_dir_all(&include_dir).expect("Failed to create include directory");
    }

    let header_path = include_dir.join("sdaa_data.h");

    // 执行 cbindgen 命令
    if let Ok(status) = Command::new("cbindgen")
        .arg("--config")
        .arg("cbindgen.toml") // 可选：可省略
        .arg("--crate")
        .arg("sdaa_data") // ⚠️ 替换为你的 crate 名
        .arg("--output")
        .arg(header_path)
        .current_dir(&crate_dir)
        .status()
    {
        if !status.success() {
            eprintln!("Warning: cbindgen failed");
        }
    } else {
        eprintln!("Warning: cbindgen failed");
    }

    #[cfg(feature = "cuda")]
    {
        println!("cargo:rustc-link-search=../cuddc");
        println!("cargo:rustc-link-lib=cuddc");
        //println!("cargo:rustc-link-search=/usr/local/cuda/lib64");
        println!("cargo:rustc-link-lib=cudart");
        //println!("cargo:rustc-link-lib=stdc++");

        let header = PathBuf::from("../cuddc/ddc.h");
        println!(
            "cargo:rerun-if-changed={}",
            header.to_str().expect("invalid path")
        );
        let bindings = bindgen::Builder::default()
            // The input header we would like to generate
            // bindings for.
            .header(header.to_str().expect("invalid path"))
            // Tell cargo to invalidate the built crate whenever any of the
            // included header files changed.
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            // Finish the builder and generate the bindings.
            .generate()
            // Unwrap the Result and panic on failure.
            .expect("Unable to generate bindings");

        // Write the bindings to the $OUT_DIR/bindings.rs file.
        let out_path = PathBuf::from(var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("bindings.rs"))
            .expect("Couldn't write bindings!");
    }
}
