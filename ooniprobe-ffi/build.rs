// ooniprobe-ffi/build.rs
fn main() {
    uniffi::generate_scaffolding("src/ooniprobe.udl").unwrap();
}
