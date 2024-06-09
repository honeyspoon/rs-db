use log::info;

use serde::{Deserialize, Serialize};

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

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

fn parse_insert(words: &[&str]) -> Result<Statement, &'static str> {
    match words {
        [id, username, email] => match id.parse() {
            Ok(id) => Ok(Statement::Insert(Row {
                id,
                username: username.to_string(),
                email: email.to_string(),
            })),
            _ => Err("invalid id. not a number"),
        },
        _ => Err("invalid insert expected 3 args"),
    }
}

fn prepare_statment(buffer: String) -> Result<Statement, &'static str> {
    let parts: Vec<&str> = buffer.trim().split(' ').collect();

    match parts.as_slice() {
        ["insert", rest @ ..] => parse_insert(rest),
        ["select", rest @ ..] => Err("not handled yet"),
        _ => Err("unknown command"),
    }
}

fn execute_statment(statement: Statement) -> Result<(), &'static str> {
    match statement {
        Statement::Insert(row) => {
            info!("insert {:?}", row);
            Ok(())
        }
        Statement::Select(_) => {
            println!("select");
            Ok(())
        }
    }
}

fn parse_statement(line: String) -> Result<(), &'static str> {
    match prepare_statment(line) {
        Ok(statement) => execute_statment(statement),
        Err(err) => Err(err),
    }
}

fn parse_command(line: String) -> Result<(), &'static str> {
    match line.trim() {
        EXIT_COMMAND => Ok(()),
        HELP_COMMAND => {
            print_help();
            Ok(())
        }
        _ => Err("unknown command"),
    }
}

fn parse_line(line: String) -> Result<(), &'static str> {
    match line.chars().next() {
        Some('.') => parse_command(line),
        Some(_) => parse_statement(line),
        None => Ok(println!("error")),
    }
}

fn main() -> rustyline::Result<()> {
    env_logger::init();

    let mut rl = DefaultEditor::new()?;
    println!("~ rsdb");

    let hist_file = "/tmp/history.txt";
    if rl.load_history(hist_file).is_err() {
        println!("No previous history.");
    }

    loop {
        let readline = rl.readline("> ");

        match readline {
            Ok(line) => {
                if rl.add_history_entry(line.as_str()).is_ok() {
                    rl.save_history(hist_file).unwrap();
                }

                if let Err(err) = parse_line(line) {
                    println!("{}", err);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    info!("end");

    Ok(())
}
