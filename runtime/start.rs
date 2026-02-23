#[link(name = "our_code")]
extern "C" {
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here() -> i64;
}

fn main() {
    let result: i64 = unsafe { our_code_starts_here() };
    println!("{}", result);
}