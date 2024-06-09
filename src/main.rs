use log::{error, info};
use std::io;
use std::io::Write;

use serde::{Deserialize, Serialize};

fn print_help() {
    println!("help");
}

const EXIT_COMMAND: &str = ".exit";
const HELP_COMMAND: &str = ".help";

const COLUMN_USERNAME_SIZE: usize = 32;
const COLUMN_EMAIL_SIZE: usize = 255;

#[derive(Serialize, Deserialize, Debug)]
struct Row {
    id: i32,
    username: String,
    email: String,
}

enum Statement {
    Select(Row),
    Insert(Row),
}

fn parse_insert(words: &[&str]) -> Option<Statement> {
    match words {
        [id, username, email] => match id.parse() {
            Ok(id) => Some(Statement::Insert(Row {
                id,
                username: username.to_string(),
                email: email.to_string(),
            })),
            _ => {
                error!("invalid id");
                None
            }
        },
        _ => {
            error!("invalid insert expected 3 args");
            None
        }
    }
}

fn prepare_statment(buffer: &str) -> Option<Statement> {
    let parts: Vec<&str> = buffer.trim().split(' ').collect();

    match parts.as_slice() {
        ["insert", rest @ ..] => parse_insert(rest),
        ["select", rest @ ..] => None,
        _ => None,
    }
}

fn execute_statment(statement: Statement) {
    match statement {
        Statement::Insert(row) => {
            info!("insert {:?}", row);
        }
        Statement::Select(_) => {
            println!("select");
        }
    }
}

fn parse_statement(line: &str) {
    match prepare_statment(line) {
        Some(statement) => execute_statment(statement),
        None => println!("unsupported command"),
    }
}

fn parse_command(line: &str) -> bool {
    match line.trim() {
        EXIT_COMMAND => return true,
        HELP_COMMAND => print_help(),
        _ => println!("unknown command"),
    };
    false
}

fn parse_line(line: &str) -> bool {
    match line.chars().next() {
        Some('.') => return parse_command(line),
        Some('\n') => {}
        Some(_) => parse_statement(line),
        None => println!("error"),
    };
    false
}

fn main() -> io::Result<()> {
    env_logger::init();
    println!("~ rsdb");

    loop {
        print!("> ");
        let mut buffer = String::new();
        let _ = io::stdout().flush();
        io::stdin().read_line(&mut buffer)?;
        if parse_line(&buffer) {
            break;
        }
    }

    info!("end");

    Ok(())
}
