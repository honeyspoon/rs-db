use log::info;
use std::fmt;

use serde::{Deserialize, Serialize};

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const TABLE_MAX_PAGES: usize = 100;
const PAGE_SIZE: usize = 4096;

struct Table {
    nb_rows: u32,
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
        let rows_per_page: usize = PAGE_SIZE / self.row_size;
        if self.pages[page_id].is_none() {
            let capacity = rows_per_page * self.row_size;
            self.pages[page_id] = Some(vec![0; capacity]);
        }
    }

    fn get_page_id(&self, row_id: u32) -> usize {
        row_id as usize / TABLE_MAX_PAGES
    }

    fn get_row_offset(&self, row_id: u32) -> usize {
        (row_id as usize % TABLE_MAX_PAGES) * self.row_size
    }

    fn insert_row(&mut self, row: &Row) -> Result<(), &'static str> {
        let row_offset = self.get_row_offset(row.id);
        match bincode::serialize(&row) {
            Ok(row_bytes) => {
                let page_id = self.get_page_id(row.id);
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

#[derive(Serialize, Deserialize)]
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

fn prepare_statment(buffer: String) -> Result<Statement, &'static str> {
    let parts: Vec<&str> = buffer.trim().split(' ').collect();

    match parts.as_slice() {
        ["insert", rest @ ..] => parse_insert(rest),
        ["select"] => Ok(Statement::Select),
        _ => Err("unknown command"),
    }
}

fn execute_statment(statement: Statement, table: &mut Table) -> Result<(), &'static str> {
    match statement {
        Statement::Insert(row) => {
            table.insert_row(&row);
            Ok(())
        }
        Statement::Select => {
            for row in table.select_row() {
                println!("{:?}", row);
            }
            Ok(())
        }
    }
}

fn parse_statement(line: String, table: &mut Table) -> Result<(), &'static str> {
    match prepare_statment(line) {
        Ok(statement) => execute_statment(statement, table),
        Err(err) => Err(err),
    }
}

fn print_help() {
    println!("help");
}

const EXIT_COMMAND: &str = ".exit";
const HELP_COMMAND: &str = ".help";

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

fn parse_line(line: String, table: &mut Table) -> Result<(), &'static str> {
    match line.chars().next() {
        Some('.') => parse_command(line),
        Some(_) => parse_statement(line, table),
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

    let mut table = Table::new();

    loop {
        let readline = rl.readline("> ");

        match readline {
            Ok(line) => {
                if rl.add_history_entry(line.as_str()).is_ok() {
                    rl.save_history(hist_file).unwrap();
                }

                if let Err(err) = parse_line(line, &mut table) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_rows() {
        let row = Row::new(1, "foo", "bar");
        assert_eq!(row.id, 1);
        assert_eq!(row.username, pad_string("foo", COLUMN_USERNAME_SIZE));
        assert_eq!(row.email, pad_string("bar", COLUMN_EMAIL_SIZE));
    }

    #[test]
    fn basic() {
        let mut table = Table::new();
        let _ = table.insert_row(&Row::new(1, "foo", "bar"));
        let _ = table.insert_row(&Row::new(2, "foo", "bar"));
        let rows = table.select_row();
        assert_eq!(rows.len(), 2);
    }
}
