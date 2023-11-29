use std::{env, fs, path::Path, process::Command};

use cmake::Config;
use fs_extra::dir::CopyOptions;

fn have_feature(flag: &str) -> bool {
    env::var(format!(
        "CARGO_FEATURE_{}",
        flag.replace('-', "_").to_uppercase()
    ))
    .is_ok()
}

fn read_cmake_opt(flag: &str) -> &'static str {
    if have_feature(flag) {
        "ON"
    } else {
        "OFF"
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").expect("Missing OUT_DIR");

    // Make sure sub-repositories are correctly init
    update_sub_repositories();

    // Tell Cargo that if the given file changes, to rerun this build script.
    println!("cargo:rerun-if-changed=./CMakeLists.txt");
    println!("cargo:rerun-if-changed=./libquickfix");
    println!("cargo:rerun-if-changed=./quickfix-bind");

    // Clone libquickfix to OUT_DIR because it modify itself when building
    let libquickfix_build_dir = Path::new(&out_dir).join("libquickfix");

    let _ = fs::remove_dir_all(&libquickfix_build_dir);
    fs_extra::copy_items(&["./libquickfix"], &out_dir, &CopyOptions::default())
        .expect("Fail to copy libquickfix");

    // Build quickfix as a static library
    let quickfix_dst = Config::new(libquickfix_build_dir)
        .define("HAVE_SSL", "OFF")
        .define("HAVE_MYSQL", read_cmake_opt("build-with-mysql"))
        .define("HAVE_POSTGRESQL", read_cmake_opt("build-with-postgres"))
        .define("HAVE_PYTHON", "OFF")
        .define("HAVE_PYTHON3", "OFF")
        .define("QUICKFIX_SHARED_LIBS", "OFF")
        .define("QUICKFIX_EXAMPLES", "OFF")
        .define("QUICKFIX_TESTS", "OFF")
        .build();

    let quickfix_include_path = format!("{}/include", quickfix_dst.display());
    let quickfix_lib_path = format!("{}/lib", quickfix_dst.display());

    // Build quickfix C bind also as a static library.
    env::set_var("CMAKE_LIBRARY_PATH", [quickfix_lib_path].join(";"));

    let quickfix_bind_dst = Config::new(".")
        .cflag(format!("-I{quickfix_include_path}"))
        .cxxflag(format!("-I{quickfix_include_path}"))
        .define("QUICKFIX_BIND_EXAMPLES", "OFF")
        .define("HAVE_MYSQL", read_cmake_opt("build-with-mysql"))
        .define("HAVE_POSTGRESQL", read_cmake_opt("build-with-postgres"))
        .define("WITH_PRINT_EX_STDOUT", read_cmake_opt("print-ex"))
        .build();

    // Configure rustc.
    println!(
        "cargo:rustc-link-search=native={}/lib",
        quickfix_dst.display()
    );
    println!(
        "cargo:rustc-link-search=native={}/lib",
        quickfix_bind_dst.display()
    );

    if matches!(env::var("PROFILE").as_deref(), Ok("debug"))
        && matches!(env::var("CARGO_CFG_TARGET_OS").as_deref(), Ok("windows"))
    {
        // libquickfix as a different name on windows with debug profile.
        println!("cargo:rustc-link-lib=static=quickfixd");
    } else {
        println!("cargo:rustc-link-lib=static=quickfix");
    }

    println!("cargo:rustc-link-lib=static=quickfixbind");

    // Lib std C++ is only available on UNIX platform.
    if env::var("CARGO_CFG_UNIX").is_ok() {
        println!("cargo:rustc-link-lib=stdc++");
    }

    // Link with external libraries if needed.
    if have_feature("build-with-mysql") {
        println!("cargo:rustc-link-lib=mysqlclient");
    }
    if have_feature("build-with-postgres") {
        println!("cargo:rustc-link-lib=pq");
    }
}

fn update_sub_repositories() {
    if Path::new("libquickfix/LICENSE").exists() {
        return;
    }

    if !Command::new("git")
        .args(["submodule", "update", "--init", "--recursive"])
        .current_dir("..")
        .status()
        .expect("Fail to get command status")
        .success()
    {
        panic!("Fail to update sub repo");
    }
}
