use std::cmp::max;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};

use log::info;

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};

const TABLE_MAX_PAGES: usize = 100;
const PAGE_SIZE: usize = 4096;

#[derive(Clone)]
struct Page {
    bytes: [u8; PAGE_SIZE],
    end_offset: usize,
}

impl Page {
    fn new(data: [u8; PAGE_SIZE]) -> Self {
        Self {
            bytes: data,
            end_offset: 0,
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if data.len() + offset > PAGE_SIZE {
            Err("not enough space to write")
        } else {
            self.bytes[offset..offset + data.len()].copy_from_slice(data);
            self.end_offset = max(offset + data.len(), self.end_offset);
            Ok(())
        }
    }
}

trait RW: Read + Write + Seek {}
impl<T: Read + Write + Seek> RW for T {}

struct Pager {
    file: Box<dyn RW>,
    file_size: usize,
    pages: Vec<Option<Page>>,
}

impl Pager {
    fn new(mut file: Box<dyn RW>) -> Self {
        let file_size = file.seek(SeekFrom::End(0)).unwrap() as usize;
        file.seek(SeekFrom::Start(0)).unwrap();
        Self {
            file,
            file_size,
            pages: vec![None; TABLE_MAX_PAGES],
        }
    }

    fn flush(&mut self, page_id: usize) {
        let offset = (PAGE_SIZE * page_id) as u64;

        if let Some(page) = self.pages[page_id].as_mut() {
            let _ = self.file.seek(SeekFrom::Start(offset));
            let _ = self.file.write(&page.bytes[0..page.end_offset]);
        }
    }

    fn get_nb_pages(&self) -> (usize, usize) {
        let mut num_pages = self.file_size / PAGE_SIZE;
        let rest = self.file_size % PAGE_SIZE;
        if rest != 0 {
            num_pages += 1;
        }
        (num_pages, rest)
    }

    fn load_page(&mut self, page_id: usize) -> Result<&mut Page, &'static str> {
        let (num_pages, _) = self.get_nb_pages();

        if self.pages[page_id].is_none() {
            let mut data = [0x0; PAGE_SIZE];
            if page_id + 1 < num_pages {
                // + 1 because pages are indexed from 0
                let offset = page_id as u64 * PAGE_SIZE as u64;
                let _ = self.file.seek(SeekFrom::Start(offset));
                if self.file.read(&mut data).is_err() {
                    return Err("Failed to read file");
                }
            }

            self.pages[page_id] = Some(Page::new(data));
        }

        Ok((self.pages[page_id]).as_mut().unwrap())
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        for page_id in 0..self.pages.len() {
            self.flush(page_id);
        }
    }
}

struct Table {
    pager: Pager,
    nb_rows: usize,
    row_size: usize,
}

