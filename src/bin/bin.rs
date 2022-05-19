use clap::Parser;
use starfish::*;
use std::{fs, thread, time};

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    /// Path to *><> script
    #[clap()]
    path: String,

    /// Initial stack (example: --stack "10 'olleh'")
    #[clap(short = 's', long)]
    stack: Option<String>,

    /// Output stack each tick
    #[clap(short = 'S', long = "output-stack")]
    output_stack: bool,

    /// Output codebox each tick
    #[clap(short = 'c', long = "output-codebox")]
    output_codebox: bool,

    /// Delay between each tick in milliseconds
    #[clap(short = 'd', long = "delay", default_value_t = 0)]
    delay: u64,
}

pub fn main() {
    let args = Args::parse();
    let stack: Stack;
    match args.stack {
        None => stack = Stack::new(None),
        Some(v) => stack = Stack::from_string(&v).unwrap(),
    }
    let mut codebox = CodeBox::new(&fs::read_to_string(args.path).unwrap(), stack, false);

    let mut end = false;
    let mut output: Option<String>;
    let mut sleep_ms: f64;

    while !end {
        if args.output_codebox {
            codebox.print(false);
        }
        if args.output_stack {
            println!("Stack: {}", codebox.string_stack());
        }

        (output, end, sleep_ms) = codebox.swim();
        match output {
            Some(val) => print!("{}", val),
            None => {}
        }

        if sleep_ms > 0.0 {
            thread::sleep(time::Duration::from_millis(sleep_ms as u64));
        }
        if args.delay > 0 {
            thread::sleep(time::Duration::from_millis(args.delay));
        }
    }
}
