fn main() {
    let root = "vendor/rcheevos";
    let mut b = cc::Build::new();
    for f in ["hash_rom.c", "hash_disc.c", "hash_zip.c", "cdreader.c", "md5.c"] {
        b.file(format!("{root}/src/rhash/{f}"));
    }
    for f in ["rc_compat.c", "rc_util.c", "rc_version.c"] {
        b.file(format!("{root}/src/{f}"));
    }
    b.file("csrc/hash_candidates.c"); // includes rhash/hash.c
    b.file("csrc/sizeof.c");
    b.include(format!("{root}/include"))
        .include(format!("{root}/src"))
        .define("RC_HASH_NO_ENCRYPTED", None)
        .warnings(false)
        .compile("rcheevos_hash");
    println!("cargo:rerun-if-changed={root}/src/rhash");
    println!("cargo:rerun-if-changed=csrc");
}
