use log::info;
use std::fmt;

use serde::{Deserialize, Serialize};

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const TABLE_MAX_PAGES: usize = 100;
const PAGE_SIZE: usize = 4096;

struct Table {
    nb_rows: usize,
    pages: Vec<Option<Page>>,
    row_size: usize,
}

impl Table {
    fn new() -> Self {
        let row_size = bincode::serialized_size(&Row::new(0, "", "")).unwrap() as usize;

        Self {
            nb_rows: 0,
            pages: vec![None; TABLE_MAX_PAGES],
            row_size,
        }
    }

    fn ensure_page_exists(&mut self, page_id: usize) {
        if self.pages[page_id].is_none() {
            let capacity = self.get_row_per_page() * self.row_size;
            self.pages[page_id] = Some(vec![0; capacity]);
        }
    }

    fn get_row_per_page(&self) -> usize {
        PAGE_SIZE / self.row_size
    }

    fn get_page_id(&self, row_id: usize) -> usize {
        row_id / self.get_row_per_page()
    }

    fn get_row_offset(&self, row_id: usize) -> usize {
        (row_id % self.get_row_per_page()) * self.row_size
    }

    fn is_full(&self) -> bool {
        let max = self.get_row_per_page() * TABLE_MAX_PAGES;
        self.nb_rows == max
    }

    fn insert_row(&mut self, row: &Row) -> Result<(), &'static str> {
        if self.is_full() {
            return Err("Table is full");
        }

        match bincode::serialize(&row) {
            Ok(row_bytes) => {
                let row_offset = self.get_row_offset(row.id as usize);
                let page_id = self.get_page_id(row.id as usize);
                println!(
                    "row id: {}, page_id: {}, row_offset: {}",
                    row.id, page_id, row_offset
                );
                self.ensure_page_exists(page_id);

                match self.copy_to_page(page_id, row_offset, &row_bytes) {
                    Ok(_) => {
                        self.nb_rows += 1;
                        Ok(())
                    }
                    Err(_) => Err("failed to copy to page"),
                }
            }
            Err(_) => Err("failed to serialize row"),
        }
    }

    fn copy_to_page(
        &mut self,
        page_id: usize,
        row_offset: usize,
        row_bytes: &[u8],
    ) -> Result<(), &'static str> {
        match self.pages[page_id].as_mut() {
            Some(page) => {
                page[row_offset..row_offset + row_bytes.len()].copy_from_slice(row_bytes);
                Ok(())
            }
            None => Err("failed to get page"),
        }
    }

    fn select_row(&self) -> Vec<Row> {
        (0..self.nb_rows)
            .map(|row_id| {
                let page_id = self.get_page_id(row_id);
                let page = self.pages[page_id].as_ref().unwrap();
                let row_offset = self.get_row_offset(row_id);
                let slice = &page[row_offset..row_offset + self.row_size];
                bincode::deserialize(slice).unwrap()
            })
            .collect()
    }
}

type Page = Vec<u8>;

#[derive(Serialize, Deserialize, PartialEq)]
struct Row {
    id: u32,
    username: String,
    email: String,
}

const COLUMN_USERNAME_SIZE: usize = 32;
const COLUMN_EMAIL_SIZE: usize = 255;

impl Row {
    fn new(id: u32, username: &str, email: &str) -> Self {
        Self {
            id,
            username: pad_string(username, COLUMN_USERNAME_SIZE),
            email: pad_string(email, COLUMN_EMAIL_SIZE),
            // should I pad the strings here or when I (de)serialize them?
        }
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Row")
            .field("id", &self.id)
            .field("username", &self.username.trim())
            .field("email", &self.email.trim())
            .finish()
    }
}

fn pad_string(input: &str, size: usize) -> String {
    let mut s = String::from(input);
    s.truncate(size);
    s.push_str(&" ".repeat(size - s.len()));
    s
}

#[derive(Debug, PartialEq)]
enum Statement {
    Select,
    Insert(Row),
}

fn parse_insert(words: &[&str]) -> Result<Statement, &'static str> {
    match words {
        // should new do the validation or should it be done before ?
        [id, username, email] => match id.parse() {
            Ok(id) => Ok(Statement::Insert(Row::new(id, username, email))),
            _ => Err("invalid id. not a number"),
        },
        _ => Err("invalid insert expected 3 args"),
    }
}

fn execute_statment(statement: Statement, table: &mut Table) -> Result<String, &'static str> {
    match statement {
        Statement::Insert(row) => {
            let out = String::new();
            table.insert_row(&row)?;
            Ok(out)
        }
        Statement::Select => {
            let mut out = String::new();
            for row in table.select_row() {
                out += format!("{:?}\n", row).as_str();
            }
            Ok(out)
        }
    }
}

