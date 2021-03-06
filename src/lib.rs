// Spec: https://esolangs.org/wiki/Starfish
use std::error::Error;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::io::{stdout, Write};
use std::process;
#[cfg(target_arch = "wasm32")]
use std::sync::mpsc::Sender;
use std::sync::mpsc::{channel, Receiver};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
use std::{char, panic, str};

use chrono::prelude::*;
use rand::Rng;

fn crash() {
    println!("something smells fishy...");
    process::exit(1);
}

#[derive(PartialEq)]
enum Direction {
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

    pub fn from_string(str_stack: &str) -> Result<Stack, Box<dyn Error>> {
        let mut s: Vec<f64> = Vec::new();
        let mut str_mode: u8 = 0;
        let mut cur_str = String::new();

        for b in str_stack.bytes() {
            if str_mode != 0 && b != str_mode {
                s.push(b as f64);
                continue;
            }
            match b {
                b' ' => {
                    if cur_str.len() > 0 {
                        let f: f64 = cur_str.parse()?;
                        cur_str = String::new();
                        s.push(f);
                    }
                }
                b'\'' | b'"' => {
                    if str_mode == 0 {
                        str_mode = b;
                    } else {
                        str_mode = 0;
                    }
                }
                b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' | b'.' => {
                    cur_str.push(b as char);
                }
                _ => return Err("Invalid initial stack")?,
            }
        }

        if cur_str.len() > 0 {
            let f: f64 = cur_str.parse()?;
            s.push(f);
        }

        return Ok(Stack::new(Some(s)));
    }

    /// output information about the stack
    pub fn to_string(&self) -> String {
        return format!("{:?}", self.s);
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
        self.s.push(self.s[self.s.len() - 1]);
    }

    /// reverse implements "r".
    pub fn reverse(&mut self) {
        self.s.reverse();
    }

    /// swap_two implements "$".
    pub fn swap_two(&mut self) {
        let len = self.s.len();
        self.s.swap(len - 2, len - 1);
    }

    /// swap_three implements "@": with [1,2,3,4], calling "@" results in [1,4,2,3].
    pub fn swap_three(&mut self) {
        // Is there a better way to do this?
        let len = self.s.len();
        let end = self.s[len - 1];
        self.s[len - 1] = self.s[len - 2];
        self.s[len - 2] = self.s[len - 3];
        self.s[len - 3] = end;
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
        let vals = self.s.drain(len - count..len).as_slice().to_vec();
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
    #[cfg(not(target_arch = "wasm32"))]
    file: Option<File>,
    #[cfg(not(target_arch = "wasm32"))]
    file_path: String,
    stdin_out: Receiver<u8>,
    #[cfg(target_arch = "wasm32")]
    stdin_in: Sender<u8>,
}

