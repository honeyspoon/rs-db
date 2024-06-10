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

    fn get_page_for_row(&self, row_id: u32) -> &Page {
        let page_id = row_id as usize / TABLE_MAX_PAGES;

        self.pages[page_id].as_ref().unwrap()
    }

    fn get_page_for_row_mut(&mut self, row_id: u32) -> &mut Page {
        let page_id = row_id as usize / TABLE_MAX_PAGES;
        let rows_per_page: usize = PAGE_SIZE / self.row_size;

        if self.pages[page_id].is_none() {
            let capacity = rows_per_page * self.row_size;
            self.pages[page_id] = Some(vec![0; capacity]);
        }

        self.pages[page_id].as_mut().unwrap()
    }

    fn insert_row(&mut self, row: &Row) {
        let row_offset = (row.id as usize % TABLE_MAX_PAGES) * self.row_size;
        let row_bytes = bincode::serialize(&row).unwrap();

        {
            let page = self.get_page_for_row_mut(row.id);
            page[row_offset..row_offset + row_bytes.len()].copy_from_slice(&row_bytes);
        }

        self.nb_rows += 1;
    }

    fn select_row(&self) {
        for row_id in 0..self.nb_rows {
            let page = self.get_page_for_row(row_id);
            let row_offset = (row_id as usize % TABLE_MAX_PAGES) * self.row_size;
            let slice_end = row_offset + self.row_size;
            let slice = &page[row_offset..slice_end];
            let decoded: Row = bincode::deserialize(slice).unwrap();
            println!("{:?}", decoded);
        }
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
            table.select_row();
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
