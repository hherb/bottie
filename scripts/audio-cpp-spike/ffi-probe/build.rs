//! Points the linker at a locally built `libaudiocpp`.
//!
//! `AUDIOCPP_LIB_DIR` is the directory holding `libaudiocpp.so`/`.dylib`/`.lib`,
//! which `build-libaudiocpp.sh` leaves in the audio.cpp build tree's `bin/`.

fn main() {
    let lib_dir = std::env::var("AUDIOCPP_LIB_DIR")
        .expect("set AUDIOCPP_LIB_DIR to the directory containing libaudiocpp");

    println!("cargo:rerun-if-env-changed=AUDIOCPP_LIB_DIR");
    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=dylib=audiocpp");
    // So the probe runs without the caller setting LD_LIBRARY_PATH/DYLD_LIBRARY_PATH.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
}
