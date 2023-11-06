use camino::{Utf8Path, Utf8PathBuf};
use std::{
    env::{
        consts::{DLL_EXTENSION, DLL_PREFIX},
        current_dir,
    },
    fs,
    process::Command,
};

fn main() {
    let places_root = find_places_root();
    let out_dir = &places_root.join("swift-out-dir");
    println!("------------------------ building -----------------------------");
    copy_dylib(&places_root, out_dir);
    run_uniffi_bindgen(out_dir);
    run_swift(&places_root, out_dir);
}

fn run_swift(places_root: &Utf8Path, outdir: &Utf8Path) {
    Command::new("swiftc")
        .current_dir(outdir)
        .arg("-emit-module")
        .arg("-module-name")
        .arg("places_async_mod")
        .arg("-emit-library")
        .arg("-Xcc")
        .arg(format!("-fmodule-map-file={outdir}/placesFFI.modulemap"))
        .arg("places.swift")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    fs::copy(
        places_root.join("bindings").join("places_example.swift"),
        outdir.join("places_example.swift"),
    )
    .unwrap();
    println!("------------------------ running -----------------------------");
    Command::new("swift")
        .current_dir(outdir)
        .arg("-Xcc")
        .arg(format!("-fmodule-map-file={outdir}/placesFFI.modulemap"))
        .arg("-I")
        .arg(".")
        .arg("-L")
        .arg(".")
        .arg("-l")
        .arg("places_async")
        .arg("-l")
        .arg("places_async_mod")
        .arg(format!("{outdir}/places_example.swift"))
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn run_uniffi_bindgen(out_dir: &Utf8Path) {
    uniffi_bindgen::library_mode::generate_bindings(
        &out_dir.join(format!("{DLL_PREFIX}places_async.{DLL_EXTENSION}")),
        None,
        &[uniffi_bindgen::bindings::TargetLanguage::Swift],
        out_dir,
        false,
    )
    .unwrap();
}

fn copy_dylib(places_root: &Utf8Path, outdir: &Utf8Path) {
    if !outdir.exists() {
        fs::create_dir_all(outdir).unwrap()
    }
    Command::new("cargo")
        .arg("build")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    let appservices_root = places_root.parent().unwrap().parent().unwrap();
    let source = appservices_root
        .join("target")
        .join("debug")
        .join(format!("{DLL_PREFIX}places_async.{DLL_EXTENSION}"));
    let target = outdir.join(&source.file_name().unwrap());
    fs::copy(source, target).unwrap();
}

fn find_places_root() -> Utf8PathBuf {
    let mut dir = &*current_dir().unwrap();
    while let Some(name) = dir.file_name() {
        if name == "places-async" {
            return dir.to_owned().try_into().unwrap();
        }
        dir = dir.parent().unwrap();
    }
    panic!("Must be run inside the `places-async` directory")
}
