#[link(name = "our_code")]
extern "C" {
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here(input: i64) -> i64;
}

/// Called from generated assembly when a runtime type error occurs.
/// errcode 1 → "invalid argument"
#[no_mangle]
pub extern "C" fn snek_error(errcode: i64) {
    match errcode {
        1 => eprintln!("invalid argument"),
        2 => eprintln!("overflow"),
        _ => eprintln!("unknown error (code {})", errcode),
    }
    std::process::exit(1);
}

/// Convert a tagged Cobra value to a human-readable string.
fn snek_str(val: i64) -> String {
    if val == 3 {
        "true".to_string()
    } else if val == 1 {
        "false".to_string()
    } else if val % 2 == 0 {
        // Number: stored as value << 1
        format!("{}", val >> 1)
    } else {
        format!("Unknown value: {:#x}", val)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Default input is false if none provided.
    let raw = if args.len() >= 2 { &args[1] } else { "false" };

    let input_val: i64 = if raw == "true" {
        3          // tagged true
    } else if raw == "false" {
        1          // tagged false
    } else {
        match raw.parse::<i64>() {
            Ok(n) => n << 1,   // tag as number
            Err(_) => {
                eprintln!("Invalid input: {}", raw);
                std::process::exit(1);
            }
        }
    };

    let result: i64 = unsafe { our_code_starts_here(input_val) };
    println!("{}", snek_str(result));
}