impl Table {
    fn new(pager: Pager) -> Self {
        let row_size = bincode::serialized_size(&Row::new(0, "", "")).unwrap() as usize;
        let rows_per_page = PAGE_SIZE / row_size;

        // the assumption here is that every page expect the last one if full
        // and that the last unfilled page will ocitian full rows

        let (nb_pages, rest) = pager.get_nb_pages();
        let full_pages = max(nb_pages, 1) - 1;
        let rows = full_pages * rows_per_page;
        let nb_rows = (rows + rest) / row_size;

        Self {
            nb_rows,
            pager,
            row_size,
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
                let page_id = self.get_page_id(row.id as usize);
                let offset = self.get_row_offset(row.id as usize);
                let page: &mut Page = self.pager.load_page(page_id)?;

                match page.write(offset, &row_bytes) {
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

    fn select_row(&mut self) -> Vec<Row> {
        (0..self.nb_rows)
            .map(|row_id| {
                let page_id = self.get_page_id(row_id);
                let row_offset = self.get_row_offset(row_id);
                let page = self.pager.load_page(page_id).unwrap();
                let slice = &page.bytes[row_offset..row_offset + self.row_size];
                bincode::deserialize(slice).unwrap()
            })
            .collect()
    }
}

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

    let filename = "c.db".to_string();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(filename)
        .unwrap();
    let pager = Pager::new(Box::new(file));
    let mut table = Table::new(pager);

    loop {
        let readline = rl.readline("> ");

        match readline {
            Ok(line) => {
                if rl.add_history_entry(line.as_str()).is_ok() {
                    rl.save_history(hist_file).unwrap();
                }
                match line.chars().next() {
                    Some('.') => match parse_command(line) {
                        Ok(command) => execute_command(command),
                        Err(err) => println!("Error: {}", err),
                    },
                    Some(_) => match parse_statement(line) {
                        Ok(statement) => match execute_statment(statement, &mut table) {
                            Ok(out) => println!("{}", out),
                            Err(err) => println!("Error: {}", err),
                        },
                        Err(err) => println!("Error: {}", err),
                    },
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
    use tempfile::tempfile;

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
        let username = "0123456789123456789012345678901234".to_string();
        let email = (0..COLUMN_EMAIL_SIZE + 4).map(|_| "X").collect::<String>();
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
    fn page() {
        let mut page = Page::new([0x0; PAGE_SIZE]);
        assert_eq!(
            page.write(PAGE_SIZE, &[0x1]),
            Err("not enough space to write")
        );

        assert_eq!(page.bytes.len(), PAGE_SIZE);
        assert_eq!(page.end_offset, 0);
        assert_eq!(page.write(100, &[0x1; 10]), Ok(()));
        assert_eq!(page.end_offset, 110);
    }

    #[test]
    fn pager() {
        let data: Vec<u8> = vec![0x0; PAGE_SIZE];
        let cursor = Box::new(std::io::Cursor::new(data)) as Box<dyn RW>;
        let pager = Pager::new(Box::new(cursor));
        assert_eq!(pager.get_nb_pages(), (1, 0));

        let data: Vec<u8> = vec![0x0; PAGE_SIZE + 7];
        let cursor = Box::new(std::io::Cursor::new(data)) as Box<dyn RW>;
        let pager = Pager::new(Box::new(cursor));
        assert_eq!(pager.get_nb_pages(), (2, 7));
    }

    #[test]
    fn table() {
        let data: Vec<u8> = vec![0x0; PAGE_SIZE];
        let cursor = Box::new(std::io::Cursor::new(data)) as Box<dyn RW>;
        let mut table = Table::new(Pager::new(cursor));
        let _ = table.insert_row(&Row::new(1, "foo", "bar"));
        let _ = table.insert_row(&Row::new(2, "foo", "bar"));
        let rows = table.select_row();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn fill_table() {
        let data: Vec<u8> = vec![0x0; PAGE_SIZE];
        let cursor = Box::new(std::io::Cursor::new(data)) as Box<dyn RW>;
        let mut table = Table::new(Pager::new(cursor));
        let max = (table.get_row_per_page() * TABLE_MAX_PAGES) as u32;
        for i in 0..max {
            let res = table.insert_row(&Row::new(i, "foo", "bar"));
            assert!(res.is_ok());
        }
        let res = table.insert_row(&Row::new(max, "foo", "bar"));
        assert_eq!(res, Err("Table is full"));
    }

    #[test]
    fn persistance() {
        let tempfile = tempfile().expect("Failed to create tempfile");
        {
            let mut table = Table::new(Pager::new(Box::new(
                tempfile.try_clone().expect("Failed to clone tempfile"),
            )));

            let rows = table.select_row();
            assert_eq!(rows.len(), 0);

            let _ = table.insert_row(&Row::new(0, "foo", "bar"));
            let rows = table.select_row();
            assert_eq!(rows.len(), 1);
            drop(table);
        }
        {
            let mut table = Table::new(Pager::new(Box::new(
                tempfile.try_clone().expect("Failed to clone tempfile"),
            )));

            let rows = table.select_row();
            assert_eq!(rows.len(), 1);
        }
    }

    #[test]
    fn all() {
        let filename = "/tmp/c.db".to_string();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(filename)
            .unwrap();

        let pager = Pager::new(Box::new(file));
        let mut table = Table::new(pager);
        table.insert_row(&Row::new(0, "foo", "bar")).unwrap();
        table.insert_row(&Row::new(1, "foo", "bar")).unwrap();
        table.insert_row(&Row::new(2, "foo", "bar")).unwrap();
        let rows = table.select_row();
        for row in rows {
            println!("s - {:?}", row);
        }
    }
}
