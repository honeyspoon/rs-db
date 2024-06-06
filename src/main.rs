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

fn prepare_statment(buffer: &str) -> Option<Statement> {
    let parts: Vec<&str> = buffer.trim().split(' ').collect();

    match parts.as_slice() {
        ["insert", rest @ ..] => Some(Statement::Insert),
        ["select", rest @ ..] => Some(Statement::Select),
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

fn parse_command(line: &str) -> bool {
    match line.trim() {
        EXIT_COMMAND => return true,
        HELP_COMMAND => printhelp(),
        _ => println!("unknown command"),
    }
    false
}

fn parse_line(line: &str) -> bool {
    match line.chars().next() {
        Some('.') => return parse_command(line),
        Some('\n') => println!("nothing"),
        Some(_) => match prepare_statment(line) {
            Some(statement) => execute_statment(statement),
            None => println!("unsupported command"),
        },
        None => println!("error"),
    };
    false
}

fn main() -> io::Result<()> {
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

    println!("bye");
    Ok(())
}
