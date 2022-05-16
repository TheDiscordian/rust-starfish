// Spec: https://esolangs.org/wiki/Starfish
use std::{char, thread, time, str, io, panic};
use std::fs::File;
use std::sync::mpsc::{channel, Receiver};
use std::io::{Write, Read, stdout};
use std::process;

use rand::Rng;
use chrono::prelude::*;

fn crash() {
    println!("something smells fishy...");
    process::exit(1);
}

#[derive(PartialEq)]
pub enum Direction {
    Right,
    Down,
    Left,
    Up,
}

impl Direction {
    fn from_i32(val: i32) -> Direction {
        match val {
            0 => Direction::Right,
            1 => Direction::Down,
            2 => Direction::Left,
            3 => Direction::Up,
            _ => panic!("Invalid direction (valid values are 0-3): {}", val),
        }
    }
}

/// Stack is a type representing a stack in *><>. It holds the stack values in s, as well as a register. The
/// register may contain data, but will only be considered filled if filled_register is also true.
pub struct Stack {
    pub s: Vec<f64>,
    register: f64,
    filled_register: bool,
}

impl Stack {
    pub fn new(s: Option<Vec<f64>>) -> Stack {
        let new_stack: Vec<f64>;
        match s {
            Some(inner) => new_stack = inner,
            None => new_stack = Vec::new(),
        }
        Stack {
            s: new_stack,
            register: 0.0,
            filled_register: false,
        }
    }

    /// output information about the stack
    pub fn output(&self) {
        println!("stack: {:?}\nregister: {}, filled_register: {}", self.s, self.register, self.filled_register);
    }

    /// push r to the end of the stack
    pub fn push(&mut self, r: f64) {
        self.s.push(r);
    }

    /// pop a value from the end of the stack, and return it
    pub fn pop(&mut self) -> f64 {
        self.s.pop().unwrap()
    }

    /// register implements "&".
    pub fn register(&mut self) {
        if self.filled_register {
            self.s.push(self.register);
            self.filled_register = false;
        } else {
            self.register = self.s.pop().unwrap();
            self.filled_register = true;
        }
    }

    /// extend implements ":".
    pub fn extend(&mut self) {
        self.s.push(self.s[self.s.len()-1]);
    }

    /// reverse implements "r".
    pub fn reverse(&mut self)  {
        self.s.reverse();
    }

    /// swap_two implements "$".
    pub fn swap_two(&mut self) {
        let len = self.s.len();
        self.s.swap(len-2, len-1);
    }

    /// swap_three implements "@": with [1,2,3,4], calling "@" results in [1,4,2,3].
    pub fn swap_three(&mut self) { // Is there a better way to do this?
        let len = self.s.len();
        let end = self.s[len-1];
        self.s[len-1] = self.s[len-2];
        self.s[len-2] = self.s[len-3];
        self.s[len-3] = end;
    }

    /// shift_right implements "}".
    pub fn shift_right(&mut self) {
        let end = self.s.pop().unwrap();
        self.s.insert(0, end);
    }

    /// shift_left implements "{".
    pub fn shift_left(&mut self) {
        let beg = self.s[0];
        self.s.remove(0);
        self.s.push(beg);
    }

    /// get_bytes removes c values from the stack, then returns them as a byte vector.
    pub fn get_bytes(&mut self, count: usize) -> Vec<u8> {
        let len = self.s.len();
        let vals = self.s.drain(len-count..len).as_slice().to_vec();
        let mut out = vec![0; vals.len()];
        for i in 0..vals.len() {
            out[i] = vals[i] as u8;
        }
        out
    }
}

/// CodeBox is an object. It contains a *><> program complete with a stack, and is typically run in steps via CodeBox.Swim.
pub struct CodeBox {
    f_x: usize,
    f_y: usize,
    width: usize,
    height: usize,
    f_dir: Direction,
    was_left: bool,
    escaped_hook: bool,
    code_box: Vec<Vec<u8>>,
    stacks: Vec<Stack>,
    p: usize, // Used to keep track of current stack
    string_mode: u8,
    compatibility_mode: bool,
    deep_sea: bool,
    file: Option<File>,
    file_path: String,
    stdin_out: Receiver<u8>,
}