fn parse_statement(line: String) -> Result<Statement, &'static str> {
    let parts: Vec<&str> = line.trim().split(' ').collect();

    match parts.as_slice() {
        ["insert", rest @ ..] => parse_insert(rest),
        ["select"] => Ok(Statement::Select),
        _ => Err("unknown command"),
    }
}

// find a way to just put the strings in the command enum and match on the underlying
// strum?

#[derive(Debug, PartialEq)]
enum Command {
    Help,
    Exit,
}
const EXIT_COMMAND: &str = ".exit";
const HELP_COMMAND: &str = ".help";

fn execute_command(command: Command) {
    match command {
        Command::Help => print_help(),
        Command::Exit => exit(),
    }
}

fn print_help() {
    println!("help");
}

fn exit() {
    std::process::exit(0)
}

fn parse_command(line: String) -> Result<Command, &'static str> {
    match line.trim() {
        EXIT_COMMAND => Ok(Command::Exit),
        HELP_COMMAND => Ok(Command::Help),
        _ => Err("unknown command"),
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

    let mut table = Table::new();

    loop {
        let readline = rl.readline("> ");

        match readline {
            Ok(line) => {
                if rl.add_history_entry(line.as_str()).is_ok() {
                    rl.save_history(hist_file).unwrap();
                }
                match line.chars().next() {
                    Some('.') => {
                        let command = parse_command(line).unwrap();
                        execute_command(command);
                    }
                    Some(_) => {
                        let statement = parse_statement(line).unwrap();
                        match execute_statment(statement, &mut table) {
                            Ok(out) => println!("{}", out),
                            Err(err) => println!("Error: {}", err),
                        };
                    }
                    None => {
                        println!("empty line");
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands() {
        assert_eq!(parse_command(String::from(".help")).unwrap(), Command::Help);
        assert_eq!(parse_command(String::from(".exit")).unwrap(), Command::Exit);
        assert_eq!(
            parse_command(String::from(".elxit")),
            Err("unknown command")
        );
    }

    #[test]
    fn statement_select() {
        assert_eq!(
            parse_statement(String::from("select")).unwrap(),
            Statement::Select
        );
    }

    #[test]
    fn statement_insert() {
        assert_eq!(
            parse_statement(String::from("insert 1 abc def")).unwrap(),
            Statement::Insert(Row::new(1, "abc", "def"))
        );
        assert_eq!(
            parse_statement(String::from("insert")),
            Err("invalid insert expected 3 args")
        );
        assert_eq!(
            parse_statement(String::from("insert a abc def")),
            Err("invalid id. not a number")
        );
    }

    #[test]
    fn insert_truncate() {
        let username: String = "0123456789123456789012345678901234".to_string();
        let email: String = (0..COLUMN_EMAIL_SIZE + 4).map(|_| "X").collect::<String>();
        let query = format!("insert 1 {} {}", username, email);
        let Statement::Insert(row) = parse_statement(query).unwrap() else {
            todo!()
        };
        assert_eq!(row, Row::new(1, username.as_str(), email.as_str()));
        assert_ne!(row.username.len(), username.len());
        assert_eq!(row.username.len(), COLUMN_USERNAME_SIZE);
        assert_ne!(row.email.len(), email.len());
        assert_eq!(row.email.len(), COLUMN_EMAIL_SIZE);
    }

    #[test]
    fn rows() {
        let row = Row::new(1, "foo", "bar");
        assert_eq!(row.id, 1);
        assert_eq!(row.username, pad_string("foo", COLUMN_USERNAME_SIZE));
        assert_eq!(row.email, pad_string("bar", COLUMN_EMAIL_SIZE));
    }

    #[test]
    fn table() {
        let mut table = Table::new();
        let _ = table.insert_row(&Row::new(1, "foo", "bar"));
        let _ = table.insert_row(&Row::new(2, "foo", "bar"));
        let rows = table.select_row();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn fill_table() {
        let mut table = Table::new();
        let page_capacity = (PAGE_SIZE / table.row_size) as u32;
        let max = page_capacity * TABLE_MAX_PAGES as u32;
        for i in 0..max {
            let res = table.insert_row(&Row::new(i, "foo", "bar"));
            assert!(res.is_ok());
        }
        let res = table.insert_row(&Row::new(max + 1, "foo", "bar"));
        assert_eq!(res, Err("Table is full"));
    }
}
