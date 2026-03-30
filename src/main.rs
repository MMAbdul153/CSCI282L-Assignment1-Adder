use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

use sexp::Atom::*;
use sexp::*;

use im::HashMap;

#[derive(Debug)]
enum Op1 {
    Add1,
    Sub1,
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
    Id(String),
    Let(Vec<(String, Expr)>, Box<Expr>),
    UnOp(Op1, Box<Expr>),
    BinOp(Op2, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
}

// ================= PARSER =================

fn parse_expr(s: &Sexp) -> Expr {
    match s {

        Sexp::Atom(I(n)) => Expr::Number(i32::try_from(*n).unwrap()),

        Sexp::Atom(S(s)) if s == "true" => Expr::Boolean(true),
        Sexp::Atom(S(s)) if s == "false" => Expr::Boolean(false),

        Sexp::Atom(S(name)) => Expr::Id(name.to_string()),

        Sexp::List(vec) => match &vec[..] {

            [Sexp::Atom(S(op)), e] if op == "add1" =>
                Expr::UnOp(Op1::Add1, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e] if op == "sub1" =>
                Expr::UnOp(Op1::Sub1, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e1, e2] if op == "+" =>
                Expr::BinOp(Op2::Plus, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "-" =>
                Expr::BinOp(Op2::Minus, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "*" =>
                Expr::BinOp(Op2::Times, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "=" =>
                Expr::BinOp(Op2::Equal, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == ">" =>
                Expr::BinOp(Op2::Greater, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == ">=" =>
                Expr::BinOp(Op2::GreaterEqual, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "<" =>
                Expr::BinOp(Op2::Less, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "<=" =>
                Expr::BinOp(Op2::LessEqual, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), cond, thn, els] if op == "if" =>
                Expr::If(
                    Box::new(parse_expr(cond)),
                    Box::new(parse_expr(thn)),
                    Box::new(parse_expr(els)),
                ),

            [Sexp::Atom(S(op)), Sexp::List(bindings), body] if op == "let" =>
                Expr::Let(parse_bind(bindings), Box::new(parse_expr(body))),

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

fn compile_to_instrs(
    e: &Expr,
    si: i32,
    env: &HashMap<String, i32>,
) -> String {

    match e {

        Expr::Number(n) =>
            format!("mov rax, {}", n << 1),

        Expr::Boolean(true) => "mov rax, 3".to_string(),
        Expr::Boolean(false) => "mov rax, 1".to_string(),

        Expr::Id(name) => {
            match env.get(name) {
                Some(offset) => format!("mov rax, [rsp{}]", offset),
                None => panic!("Unbound variable identifier {}", name),
            }
        }

        Expr::UnOp(op, expr) => {

            let sub = compile_to_instrs(expr, si, env);

            let check = "
test rax, 1
jnz throw_error
";

            match op {
                Op1::Add1 =>
                    format!("{}\n{}\nadd rax, 2", sub, check),
                Op1::Sub1 =>
                    format!("{}\n{}\nsub rax, 2", sub, check),
            }
        }

        Expr::BinOp(op, e1, e2) => {

            let left = compile_to_instrs(e1, si, env);
            let save = format!("mov [rsp{}], rax", -8 * si);
            let right = compile_to_instrs(e2, si + 1, env);

            let check = "
test rax, 1
jnz throw_error
";

            let instr = match op {

                Op2::Plus =>
                    format!("{}\nadd rax, [rsp{}]", check, -8 * si),

                Op2::Minus =>
                    format!(
                        "{}\nmov rbx, [rsp{}]\nsub rbx, rax\nmov rax, rbx",
                        check, -8 * si
                    ),

                Op2::Times =>
                    format!(
                        "{}\nsar rax, 1\nimul rax, [rsp{}]",
                        check, -8 * si
                    ),

                Op2::Equal =>
                    format!(
                        "cmp rax, [rsp{}]\nmov rax, 3\nje done{}\nmov rax, 1\ndone{}:",
                        -8 * si, si, si
                    ),

                Op2::Greater =>
                    format!(
                        "cmp [rsp{}], rax\nmov rax, 3\njg done{}\nmov rax, 1\ndone{}:",
                        -8 * si, si, si
                    ),

                Op2::GreaterEqual =>
                    format!(
                        "cmp [rsp{}], rax\nmov rax, 3\njge done{}\nmov rax, 1\ndone{}:",
                        -8 * si, si, si
                    ),

                Op2::Less =>
                    format!(
                        "cmp [rsp{}], rax\nmov rax, 3\njl done{}\nmov rax, 1\ndone{}:",
                        -8 * si, si, si
                    ),

                Op2::LessEqual =>
                    format!(
                        "cmp [rsp{}], rax\nmov rax, 3\njle done{}\nmov rax, 1\ndone{}:",
                        -8 * si, si, si
                    ),
            };

            format!("{}\n{}\n{}\n{}", left, save, right, instr)
        }

        Expr::If(cond, thn, els) => {

            let cond_code = compile_to_instrs(cond, si, env);
            let thn_code = compile_to_instrs(thn, si, env);
            let els_code = compile_to_instrs(els, si, env);

            let else_label = format!("else_{}", si);
            let end_label = format!("ifend_{}", si);

            format!(
                "{}
cmp rax, 1
je {}
{}
jmp {}
{}:
{}
{}:",
                cond_code,
                else_label,
                thn_code,
                end_label,
                else_label,
                els_code,
                end_label
            )
        }

        Expr::Let(bindings, body) => {

            let mut new_env = env.clone();
            let mut code = String::new();
            let mut new_si = si;

            for (name, expr) in bindings {

                if new_env.contains_key(name) {
                    panic!("Duplicate binding");
                }

                code += &compile_to_instrs(expr, new_si, &new_env);
                code += &format!("\nmov [rsp{}], rax\n", -8 * new_si);

                new_env = new_env.update(name.clone(), -8 * new_si);
                new_si += 1;
            }

            code += &compile_to_instrs(body, new_si, &new_env);

            code
        }
    }
}

fn compile(e: &Expr) -> String {

    let env = HashMap::new();
    let body = compile_to_instrs(e, 2, &env);

    format!(
        "
section .text
extern snek_error
global our_code_starts_here

our_code_starts_here:
{}
ret

throw_error:
mov rdi, 1
call snek_error
",
        body
    )
}

// ================= MAIN =================

fn main() -> io::Result<()> {

    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        panic!("Usage: cargo run -- <input> <output>");
    }

    let mut in_file = File::open(&args[1])?;
    let mut contents = String::new();
    in_file.read_to_string(&mut contents)?;

    let parsed = parse(&contents).unwrap();
    let expr = parse_expr(&parsed);

    let result = compile(&expr);

    let mut out_file = File::create(&args[2])?;
    out_file.write_all(result.as_bytes())?;

    Ok(())
}