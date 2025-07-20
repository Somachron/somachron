fn main() {
    for (key, value) in std::env::vars() {
        println!("cargo:rustc-env={}={}", key, value);
    }
}
