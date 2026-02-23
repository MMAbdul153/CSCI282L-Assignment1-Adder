use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

use sexp::Atom::*;
use sexp::*;

// === AST Definition ===

#[derive(Debug)]
enum Expr {
    Num(i32),
    Add1(Box<Expr>),
    Sub1(Box<Expr>),
    Negate(Box<Expr>),
}

fn parse_expr(s: &Sexp) -> Expr {
    match s {
        Sexp::Atom(I(n)) => {
            Expr::Num(i32::try_from(*n).unwrap())
        }

        Sexp::List(vec) => {
            match &vec[..] {
                [Sexp::Atom(S(op)), e] if op == "add1" => {
                    Expr::Add1(Box::new(parse_expr(e)))
                }
                [Sexp::Atom(S(op)), e] if op == "sub1" => {
                    Expr::Sub1(Box::new(parse_expr(e)))
                }
                [Sexp::Atom(S(op)), e] if op == "negate" => {
                    Expr::Negate(Box::new(parse_expr(e)))
                }
                _ => panic!("Invalid expression"),
            }
        }

        _ => panic!("Invalid expression"),
    }
}

fn compile_expr(e: &Expr) -> String {
    match e {
        Expr::Num(n) => {
            format!("mov rax, {}", n)
        }

        Expr::Add1(subexpr) => {
            format!(
                "{}\nadd rax, 1",
                compile_expr(subexpr)
            )
        }

        Expr::Sub1(subexpr) => {
            format!(
                "{}\nsub rax, 1",
                compile_expr(subexpr)
            )
        }

        Expr::Negate(subexpr) => {
            format!(
                "{}\nneg rax",
                compile_expr(subexpr)
            )
        }
    }
}

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
    let args: Vec<String> = env::args().collect();

    let in_name = &args[1];
    let out_name = &args[2];

    // Read input file
    let mut in_file = File::open(in_name)?;
    let mut contents = String::new();
    in_file.read_to_string(&mut contents)?;

    // Parse S-expression
    let parsed = parse(&contents).unwrap();
    let expr = parse_expr(&parsed);

    // Compile to assembly
    let result = compile_program(&expr);

    // Write to output file
    let mut out_file = File::create(out_name)?;
    out_file.write_all(result.as_bytes())?;

    Ok(())
}