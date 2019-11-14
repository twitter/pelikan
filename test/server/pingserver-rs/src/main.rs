
fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");

    check_pingserver_rs::main();
}