impl CodeBox {
    /// new returns a new CodeBox. "script" should be a complete *><> script, "stack" should
    /// be the initial stack, and compatibility_mode should be set if old fishinterpreter.com behaviour is needed.
    pub fn new(script: &str, stack: Option<Vec<f64>>, compatibility_mode: bool) -> CodeBox {
        let height = script.lines().count();
        let mut width = 0;
        for line in script.lines() {
            let len = line.len();
            if len > width {
                width = len;
            }
        }
        let mut lines = script.lines();
        let mut code_box = vec![vec![b' '; width]; height];
        for i in 0..code_box.len() {
            let line = lines.next().unwrap().as_bytes();
            for ii in 0..line.len() {
                code_box[i][ii] = line[ii];
            }
        }

        let mut stacks = Vec::new();
        stacks.push(Stack::new(stack));

        let (stdin_in, stdin_out) = channel();
        thread::spawn(move|| {
            let mut stdin = io::stdin();

            loop {
                let mut bs: [u8; 1] = [0];
                let read_res = stdin.read(&mut bs);
                match read_res {
                    Ok(v) => {
                        if v > 0 {
                            _ = stdin_in.send(bs[0]);
                        }
                    },
                    Err(_e) => {
                        return;
                    },
                }
            }
        });

        panic::set_hook(Box::new(|_| {
            crash();
        }));

        CodeBox {
            f_x: 0,
            f_y: 0,
            width: width,
            height: height,
            f_dir: Direction::Right,
            was_left: false,
            escaped_hook: false,
            code_box: code_box,
            stacks: stacks,
            p: 0,
            string_mode: 0,
            compatibility_mode: compatibility_mode,
            deep_sea: false,
            file: None,
            file_path: String::new(),
            stdin_out: stdin_out,
        }
    }

    /// shift changes the fish's x/y coordinates based on CodeBox.f_dir.
    pub fn shift(&mut self) {
        match &self.f_dir {
            Direction::Right => {
                self.f_x += 1;
                if self.f_x >= self.width {
                    self.f_x = 0;
                }
            },
            Direction::Down => {
                self.f_y += 1;
                if self.f_y >= self.height {
                    self.f_y = 0;
                }
            },
            Direction::Left => {
                if self.f_x > 0 {
                    self.f_x -= 1;
                } else {
                    self.f_x = self.width - 1;
                }
            },
            Direction::Up => {
                if self.f_y > 0 {
                    self.f_y -= 1;
                } else {
                    self.f_y = self.height - 1;
                }
            },
        }
    }

