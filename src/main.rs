use clap::{Subcommand, Parser};
use rusty_keys::{add, all, read_from_memory};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Add { text: Vec<String> },
    All,
    Get {key: String }
}

fn main()-> Result<(), anyhow::Error> {
  let args = Args::parse();

  match args.command {
    Command::Add { text } => {
      let text_collection: Vec<String> = text.into_iter().clone().collect();

      if text_collection.len() == 1 {
        print!("no")
      } else if text_collection.len() > 2 {
        print!("no")
      } else {
        let result = add(text_collection[0].clone(), text_collection[1].clone(), None::<PathBuf>);
        println!("{:?}", result)
      }
    },
    Command::All => {
      let result = all(None::<PathBuf>)?;
      println!("{:?},", result)
    },
    Command::Get { key} => {
      let result = read_from_memory(key, None::<PathBuf>);
      println!("{:?}", result)
    }
  }
  Ok(())
}


// hint files used for merge, compaction, speedy access