impl CodeBox {
    /// new returns a new CodeBox. "script" should be a complete *><> script, "stack" should
    /// be the initial stack, and compatibility_mode should be set if old fishinterpreter.com behaviour is needed.
    pub fn new(script: &str, stack: Stack, compatibility_mode: bool) -> CodeBox {
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
        stacks.push(stack);

        let (stdin_in, stdin_out) = channel();
        #[cfg(not(target_arch = "wasm32"))]
        {
            thread::spawn(move || {
                let mut stdin = io::stdin();

                loop {
                    let mut bs: [u8; 1] = [0];
                    let read_res = stdin.read(&mut bs);
                    match read_res {
                        Ok(v) => {
                            if v > 0 {
                                _ = stdin_in.send(bs[0]);
                            }
                        }
                        Err(_e) => {
                            return;
                        }
                    }
                }
            });
        }

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
            #[cfg(not(target_arch = "wasm32"))]
            file: None,
            #[cfg(not(target_arch = "wasm32"))]
            file_path: String::new(),
            stdin_out: stdin_out,
            #[cfg(target_arch = "wasm32")]
            stdin_in: stdin_in,
        }
    }

    #[cfg(target_arch = "wasm32")]
    /// inject_input acts like stdin, inserting whatever Vec<u8> is passed
    pub fn inject_input(&self, inp: Vec<u8>) {
        for i in inp {
            _ = self.stdin_in.send(i);
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
            }
            Direction::Down => {
                self.f_y += 1;
                if self.f_y >= self.height {
                    self.f_y = 0;
                }
            }
            Direction::Left => {
                if self.f_x > 0 {
                    self.f_x -= 1;
                } else {
                    self.f_x = self.width - 1;
                }
            }
            Direction::Up => {
                if self.f_y > 0 {
                    self.f_y -= 1;
                } else {
                    self.f_y = self.height - 1;
                }
            }
        }
    }

    /// exe executes the instruction the ><> is currently on top of. It returns the string it intends to output (None if none) and true when it executes ";".
    /// It also returns the time it should sleep for.
    pub fn exe(&mut self, r: u8) -> (Option<String>, bool, f64) {
        match r {
            b' ' => return (None, false, 0.0),
            b'>' => {
                self.f_dir = Direction::Right;
                self.was_left = false;
                return (None, false, 0.0);
            }
            b'v' => {
                self.f_dir = Direction::Down;
                return (None, false, 0.0);
            }
            b'<' => {
                self.f_dir = Direction::Left;
                self.was_left = true;
                return (None, false, 0.0);
            }
            b'^' => {
                self.f_dir = Direction::Up;
                return (None, false, 0.0);
            }
            b'|' => {
                if self.f_dir == Direction::Right {
                    self.f_dir = Direction::Left;
                    self.was_left = true;
                } else if self.f_dir == Direction::Left {
                    self.f_dir = Direction::Right;
                    self.was_left = false;
                }
                return (None, false, 0.0);
            }
            b'_' => {
                if self.f_dir == Direction::Down {
                    self.f_dir = Direction::Up;
                } else if self.f_dir == Direction::Up {
                    self.f_dir = Direction::Down;
                }
                return (None, false, 0.0);
            }
            b'#' => {
                match self.f_dir {
                    Direction::Right => {
                        self.f_dir = Direction::Left;
                        self.was_left = true;
                    }
                    Direction::Down => self.f_dir = Direction::Up,
                    Direction::Left => {
                        self.f_dir = Direction::Right;
                        self.was_left = false;
                    }
                    Direction::Up => self.f_dir = Direction::Down,
                }
                return (None, false, 0.0);
            }
            b'/' => {
                match self.f_dir {
                    Direction::Right => self.f_dir = Direction::Up,
                    Direction::Down => {
                        self.f_dir = Direction::Left;
                        self.was_left = true;
                    }
                    Direction::Left => self.f_dir = Direction::Down,
                    Direction::Up => {
                        self.f_dir = Direction::Right;
                        self.was_left = false;
                    }
                }
                return (None, false, 0.0);
            }
            b'\\' => {
                match self.f_dir {
                    Direction::Right => self.f_dir = Direction::Down,
                    Direction::Down => {
                        self.f_dir = Direction::Right;
                        self.was_left = false;
                    }
                    Direction::Left => self.f_dir = Direction::Up,
                    Direction::Up => {
                        self.f_dir = Direction::Left;
                        self.was_left = true;
                    }
                }
                return (None, false, 0.0);
            }
            b'x' => {
                self.f_dir = Direction::from_i32(rand::thread_rng().gen_range(0..4));
                if self.f_dir == Direction::Right {
                    self.was_left = false;
                } else {
                    self.was_left = true;
                }
                return (None, false, 0.0);
            }
            // *><> commands
            b'O' => {
                self.deep_sea = false;
                return (None, false, 0.0);
            }
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
                return (None, false, 0.0);
            }
            _ => {}
        }

        if self.deep_sea {
            return (None, false, 0.0);
        }

        let mut output = None;

        match r {
            b';' => return (None, true, 0.0),
            b'"' | b'\'' => {
                if self.string_mode == 0 {
                    self.string_mode = r;
                } else if self.string_mode == r {
                    self.string_mode = 0;
                }
            }
            b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' => {
                self.push((r - b'0') as f64)
            }
            b'a' | b'b' | b'c' | b'd' | b'e' | b'f' => self.push((r - b'a' + 10) as f64),
            b'&' => self.register(),
            b'o' => output = Some(char::from_u32(self.pop() as u32).unwrap().to_string()),
            b'n' => output = Some((self.pop() as i64).to_string()),
            b'r' => self.reverse_stack(),
            b'+' => {
                let a = self.pop();
                let res = self.pop() + a;
                self.push(res);
            }
            b'-' => {
                let a = self.pop();
                let res = self.pop() - a;
                self.push(res);
            }
            b'*' => {
                let a = self.pop();
                let res = self.pop() * a;
                self.push(res);
            }
            b',' => {
                let a = self.pop();
                let res = self.pop() / a;
                self.push(res);
            }
            b'%' => {
                let a = self.pop();
                let res = self.pop().rem_euclid(a);
                self.push(res);
            }
            b'=' => {
                let a = self.pop();
                if self.pop() == a {
                    self.push(1.0);
                } else {
                    self.push(0.0);
                }
            }
            b')' => {
                let a = self.pop();
                if self.pop() > a {
                    self.push(1.0);
                } else {
                    self.push(0.0);
                }
            }
            b'(' => {
                let a = self.pop();
                if self.pop() < a {
                    self.push(1.0);
                } else {
                    self.push(0.0);
                }
            }
            b'!' => self.shift(),
            b'?' => {
                if self.pop() == 0.0 {
                    self.shift();
                }
            }
            b'.' => {
                self.f_y = self.pop() as usize;
                self.f_x = self.pop() as usize;
            }
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
            }
            b'l' => self.stack_length(),
            b'g' => {
                let y = self.pop() as usize;
                let x = self.pop() as usize;
                self.push(self.code_box[y][x] as f64);
            }
            b'p' => {
                let y = self.pop() as usize;
                let x = self.pop() as usize;
                let val = self.pop() as u8;
                self.code_box[y][x] = val;
            }
            b'i' => {
                let mut r = -1.0;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    match &self.file {
                        None => match self.stdin_out.try_recv() {
                            Ok(v) => r = v as f64,
                            Err(_e) => {}
                        },
                        Some(_inner) => {
                            let mut bs = [0];
                            let read_res = self.file.as_ref().unwrap().read(&mut bs);
                            match read_res {
                                Ok(v) => {
                                    if v > 0 {
                                        r = bs[0] as f64;
                                    }
                                }
                                Err(_e) => {}
                            }
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    match self.stdin_out.try_recv() {
                        Ok(v) => r = v as f64,
                        Err(_e) => {}
                    }
                }
                self.push(r);
            }
            // *><> commands
            b'h' => self.push(Local::now().hour() as f64),
            b'm' => self.push(Local::now().minute() as f64),
            b's' => self.push(Local::now().second() as f64),
            b'S' => {
                _ = stdout().flush();
                return (output, false, self.pop() * 100.0);
            }
            b'u' => self.deep_sea = true,
            b'F' => {
                #[cfg(target_arch = "wasm32")]
                return (Some(String::from("\nsomething smells fishy...")), true, 0.0);
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let count = self.pop() as usize;
                    let vals = self.stacks[self.p].get_bytes(count);
                    match &self.file {
                        Some(_inner) => {
                            self.file = None;
                            let mut file = File::create(&self.file_path).unwrap();
                            _ = file.write_all(&vals);
                        }
                        None => {
                            self.file_path = str::from_utf8(&vals).unwrap().to_string();
                            let file_res = File::open(&self.file_path);
                            match file_res {
                                Ok(v) => {
                                    self.file = Some(v);
                                }
                                Err(_e) => {
                                    self.file = Some(File::create(&self.file_path).unwrap());
                                    self.file = Some(File::open(&self.file_path).unwrap());
                                }
                            }
                        }
                    }
                }
            }
            b'C' => self.call(),
            b'R' => self.ret(),
            b'I' => self.p += 1,
            b'D' => self.p -= 1,
            _ => return (Some(String::from("\nsomething smells fishy...")), true, 0.0),
        }

        return (output, false, 0.0);
    }

    /// swim causes the ><> to execute an instruction, then move. It returns a string of non-zero length when it has output and true when it encounters ";".
    pub fn swim(&mut self) -> (Option<String>, bool, f64) {
        let y = self.f_y;
        let x = self.f_x;
        let r = self.code_box[y][x];
        let string_mode = self.string_mode != 0;

        let mut output = None;
        let mut end = false;
        let mut sleep_ms: f64 = 0.0;

        if string_mode && r != self.string_mode {
            self.push(r as f64);
        } else {
            (output, end, sleep_ms) = self.exe(r);
        }
        self.shift();
        return (output, end, sleep_ms);
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
        let vals = self.stacks[self.p]
            .s
            .drain(len - n..len)
            .as_slice()
            .to_vec();
        self.p += 1;
        self.stacks.insert(self.p, Stack::new(Some(vals)));
        if self.compatibility_mode {
            self.stacks[self.p].reverse(); // This is done to match the old fishlanguage.com interpreter.
        }
    }

    /// call implements "C".
    pub fn call(&mut self) {
        self.stacks.insert(
            self.p,
            Stack::new(Some(vec![self.f_x as f64, self.f_y as f64])),
        );
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

    /// string_stack returns a copy of the current stack as a string.
    pub fn string_stack(&self) -> String {
        self.stacks[self.p].to_string()
    }

    /// size returns the width and height of the codebox.
    pub fn size(&self) -> (usize, usize) {
        return (self.width, self.height);
    }

    /// code_box outputs a copy of the current state of the codebox.
    pub fn code_box(&self) -> Vec<Vec<u8>> {
        return self.code_box.to_vec();
    }

    /// deep_sea returns if the ><> is in deepsea mode or not.
    pub fn deep_sea(&self) -> bool {
        return self.deep_sea;
    }

    /// position returns the x/y coordinates of the ><>.
    pub fn position(&self) -> (usize, usize) {
        return (self.f_x, self.f_y);
    }
}
