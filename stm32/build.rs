fn main() {
    // println!("RUSTFLAGS=\"-Zfmt-debug=none-Z location-detail=none\"");
    // println!("-Z build-std-features=\"optimize_for_size\"");

    println!("cargo:rerun-if-env-changed=DEFMT_LOG");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
}
