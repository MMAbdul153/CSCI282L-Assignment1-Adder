#[link(name = "our_code")]
extern "C" {
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here() -> i64;
}
#[no_mangle]
pub extern "C" fn snek_error(errcode: i64) {
    println!("Error {}", errcode);
    std::process::exit(1);
}

fn main() {
    let result: i64 = unsafe { our_code_starts_here() };
    println!("{}", result);
} 