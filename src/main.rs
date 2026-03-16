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
}

#[derive(Debug)]
enum Expr {
    Number(i32),
    Id(String),
    Let(Vec<(String, Expr)>, Box<Expr>),
    UnOp(Op1, Box<Expr>),
    BinOp(Op2, Box<Expr>, Box<Expr>),
}

fn parse_expr(s: &Sexp) -> Expr {
    match s {

        Sexp::Atom(I(n)) => Expr::Number(i32::try_from(*n).unwrap()),

        Sexp::Atom(S(name)) => Expr::Id(name.to_string()),

        Sexp::List(vec) => match &vec[..] {

            [Sexp::Atom(S(op)), e] if op == "add1" =>
                Expr::UnOp(Op1::Add1, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e] if op == "sub1" =>
                Expr::UnOp(Op1::Sub1, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e1, e2] if op == "+" =>
                Expr::BinOp(Op2::Plus,
                    Box::new(parse_expr(e1)),
                    Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "-" =>
                Expr::BinOp(Op2::Minus,
                    Box::new(parse_expr(e1)),
                    Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), e1, e2] if op == "*" =>
                Expr::BinOp(Op2::Times,
                    Box::new(parse_expr(e1)),
                    Box::new(parse_expr(e2))),

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

fn compile_to_instrs(
    e: &Expr,
    si: i32,
    env: &HashMap<String, i32>,
) -> String {

    match e {

        Expr::Number(n) =>
            format!("mov rax, {}", n),

        Expr::Id(name) => {
            match env.get(name) {

                Some(offset) =>
                    format!("mov rax, [rsp{}]", offset),

                None =>
                    panic!("Unbound variable identifier {}", name),
            }
        }

        Expr::UnOp(op, expr) => {

            let sub = compile_to_instrs(expr, si, env);

            match op {

                Op1::Add1 =>
                    format!("{}\nadd rax, 1", sub),

                Op1::Sub1 =>
                    format!("{}\nsub rax, 1", sub),
            }
        }

        Expr::BinOp(op, e1, e2) => {

            let left = compile_to_instrs(e1, si, env);

            let save =
                format!("mov [rsp{}], rax", -8 * si);

            let right =
                compile_to_instrs(e2, si + 1, env);

            let instr = match op {

                Op2::Plus =>
                    format!("add rax, [rsp{}]", -8 * si),

                Op2::Minus =>
                    format!(
                        "mov rbx, [rsp{}]\nsub rbx, rax\nmov rax, rbx",
                        -8 * si
                    ),

                Op2::Times =>
                    format!("imul rax, [rsp{}]", -8 * si),
            };

            format!(
                "{}\n{}\n{}\n{}",
                left, save, right, instr
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

                code += &format!(
                    "\nmov [rsp{}], rax\n",
                    -8 * new_si
                );

                new_env =
                    new_env.update(name.clone(), -8 * new_si);

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

    let mut in_file = File::open(in_name)?;
    let mut contents = String::new();
    in_file.read_to_string(&mut contents)?;

    let parsed = parse(&contents).unwrap();

    let expr = parse_expr(&parsed);

    let result = compile(&expr);

    let mut out_file = File::create(out_name)?;
    out_file.write_all(result.as_bytes())?;

    Ok(())
}