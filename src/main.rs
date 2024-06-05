use std::io;
use std::io::Write;

fn printhelp() {
    println!("help");
}

fn main() -> io::Result<()> {
    println!("~ rsdb");

    let mut buffer = String::new();

    let mut exit = false;
    while !exit {
        print!("> ");
        let _ = io::stdout().flush();
        buffer.clear();
        io::stdin().read_line(&mut buffer)?;

        match buffer.trim() {
            ".exit" => exit = true,
            ".help" => printhelp(),
            _ => println!("unknown command"),
        }
    }

    println!("bye");
    Ok(())
}
