use camino::{Utf8Path, Utf8PathBuf};
use std::{
    env::{
        self,
        consts::{DLL_EXTENSION, DLL_PREFIX},
        current_dir,
    },
    fs,
    process::Command,
};

fn main() {
    let places_root = find_places_root();
    let out_dir = &places_root.join("kotlin-out-dir");
    println!("------------------------ building -----------------------------");
    copy_dylib(&places_root, out_dir);
    run_uniffi_bindgen(out_dir);
    run_kotlin(&places_root, out_dir);
}

fn run_kotlin(places_root: &Utf8Path, outdir: &Utf8Path) {
    Command::new("kotlinc")
        .current_dir(outdir)
        .arg("-d")
        .arg("places_async.jar")
        .arg("-classpath")
        .arg(env::var("CLASSPATH").unwrap())
        .arg("uniffi/places/places.kt")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    fs::copy(
        places_root.join("bindings").join("places_example.kts"),
        outdir.join("places_example.kts"),
    )
    .unwrap();
    println!("------------------------ running -----------------------------");
    Command::new("kotlinc")
        .current_dir(outdir)
        .arg("-classpath")
        .arg(format!(
            "{}:places_async.jar",
            env::var("CLASSPATH").unwrap()
        ))
        // Enable runtime assertions, for easy testing etc.
        .arg("-J-ea")
        // Our test scripts should not produce any warnings.
        .arg("-script")
        .arg("places_example.kts")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn run_uniffi_bindgen(out_dir: &Utf8Path) {
    uniffi_bindgen::library_mode::generate_bindings(
        &out_dir.join(format!("{DLL_PREFIX}places_async.{DLL_EXTENSION}")),
        None,
        &[uniffi_bindgen::bindings::TargetLanguage::Kotlin],
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
