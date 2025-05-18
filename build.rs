use std::{env::var, fs, path::PathBuf};

pub fn main() {
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    // 获取 crate 根路径
    let crate_dir = var("CARGO_MANIFEST_DIR").unwrap();
    let include_dir = PathBuf::from(&crate_dir).join("include");
    if !include_dir.exists() {
        fs::create_dir_all(&include_dir).expect("Failed to create include directory");
    }

    let header_path = include_dir.join("sdaa_data.h");

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(cbindgen::Config::from_file("cbindgen.toml").unwrap())
        .generate()
        .unwrap()
        .write_to_file(header_path);

    #[cfg(feature = "cuda")]
    {
        println!("cargo:rustc-link-search=../cuddc");
        println!("cargo:rustc-link-search=../cuwf");
        println!("cargo:rustc-link-lib=cuddc");
        println!("cargo:rustc-link-lib=cuwf");
        //println!("cargo:rustc-link-search=/usr/local/cuda/lib64");
        println!("cargo:rustc-link-lib=cudart");
        //println!("cargo:rustc-link-lib=cuda");
        //println!("cargo:rustc-link-lib=stdc++");

        let header_ddc = PathBuf::from("../cuddc/ddc.h");
        let header_cuwf = PathBuf::from("../cuwf/cuwf.h");
        println!(
            "cargo:rerun-if-changed={}",
            header_ddc.to_str().expect("invalid path")
        );

        println!(
            "cargo:rerun-if-changed={}",
            header_cuwf.to_str().expect("invalid path")
        );
        let bindings = bindgen::Builder::default()
            // The input header we would like to generate
            // bindings for.
            //.header(header_cuwf.to_str().expect("invalid path"))
            .header(header_ddc.to_str().expect("invalid path"))
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
            .write_to_file(out_path.join("ddc_bindings.rs"))
            .expect("Couldn't write bindings!");


        let bindings = bindgen::Builder::default()
            // The input header we would like to generate
            // bindings for.
            //.header(header_cuwf.to_str().expect("invalid path"))
            .header(header_cuwf.to_str().expect("invalid path"))
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
            .write_to_file(out_path.join("cuwf_bindings.rs"))
            .expect("Couldn't write bindings!");
    }
}
