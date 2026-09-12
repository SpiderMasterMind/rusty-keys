use rusty_keys::{add, all, read_from_memory};
use clap::{Parser, Subcommand};
use smol::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use smol::{Async, io};
use std::net::{TcpListener, TcpStream};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Add { input: Vec<String> },
    All { input: Vec<String> },
    Get { input: Vec<String> },
}

const WAL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/wal.log");

async fn handle(stream: Async<TcpStream>) -> io::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();

    let mut writer = &stream;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break;
        }

        let response = process_command(&line, Some(WAL_PATH));

        writer
            .write_all(format!("{}\n", response).as_bytes())
            .await?;
    }

    Ok(())
}

fn process_command(line: &String, wal_path: Option<&str>) -> String {
    let path = wal_path.unwrap_or(WAL_PATH);

    let mut parts = line.trim_end().split_whitespace();
    let command = parts.next().map(|w| w.to_lowercase());
    let clap_args = std::iter::once("rusty-keys".to_string())
      .chain(command)
      .chain(parts.map(|s| s.to_string()));

    match Args::try_parse_from(clap_args) {
        Ok(parsed) => match parsed.command {
            Command::Add { input } => {
                if input.len() < 2 {
                    "ADD called with not enough arguments!".to_string()
                } else {
                    let (first, rest) = input.split_first().unwrap();

                    match add(first.to_string(), rest.join(" "), Some(path)) {
                      Ok(v) => format!("ADD {:?}", v),
                      Err(e) => format!("Error: {:?}", e)
                    }
                }
            },
            Command::All { input } => {
              if input.len() > 0 {
                "ALL may only be called on its own!".to_string()
              } else {
                format!("ALL: {:?}", all(Some(path)))
              }
            },
            Command::Get { input } => match input.first() {
                None => "GET called with no key!".to_string(),
                Some(_) if input.len() > 1 => "GET called with too many arguments!".to_string(),
                Some(key) => match read_from_memory(key.clone(), Some(path)) {
                    Ok(v) => format!("{:?}", v),
                    Err(e) => format!("Error: {:?}", e),
                },
            },
        },
        Err(e) => format!("Error: {}", e),
    }
}

fn main() -> io::Result<()> {
    smol::block_on(async {
        let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 7000))?;
        println!("Listening on {}", listener.get_ref().local_addr()?);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            println!("Accepted client: {}", peer_addr);

            smol::spawn(handle(stream)).detach();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn add_call_returns_formatted_response_of_key_and_value() {
        let temp_file = NamedTempFile::new().unwrap();
        let wal_path = temp_file.path().to_str().unwrap();

        let response = process_command(&"ADD foo bar".to_string(), Some(wal_path));
        println!("{:?}", response);
        assert_eq!(response, "ADD (\"foo\", \"bar\")");
    }

    fn all_returns_everything_in_wal_log() {
    }

    fn get_key_returns_corresponding_value() {
    }

    #[test]
    fn get_with_no_key_returns_error_message() {
        let response = process_command(&"GET".to_string(), None);
        assert_eq!(response, "GET called with no key!");
    }

    #[test]
    fn get_with_too_many_args_returns_error_message() {
        let response = process_command(&"GET foo bar".to_string(), None);
        assert_eq!(response, "GET called with too many arguments!");
    }

    #[test]
    fn add_with_one_arg_returns_error_message() {
        let response = process_command(&"ADD foo".to_string(), None);
        assert_eq!(response, "ADD called with not enough arguments!");
    }

    #[test]
    fn add_alone_returns_error_message() {
        let response = process_command(&"ADD".to_string(), None);
        assert_eq!(response, "ADD called with not enough arguments!");
    }

    #[test]
    fn all_with_any_additional_args_returns_error_message() {
        let response = process_command(&"ALL foo".to_string(), None);
        assert_eq!(response, "ALL may only be called on its own!");
    }
}

// hint files used for merge, compaction, speedy access
