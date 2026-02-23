use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

use sexp::Atom::*;
use sexp::*;

// === AST (Abstract Syntax Tree) Definition ===
// This enum defines the structure of our language. 
// We use Box<Expr> because the tree is recursive (an Add1 contains another Expr).
#[derive(Debug)]
enum Expr {
    Num(i32),           // A literal integer
    Add1(Box<Expr>),    // (add1 <expr>)
    Sub1(Box<Expr>),    // (sub1 <expr>)
    Negate(Box<Expr>),  // (negate <expr>)
}

/// Converts S-expressions (like "(add1 5)") into our internal Expr AST.
fn parse_expr(s: &Sexp) -> Expr {
    match s {
        // Case: The expression is just a single number (e.g., "10")
        Sexp::Atom(I(n)) => {
            Expr::Num(i32::try_from(*n).unwrap())
        }

        // Case: The expression is a list (e.g., "(op arg)")
        Sexp::List(vec) => {
            match &vec[..] {
                // Matches (add1 <e>)
                [Sexp::Atom(S(op)), e] if op == "add1" => {
                    Expr::Add1(Box::new(parse_expr(e)))
                }
                // Matches (sub1 <e>)
                [Sexp::Atom(S(op)), e] if op == "sub1" => {
                    Expr::Sub1(Box::new(parse_expr(e)))
                }
                // Matches (negate <e>)
                [Sexp::Atom(S(op)), e] if op == "negate" => {
                    Expr::Negate(Box::new(parse_expr(e)))
                }
                _ => panic!("Invalid expression: unexpected operator or argument count"),
            }
        }

        _ => panic!("Invalid expression: expected a number or a parenthesized list"),
    }
}

/// Recursively generates x86-64 assembly instructions for a given expression.
/// We use the RAX register as our primary accumulator for calculations.
fn compile_expr(e: &Expr) -> String {
    match e {
        // Base case: move the number directly into rax
        Expr::Num(n) => {
            format!("mov rax, {}", n)
        }

        // Recursive case: Compile the sub-expression first, then increment rax
        Expr::Add1(subexpr) => {
            format!(
                "{}\nadd rax, 1",
                compile_expr(subexpr)
            )
        }

        // Recursive case: Compile sub-expression, then decrement rax
        Expr::Sub1(subexpr) => {
            format!(
                "{}\nsub rax, 1",
                compile_expr(subexpr)
            )
        }

        // Recursive case: Compile sub-expression, then mathematically negate rax
        Expr::Negate(subexpr) => {
            format!(
                "{}\nneg rax",
                compile_expr(subexpr)
            )
        }
    }
}

/// Wraps the compiled assembly in the necessary boilerplate for an x86-64 program.
/// This defines the entry point that the C/Rust "runner" will call.
fn compile_program(e: &Expr) -> String {
    let body = compile_expr(e);

    format!(
        "
section .text
global our_code_starts_here

our_code_starts_here:
{}
ret
",
        body
    )
}

fn main() -> io::Result<()> {
    // Collect command line arguments (e.g., input_file output_file)
    let args: Vec<String> = env::args().collect();

    // Basic check: Ensure we have enough arguments
    if args.len() < 3 {
        panic!("Usage: cargo run -- <input_file> <output_file>");
    }

    let in_name = &args[1];
    let out_name = &args[2];

    // --- Step 1: Read the source code (.snek file) ---
    let mut in_file = File::open(in_name)?;
    let mut contents = String::new();
    in_file.read_to_string(&mut contents)?;

    // --- Step 2: Parse text into an AST ---
    // parse() comes from the sexp crate; parse_expr() is our custom logic
    let parsed = parse(&contents).expect("Invalid S-expression syntax");
    let expr = parse_expr(&parsed);

    // --- Step 3: Compile the AST into assembly string ---
    let result = compile_program(&expr);

    // --- Step 4: Write assembly to the .s (output) file ---
    let mut out_file = File::create(out_name)?;
    out_file.write_all(result.as_bytes())?;

    Ok(())
}
