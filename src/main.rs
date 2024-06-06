use std::io;
use std::io::Write;

fn printhelp() {
    println!("help");
}

const EXIT_COMMAND: &str = ".exit";
const HELP_COMMAND: &str = ".help";

enum Statement {
    Select,
    Insert,
}

fn prepare_statment(buffer: &String) -> Option<Statement> {
    match buffer.trim() {
        "select" => Some(Statement::Select),
        "insert" => Some(Statement::Insert),
        _ => None,
    }
}

fn execute_statment(statement: Statement) {
    match statement {
        Statement::Insert => {
            println!("insert");
        }
        Statement::Select => {
            println!("select");
        }
    }
}

fn main() -> io::Result<()> {
    println!("~ rsdb");

    let mut exit = false;
    while !exit {
        print!("> ");
        let mut buffer = String::new();
        let _ = io::stdout().flush();
        io::stdin().read_line(&mut buffer)?;

        match buffer.chars().next() {
            Some('.') => match buffer.trim() {
                EXIT_COMMAND => exit = true,
                HELP_COMMAND => printhelp(),
                _ => println!("unknown command"),
            },
            Some('\n') => println!("nothing"),
            Some(_) => match prepare_statment(&buffer) {
                Some(statement) => execute_statment(statement),
                None => println!("unsupported command"),
            },
            None => println!("error"),
        };
    }

    println!("bye");
    Ok(())
}
