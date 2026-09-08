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
    Add { inputs: Vec<String> },
    All,
    Get { input: Vec<String> },
}

const WAL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/wal.log");

async fn handle(stream: Async<TcpStream>) -> io::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();

    let mut writer = &stream;
    let mut response: String;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break;
        }

        let command = &line.trim_end();
        let parts = command.split_whitespace();

        let args = std::iter::once("rusty-keys").chain(parts);

        match Args::try_parse_from(args) {
            Ok(parsed) => match parsed.command {
                Command::Add { inputs } => {
                    if inputs.len() < 2 {
                        response = format!("ADD called with not enough arguments!");
                    } else {
                        let (first, rest) = inputs.split_first().unwrap();
                        let result = add(first.to_string(), rest.join(" "), Some(WAL_PATH));

                        response = match result {
                          Ok(v) => format!("ADD {:?}", v),
                          Err(e) => format!("Error: {:?}", e)
                        }
                    }
                }
                Command::All => {
                    let contents = all(Some(WAL_PATH));
                    response = contents.unwrap();
                }
                Command::Get { input } => {
                    if input.len() > 1 {
                        response = format!("GET called with too many arguments!");
                    } else {
                        let key = input.clone().remove(0);
                        let result = read_from_memory(key, Some(WAL_PATH));

                        response = match result {
                          Ok(v) => v.1,
                          Err(e) => format!("Error for GET: {:?}", e)
                        }
                    }
                }
            },
            Err(e) => {
                writer
                    .write_all(format!("Error: {}\n", e).as_bytes())
                    .await?;
                continue;
            }
        }

        writer
            .write_all(format!("{}\n", response).as_bytes())
            .await?;
    }

    Ok(())
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

// hint files used for merge, compaction, speedy access
