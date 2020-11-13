fn main() {
    let dst = cmake::Config::new("libfceux").very_verbose(true).build();

    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rustc-link-lib=minizip");
    println!("cargo:rustc-link-lib=z");

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=fceux_static");
}
