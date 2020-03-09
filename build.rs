use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

lazy_static! {
    static ref MANIFEST_DIR: PathBuf = {
        PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR")
                .expect("The CARGO_MANIFEST_DIR environment variable is not set"),
        )
    };
    static ref OUT_DIR: PathBuf = {
        PathBuf::from(
            std::env::var_os("OUT_DIR")
                .expect("The CARGO_MANIFEST_DIR environment variable is not set"),
        )
    };
}

enum SrcMethod {
    Cmake(PathBuf),
    RawStatic(PathBuf),
    PkgConfig,
}

fn get_source_method() -> SrcMethod {
    let src = std::env::var_os("APRILTAG_SRC");
    let method: Option<String> = std::env::var_os("APRILTAG_SYS_METHOD").map(|s| {
        s.into_string()
            .expect("If set, APRILTAG_SYS_METHOD environment variable must be UTF-8 string.")
    });

    let method: String = match method {
        None => "pkg-config".to_string(), // This is the default.
        Some(s) => s,
    };

    match method.as_str() {
        "pkg-config" => SrcMethod::PkgConfig,
        "raw,static" => SrcMethod::RawStatic(
            src.expect("APRILTAG_SRC environment variable must be set")
                .into(),
        ),
        "cmake,dynamic" => SrcMethod::Cmake(
            src.expect("APRILTAG_SRC environment variable must be set")
                .into(),
        ),
        _ => {
            panic!(
                "The APRILTAG_SYS_METHOD was not recognized. See README.md of the \
                 apriltag-sys crate for a description of this environment variable."
            );
        }
    }
}

#[derive(Debug)]
struct Error {}

impl From<glob::PatternError> for Error {
    fn from(_: glob::PatternError) -> Error {
        Error {}
    }
}

impl From<glob::GlobError> for Error {
    fn from(_: glob::GlobError) -> Error {
        Error {}
    }
}

fn main() -> Result<(), Error> {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=APRILTAG_SRC");
    println!("cargo:rerun-if-env-changed=APRILTAG_SYS_METHOD");

    #[allow(unused_variables)]
    let clang_args = match get_source_method() {
        SrcMethod::Cmake(src_path) => build_cmake(src_path),
        SrcMethod::RawStatic(sdk_path) => build_raw_static(sdk_path)?,
        SrcMethod::PkgConfig => {
            // check if apriltag is available on system
            let _ = pkg_config::probe_library("apriltag").unwrap();
            vec![]
        }
    };

    #[cfg(feature = "buildtime-bindgen")]
    {
        let bindgen_builder = bindgen::Builder::default()
            .header("wrapper.h")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks))
            .whitelist_type("apriltag_.*")
            .whitelist_type("image_u8_.*")
            .whitelist_type("image_u8x3_.*")
            .whitelist_type("image_u8x4_.*")
            .whitelist_type("zarray_.*")
            .whitelist_type("matd_.*")
            .whitelist_function("apriltag_.*")
            .whitelist_function("tag16h5_.*")
            .whitelist_function("tag25h9_.*")
            .whitelist_function("tag36h11_.*")
            .whitelist_function("tagCircle21h7_.*")
            .whitelist_function("tagCircle49h12_.*")
            .whitelist_function("tagCustom48h12_.*")
            .whitelist_function("tagStandard41h12_.*")
            .whitelist_function("tagStandard52h13_.*")
            .whitelist_function("image_u8_.*")
            .whitelist_function("image_u8x3_.*")
            .whitelist_function("image_u8x4_.*")
            .whitelist_function("zarray_.*")
            .whitelist_function("matd_.*");
        let bindgen_builder = bindgen_builder.clang_args(clang_args);
        let bindings = bindgen_builder
            .generate()
            .expect("Unable to generate bindings");

        let bindings_path = MANIFEST_DIR.join("bindings").join("bindings.rs");
        std::fs::create_dir_all(bindings_path.parent().unwrap()).unwrap();
        bindings
            .write_to_file(bindings_path)
            .expect("Couldn't write bindings!");
    }

    Ok(())
}

/// Use cmake to build April Tags as a shared library.
fn build_cmake<P>(src_path: P) -> Vec<String>
where
    P: AsRef<Path>,
{
    println!("cargo:rustc-link-lib=apriltag");

    let build_dir = OUT_DIR.join("cmake_build");
    std::fs::create_dir_all(&build_dir).unwrap();

    let dst = cmake::Config::new(src_path).out_dir(build_dir).build();

    let include_dir = dst.join("include");
    let lib_dir = dst.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    vec![
        format!("-I{}", include_dir.display()),
        format!("-L{}", lib_dir.display()),
    ]
}

/// Build April Tags source by passing all .c files and compiling static lib.
fn build_raw_static(sdk_path: PathBuf) -> Result<Vec<String>, Error> {
    let inc_dir = sdk_path.clone();

    let mut compiler = cc::Build::new();

    // add files in base SDK dir
    let mut c_files = sdk_path.clone();
    c_files.push("*.c");
    let glob_pattern = c_files.to_str().unwrap();
    let paths = glob::glob_with(glob_pattern, glob::MatchOptions::new())?;
    for path in paths {
        let path = path?;
        let path_str = path.display().to_string();
        if path_str.ends_with("apriltag_pywrap.c") {
            continue;
        }
        compiler.file(&path);
    }

    // add files in base/common SDK dir
    let mut c_files = sdk_path.clone();
    c_files.push("common");
    c_files.push("*.c");
    let glob_pattern = c_files.to_str().unwrap();
    let paths = glob::glob_with(glob_pattern, glob::MatchOptions::new())?;
    for path in paths {
        compiler.file(&path?);
    }

    compiler.include(&inc_dir);
    compiler.extra_warnings(false);
    compiler.compile("apriltags");

    Ok(vec![])
}
