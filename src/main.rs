use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

use sexp::Atom::*;
use sexp::*;

use im::HashMap;

// ================= AST =================

#[derive(Debug)]
enum Op1 {
    Add1,
    Sub1,
    Negate,
    IsNum,
    IsBool,
}

#[derive(Debug)]
enum Op2 {
    Plus,
    Minus,
    Times,
    Equal,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Debug)]
enum Expr {
    Number(i32),
    Boolean(bool),
    Input,
    Id(String),
    Let(Vec<(String, Expr)>, Box<Expr>),
    UnOp(Op1, Box<Expr>),
    BinOp(Op2, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Block(Vec<Expr>),
    Loop(Box<Expr>),
    Break(Box<Expr>),
    Set(String, Box<Expr>),
}

// ================= PARSER =================

fn parse_expr(s: &Sexp) -> Expr {
    match s {
        Sexp::Atom(I(n)) => Expr::Number(i32::try_from(*n).unwrap()),

        Sexp::Atom(S(s)) if s == "true"  => Expr::Boolean(true),
        Sexp::Atom(S(s)) if s == "false" => Expr::Boolean(false),
        Sexp::Atom(S(s)) if s == "input" => Expr::Input,

        Sexp::Atom(S(name)) => Expr::Id(name.to_string()),

        Sexp::List(vec) => match &vec[..] {

            [Sexp::Atom(S(op)), e] if op == "add1" =>
                Expr::UnOp(Op1::Add1, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e] if op == "sub1" =>
                Expr::UnOp(Op1::Sub1, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e] if op == "negate" =>
                Expr::UnOp(Op1::Negate, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e] if op == "isnum" =>
                Expr::UnOp(Op1::IsNum, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e] if op == "isbool" =>
                Expr::UnOp(Op1::IsBool, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e1, e2] if op == "+" =>
                Expr::BinOp(Op2::Plus,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "-" =>
                Expr::BinOp(Op2::Minus,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "*" =>
                Expr::BinOp(Op2::Times,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "=" =>
                Expr::BinOp(Op2::Equal,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == ">" =>
                Expr::BinOp(Op2::Greater,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == ">=" =>
                Expr::BinOp(Op2::GreaterEqual,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "<" =>
                Expr::BinOp(Op2::Less,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "<=" =>
                Expr::BinOp(Op2::LessEqual,
                    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), cond, thn, els] if op == "if" =>
                Expr::If(
                    Box::new(parse_expr(cond)),
                    Box::new(parse_expr(thn)),
                    Box::new(parse_expr(els)),
                ),

            [Sexp::Atom(S(op)), Sexp::List(bindings), body] if op == "let" =>
                Expr::Let(parse_bind(bindings), Box::new(parse_expr(body))),

            [Sexp::Atom(S(op)), body] if op == "loop" =>
                Expr::Loop(Box::new(parse_expr(body))),

            [Sexp::Atom(S(op)), expr] if op == "break" =>
                Expr::Break(Box::new(parse_expr(expr))),

            [Sexp::Atom(S(op)), Sexp::Atom(S(name)), expr] if op == "set!" =>
                Expr::Set(name.to_string(), Box::new(parse_expr(expr))),

            // (block e1 e2 ... eN) — one or more expressions
            [Sexp::Atom(S(op)), rest @ ..] if op == "block" => {
                if rest.is_empty() {
                    panic!("block requires at least one expression");
                }
                Expr::Block(rest.iter().map(parse_expr).collect())
            }

            _ => panic!("Invalid expression"),
        },

        _ => panic!("Invalid"),
    }
}

fn parse_bind(bindings: &Vec<Sexp>) -> Vec<(String, Expr)> {
    let mut result = Vec::new();
    for bind in bindings {
        match bind {
            Sexp::List(vec) => match &vec[..] {
                [Sexp::Atom(S(name)), expr] =>
                    result.push((name.to_string(), parse_expr(expr))),
                _ => panic!("Invalid binding"),
            },
            _ => panic!("Invalid binding"),
        }
    }
    result
}

// ================= COMPILER =================

/// Generate a fresh unique label (e.g. "if_else_3").
fn new_label(lc: &mut i32, prefix: &str) -> String {
    *lc += 1;
    format!("{}_{}", prefix, lc)
}

/// Compile an expression to x86-64 NASM instructions.
///
/// Parameters:
///   e            – the expression to compile
///   si           – next available stack slot index (offset = -8 * si)
///   env          – maps variable names to their stack offsets
///   lc           – mutable label counter for generating unique labels
///   break_target – label to jump to when `break` is encountered (None outside loops)
fn compile_expr(
    e: &Expr,
    si: i32,
    env: &HashMap<String, i32>,
    lc: &mut i32,
    break_target: &Option<String>,
) -> String {

    match e {

        // ---- Literals -------------------------------------------------------

        Expr::Number(n) => format!("  mov rax, {}", (*n as i64) << 1),

        Expr::Boolean(true)  => "  mov rax, 3".to_string(),   // 0b11
        Expr::Boolean(false) => "  mov rax, 1".to_string(),   // 0b01

        // ---- Input ----------------------------------------------------------

        Expr::Input => {
            // input is stored at [rsp-8] (stack slot 1) by the prologue
            "  mov rax, [rsp-8]".to_string()
        }

        // ---- Variables ------------------------------------------------------

        Expr::Id(name) => {
            match env.get(name) {
                Some(offset) => format!("  mov rax, [rsp{}]", offset),
                None => panic!("Unbound variable identifier {}", name),
            }
        }

        // ---- Unary operations -----------------------------------------------

        Expr::UnOp(op, inner) => {
            let sub = compile_expr(inner, si, env, lc, break_target);
            // type-check: rax must be a number (LSB == 0)
            let num_check = "  test rax, 1\n  jnz throw_error";

            match op {
                Op1::Add1   => format!("{}\n{}\n  add rax, 2", sub, num_check),
                Op1::Sub1   => format!("{}\n{}\n  sub rax, 2", sub, num_check),
                Op1::Negate => format!("{}\n{}\n  neg rax",    sub, num_check),

                Op1::IsNum => {
                    // returns true (3) if LSB == 0, else false (1)
                    let done = new_label(lc, "isnum_done");
                    format!(
                        "{}\n  test rax, 1\n  mov rax, 3\n  jz {}\n  mov rax, 1\n{}:",
                        sub, done, done
                    )
                }

                Op1::IsBool => {
                    // returns true (3) if LSB == 1, else false (1)
                    let done = new_label(lc, "isbool_done");
                    format!(
                        "{}\n  test rax, 1\n  mov rax, 1\n  jz {}\n  mov rax, 3\n{}:",
                        sub, done, done
                    )
                }
            }
        }

        // ---- Binary operations ----------------------------------------------

        Expr::BinOp(op, e1, e2) => {
            // Evaluate e1, save to stack; evaluate e2 into rax.
            let left_code  = compile_expr(e1, si,     env, lc, break_target);
            let save        = format!("  mov [rsp{}], rax", -8 * si);
            let right_code = compile_expr(e2, si + 1, env, lc, break_target);

            // -- Helpers (strings) --
            // check rax (e2) is a number
            let chk_e2 = "  test rax, 1\n  jnz throw_error".to_string();
            // check [rsp-8*si] (e1) is a number
            let chk_e1 = format!(
                "  mov rbx, [rsp{}]\n  test rbx, 1\n  jnz throw_error",
                -8 * si
            );
            // check both operands have the same type (for '=')
            let chk_same_type = format!(
                "  mov rbx, rax\n  xor rbx, [rsp{}]\n  test rbx, 1\n  jnz throw_error",
                -8 * si
            );

            let op_code = match op {

                Op2::Plus => format!(
                    "{}\n{}\n  add rax, [rsp{}]",
                    chk_e2, chk_e1, -8 * si
                ),

                Op2::Minus => format!(
                    "{}\n{}\n  mov rbx, [rsp{}]\n  sub rbx, rax\n  mov rax, rbx",
                    chk_e2, chk_e1, -8 * si
                ),

                Op2::Times => format!(
                    "{}\n{}\n  sar rax, 1\n  imul rax, [rsp{}]",
                    chk_e2, chk_e1, -8 * si
                ),

                Op2::Equal => {
                    let done = new_label(lc, "eq_done");
                    format!(
                        "{}\n  cmp rax, [rsp{}]\n  mov rax, 3\n  je {}\n  mov rax, 1\n{}:",
                        chk_same_type, -8 * si, done, done
                    )
                }

                Op2::Greater => {
                    let done = new_label(lc, "cmp_done");
                    format!(
                        "{}\n{}\n  cmp [rsp{}], rax\n  mov rax, 3\n  jg {}\n  mov rax, 1\n{}:",
                        chk_e2, chk_e1, -8 * si, done, done
                    )
                }

                Op2::GreaterEqual => {
                    let done = new_label(lc, "cmp_done");
                    format!(
                        "{}\n{}\n  cmp [rsp{}], rax\n  mov rax, 3\n  jge {}\n  mov rax, 1\n{}:",
                        chk_e2, chk_e1, -8 * si, done, done
                    )
                }

                Op2::Less => {
                    let done = new_label(lc, "cmp_done");
                    format!(
                        "{}\n{}\n  cmp [rsp{}], rax\n  mov rax, 3\n  jl {}\n  mov rax, 1\n{}:",
                        chk_e2, chk_e1, -8 * si, done, done
                    )
                }

                Op2::LessEqual => {
                    let done = new_label(lc, "cmp_done");
                    format!(
                        "{}\n{}\n  cmp [rsp{}], rax\n  mov rax, 3\n  jle {}\n  mov rax, 1\n{}:",
                        chk_e2, chk_e1, -8 * si, done, done
                    )
                }
            };

            format!("{}\n{}\n{}\n{}", left_code, save, right_code, op_code)
        }

        // ---- If -------------------------------------------------------------

        Expr::If(cond, thn, els) => {
            // Generate labels BEFORE compiling sub-expressions so that
            // sub-expression labels always have higher numbers.
            let else_lbl = new_label(lc, "if_else");
            let end_lbl  = new_label(lc, "if_end");

            let cond_code = compile_expr(cond, si, env, lc, break_target);
            let thn_code  = compile_expr(thn,  si, env, lc, break_target);
            let els_code  = compile_expr(els,  si, env, lc, break_target);

            // false == 0b01; anything else takes the then branch
            format!(
                "{}\n  cmp rax, 1\n  je {}\n{}\n  jmp {}\n{}:\n{}\n{}:",
                cond_code, else_lbl,
                thn_code,
                end_lbl,
                else_lbl,
                els_code,
                end_lbl
            )
        }

        // ---- Block ----------------------------------------------------------

        Expr::Block(exprs) => {
            // Evaluate each expression in order; result is the last one.
            exprs
                .iter()
                .map(|e| compile_expr(e, si, env, lc, break_target))
                .collect::<Vec<_>>()
                .join("\n")
        }

        // ---- Loop / Break ---------------------------------------------------

        Expr::Loop(body) => {
            let loop_start = new_label(lc, "loop_start");
            let loop_end   = new_label(lc, "loop_end");

            let body_code = compile_expr(
                body, si, env, lc,
                &Some(loop_end.clone()),   // break targets this loop's end
            );

            format!(
                "{}:\n{}\n  jmp {}\n{}:",
                loop_start, body_code, loop_start, loop_end
            )
        }

        Expr::Break(expr) => {
            match break_target {
                Some(lbl) => {
                    let val_code = compile_expr(expr, si, env, lc, break_target);
                    format!("{}\n  jmp {}", val_code, lbl)
                }
                None => panic!("break used outside of a loop"),
            }
        }

        // ---- Set! -----------------------------------------------------------

        Expr::Set(name, expr) => {
            match env.get(name) {
                Some(offset) => {
                    let val_code = compile_expr(expr, si, env, lc, break_target);
                    format!("{}\n  mov [rsp{}], rax", val_code, offset)
                }
                None => panic!("Unbound variable identifier {}", name),
            }
        }

        // ---- Let ------------------------------------------------------------

        Expr::Let(bindings, body) => {
            let mut new_env = env.clone();
            let mut code    = String::new();
            let mut new_si  = si;
            // Track only names bound in THIS let to detect local duplicates.
            let mut seen = std::collections::HashSet::new();

            for (name, init_expr) in bindings {
                if seen.contains(name) {
                    panic!("Duplicate binding: {}", name);
                }
                seen.insert(name.clone());

                // Compile the init expression using env BEFORE this binding
                code += &compile_expr(init_expr, new_si, &new_env, lc, break_target);
                code += &format!("\n  mov [rsp{}], rax\n", -8 * new_si);

                // Make the binding visible for subsequent bindings and the body
                new_env = new_env.update(name.clone(), -8 * new_si);
                new_si += 1;
            }

            code += &compile_expr(body, new_si, &new_env, lc, break_target);
            code
        }
    }
}

// ================= TOP-LEVEL COMPILE =================

fn compile(e: &Expr) -> String {
    // Stack layout:
    //   [rsp-8]  = input  (stored from rdi in the prologue)
    //   [rsp-16] onwards = temporaries / let bindings
    //
    // We pre-populate the env with `input` at offset -8 so that the
    // Expr::Input arm can load it like any other variable.
    let mut env: HashMap<String, i32> = HashMap::new();
    env = env.update("input".to_string(), -8i32);

    let mut lc = 0i32;
    let body = compile_expr(e, 2, &env, &mut lc, &None);

    format!(
"section .text
extern snek_error
global our_code_starts_here

our_code_starts_here:
  mov [rsp-8], rdi
{}
  ret

throw_error:
  mov rdi, 1
  call snek_error
  ret
",
        body
    )
}

// ================= MAIN =================

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        panic!("Usage: cargo run -- <input.snek> <output.s>");
    }

    let mut in_file  = File::open(&args[1])?;
    let mut contents = String::new();
    in_file.read_to_string(&mut contents)?;

    let parsed = parse(&contents).unwrap();
    let expr   = parse_expr(&parsed);
    let result = compile(&expr);

    let mut out_file = File::create(&args[2])?;
    out_file.write_all(result.as_bytes())?;

    Ok(())
}