    /// exe executes the instruction the ><> is currently on top of. It returns the string it intends to output (None if none) and true when it executes ";".
    pub fn exe(&mut self, r: u8) -> (Option<String>, bool) {
        match r {
            b' ' => return (None, false),
            b'>' => {
                self.f_dir = Direction::Right;
                self.was_left = false;
                return (None, false);
            },
            b'v' => {
                self.f_dir = Direction::Down;
                return (None, false);
            },
            b'<' => {
                self.f_dir = Direction::Left;
                self.was_left = true;
                return (None, false);
            },
            b'^' => {
                self.f_dir = Direction::Up;
                return (None, false);
            },
            b'|' => {
                if self.f_dir == Direction::Right {
                    self.f_dir = Direction::Left;
                    self.was_left = true;
                } else if self.f_dir == Direction::Left {
                    self.f_dir = Direction::Right;
                    self.was_left = false;
                }
                return (None, false);
            },
            b'_' => {
                if self.f_dir == Direction::Down {
                    self.f_dir = Direction::Up;
                } else if self.f_dir == Direction::Up {
                    self.f_dir = Direction::Down;
                }
                return (None, false);
            },
            b'#' => {
                match self.f_dir {
                    Direction::Right => {
                        self.f_dir = Direction::Left;
                        self.was_left = true;
                    },
                    Direction::Down => self.f_dir = Direction::Up,
                    Direction::Left => {
                        self.f_dir = Direction::Right;
                        self.was_left = false;
                    },
                    Direction::Up => self.f_dir = Direction::Down,
                }
                return (None, false);
            },
            b'/' => {
                match self.f_dir {
                    Direction::Right => self.f_dir = Direction::Up,
                    Direction::Down => {
                        self.f_dir = Direction::Left;
                        self.was_left = true;
                    },
                    Direction::Left => self.f_dir = Direction::Down,
                    Direction::Up => {
                        self.f_dir = Direction::Right;
                        self.was_left = false;
                    },
                }
                return (None, false);
            },
            b'\\' => {
                match self.f_dir {
                    Direction::Right => self.f_dir = Direction::Down,
                    Direction::Down => {
                        self.f_dir = Direction::Right;
                        self.was_left = false;
                    },
                    Direction::Left => self.f_dir = Direction::Up,
                    Direction::Up => {
                        self.f_dir = Direction::Left;
                        self.was_left = true;
                    },
                }
                return (None, false);
            },
            b'x' => {
                self.f_dir = Direction::from_i32(rand::thread_rng().gen_range(0..4));
                if self.f_dir == Direction::Right {
                    self.was_left = false;
                } else {
                    self.was_left = true;
                }
                return (None, false);
            },
            // *><> commands
            b'O' => {
                self.deep_sea = false;
                return (None, false);
            },
            b'`' => {
                if self.f_dir == Direction::Down || self.f_dir == Direction::Up {
                    if self.was_left {
                        self.f_dir = Direction::Left;
                    } else {
                        self.f_dir = Direction::Right;
                    }
                } else {
                    if self.escaped_hook {
                        self.f_dir = Direction::Up;
                        self.escaped_hook = false;
                    } else {
                        self.f_dir = Direction::Down;
                        self.escaped_hook = true;
                    }
                }
                return (None, false);
            }
            _ => {}
        }

        if self.deep_sea {
            return (None, false);
        }

        let mut output = None;

        match r {
            b';' => return (None, true),
            b'"' | b'\'' => {
                if self.string_mode == 0 {
                    self.string_mode = r;
                } else if self.string_mode == r {
                    self.string_mode = 0;
                }
            },
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => self.push((r - b'0') as f64),
            b'a' | b'b' | b'c' | b'd' | b'e' | b'f' => self.push((r - b'a' + 10) as f64),
            b'&' => self.register(),
            b'o' => output = Some(char::from_u32(self.pop() as u32).unwrap().to_string()),
            b'n' => output = Some((self.pop() as u32).to_string()),
            b'r' => self.reverse_stack(),
            b'+' => {
                let a = self.pop();
                let res = self.pop() + a;
                self.push(res);
            },
            b'-' => {
                let a = self.pop();
                let res = self.pop() - a;
                self.push(res);
            },
            b'*' => {
                let a = self.pop();
                let res = self.pop() * a;
                self.push(res);
            }
            b',' => {
                let a = self.pop();
                let res = self.pop() / a;
                self.push(res);
            },
            b'%' => {
                let a = self.pop();
                let res = self.pop().rem_euclid(a);
                self.push(res);
            },
            b'=' => {
                let a = self.pop();
                if self.pop() == a {
                    self.push(1.0);
                } else {
                    self.push(0.0);
                }
            },
            b')' => {
                let a = self.pop();
                if self.pop() > a {
                    self.push(1.0);
                } else {
                    self.push(0.0);
                }
            },
            b'(' => {
                let a = self.pop();
                if self.pop() < a {
                    self.push(1.0);
                } else {
                    self.push(0.0);
                }
            },
            b'!' => self.shift(),
            b'?' => {
                if self.pop() == 0.0 {
                    self.shift();
                }
            },
            b'.' => {
                self.f_y = self.pop() as usize;
                self.f_x = self.pop() as usize;
            },
            b':' => self.extend_stack(),
            b'~' => _ = self.pop(),
            b'$' => self.stack_swap_two(),
            b'@' => self.stack_swap_three(),
            b'}' => self.stack_shift_right(),
            b'{' => self.stack_shift_left(),
            b']' => self.close_stack(),
            b'[' => {
                let size = self.pop() as usize;
                self.new_stack(size);
            },
            b'l' => self.stack_length(),
            b'g' => {
                let y = self.pop() as usize;
                let x = self.pop() as usize;
                self.push(self.code_box[y][x] as f64);
            },
            b'p' => {
                let y = self.pop() as usize;
                let x = self.pop() as usize;
                let val = self.pop() as u8;
                self.code_box[y][x] = val;
            },
            b'i' => {
                let mut r = -1.0;
                match &self.file {
                    None => {
                        match self.stdin_out.try_recv() {
                            Ok(v) => r = v as f64,
                            Err(_e) => {},
                        }
                    },
                    Some(_inner) => {
                        let mut bs = [0];
                        let read_res = self.file.as_ref().unwrap().read(&mut bs);
                        match read_res {
                            Ok(v) => {
                                if v > 0 {
                                    r = bs[0] as f64;
                                }
                            },
                            Err(_e) => {},
                        }
                    },
                }
                self.push(r);
            },
            // *><> commands
            b'h' => self.push(Local::now().hour() as f64),
            b'm' => self.push(Local::now().minute() as f64),
            b's' => self.push(Local::now().second() as f64),
            b'S' => {
                _ = stdout().flush();
                thread::sleep(time::Duration::from_millis(self.pop() as u64 * 100));
            },
            b'u' => self.deep_sea = true,
            b'F' => {
                let count = self.pop() as usize;
                let vals = self.stacks[self.p].get_bytes(count);
                match &self.file {
                    Some(_inner) => {
                        self.file = None;
                        let mut file = File::create(&self.file_path).unwrap();
                        _ = file.write_all(&vals);
                    },
                    None => {
                        self.file_path = str::from_utf8(&vals).unwrap().to_string();
                        let file_res = File::open(&self.file_path);
                        match file_res {
                            Ok(v) => {
                                self.file = Some(v);
                            },
                            Err(_e) => {
                                self.file = Some(File::create(&self.file_path).unwrap());
                                self.file = Some(File::open(&self.file_path).unwrap());
                            },
                        }
                    },
                }
            },
            b'C' => self.call(),
            b'R' => self.ret(),
            b'I' => self.p += 1,
            b'D' => self.p -= 1,
            _ => panic!("something smells fishy...{}", r)
        }

        return (output, false);
    }

