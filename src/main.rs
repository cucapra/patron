// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>

use clap::{arg, Parser};
use patronus::*;

#[derive(Parser, Debug)]
#[command(name = "patron")]
#[command(author = "Kevin Laeufer <laeufer@cornell.edu>")]
#[command(version)]
#[command(about = "Tries to find a witness that shows how to get to a bad state.", long_about = None)]
struct Args {
    #[arg(short, long)]
    verbose: bool,
    #[arg(value_name = "BTOR2", index = 1)]
    filename: String,
}

fn main() {
    let args = Args::parse();
    let (ctx, sys) = btor2::parse_file(&args.filename).expect("Failed to load btor2 file!");
}
