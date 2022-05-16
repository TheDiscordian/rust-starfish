use starfish::*;
use std::{fs, time, thread};
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    /// Path to *><> script
    #[clap()]
    path: String,

    /// Initial stack
    #[clap(short = 's', long, multiple_values(true))]
    stack: Option<Vec<f64>>,

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
    let mut codebox = CodeBox::new(&fs::read_to_string(args.path).unwrap(), args.stack, false);

    let mut end = false;
    let mut output: Option<String>;
    
    while !end {
        if args.output_codebox {
            codebox.print(false);
        }
        if args.output_stack {
            codebox.print_stack();
        }

        (output, end) = codebox.swim();
        match output {
            Some(val) => print!("{}", val),
            None => {},
        }

        if args.delay > 0 {
            thread::sleep(time::Duration::from_millis(args.delay));
        }
    }
}