    /// swim causes the ><> to execute an instruction, then move. It returns a string of non-zero length when it has output and true when it encounters ";".
    pub fn swim(&mut self) -> (Option<String>, bool) {
        let y = self.f_y;
        let x = self.f_x;
        let r = self.code_box[y][x];
        let string_mode = self.string_mode != 0;

        let mut output = None;
        let mut end = false;

        if string_mode && r != self.string_mode {
            self.push(r as f64);
        } else {
            (output, end) = self.exe(r);
        }
        self.shift();
        return (output, end);
    }

    /// push appends r to the end of the current stack.
    pub fn push(&mut self, r: f64) {
        self.stacks[self.p].push(r);
    }

    /// pop removes the value on the end of the current stack and returns it.
    pub fn pop(&mut self) -> f64 {
        self.stacks[self.p].pop()
    }

    /// stack_length implements "l" on the current stack.
    pub fn stack_length(&mut self) {
        self.push(self.stacks[self.p].s.len() as f64);
    }

    /// register implements "&" on the current stack.
    pub fn register(&mut self) {
        self.stacks[self.p].register();
    }

    /// reverse_stack implements "r" on the current stack.
    pub fn reverse_stack(&mut self) {
        self.stacks[self.p].reverse();
    }

    /// extend_stack implements ":" on the current stack.
    pub fn extend_stack(&mut self) {
        self.stacks[self.p].extend();
    }

    /// stack_swap_two implements "$" on the current stack.
    pub fn stack_swap_two(&mut self) {
        self.stacks[self.p].swap_two();
    }

    /// stack_swap_three implements "@" on the current stack.
    pub fn stack_swap_three(&mut self) {
        self.stacks[self.p].swap_three();
    }

    /// stack_shift_right implements "}" on the current stack.
    pub fn stack_shift_right(&mut self) {
        self.stacks[self.p].shift_right();
    }

    /// stack_shift_left implements "{" on the current stack.
    pub fn stack_shift_left(&mut self) {
        self.stacks[self.p].shift_left();
    }

    /// close_stack implements "]".
    pub fn close_stack(&mut self) {
        if self.compatibility_mode {
            self.stacks[self.p].reverse(); // This is done to match the old fishlanguage.com interpreter.
        }
        let mut old_stack = self.stacks[self.p].s.to_vec();
        self.stacks.remove(self.p);
        self.p -= 1;
        self.stacks[self.p].s.append(&mut old_stack);
    }

    /// new_stack implements "[".
    pub fn new_stack(&mut self, n: usize) {
        let len = self.stacks[self.p].s.len();
        let vals = self.stacks[self.p].s.drain(len-n..len).as_slice().to_vec();
        self.p += 1;
        self.stacks.insert(self.p, Stack::new(Some(vals)));        
        if self.compatibility_mode {
            self.stacks[self.p].reverse(); // This is done to match the old fishlanguage.com interpreter.
        }
    }

    /// call implements "C".
    pub fn call(&mut self) {
        self.stacks.insert(self.p, Stack::new(Some(vec![self.f_x as f64, self.f_y as f64])));
        self.p += 1;
        self.f_y = self.pop() as usize;
        self.f_x = self.pop() as usize;
    }

    /// ret implements "R".
    pub fn ret(&mut self) {
        self.p -= 1;
        self.f_y = self.pop() as usize;
        self.f_x = self.pop() as usize;
        self.stacks.remove(self.p);
    }

    /// print outputs the codebox to stdout.
    pub fn print(&self, clear: bool) {
        if clear {
            print!("\x1b[0;H");
        }
        for y in 0..self.height {
            for x in 0..self.width {
                if x == self.f_x && y == self.f_y {
                    print!("*{}*", self.code_box[y][x] as char);
                } else {
                    print!(" {} ", self.code_box[y][x] as char);
                }
            }
            println!();
        }
    }

    /// print_stack outputs the current stack to stdout.
    pub fn print_stack(&self) {
        self.stacks[self.p].output();
    }
}