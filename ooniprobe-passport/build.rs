// ooniprobe-passport/build.rs
fn main() {
    uniffi::generate_scaffolding("src/passport.udl").unwrap();
}
