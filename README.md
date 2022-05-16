go-starfish
======

A [\*><>](https://esolangs.org/wiki/Starfish)  interpreter written in Go. \*><> is a language derived from [><>](http://esolangs.org/wiki/Fish).

Building
---------------

Ensure the [Rust toolchain](https://www.rust-lang.org/tools/install) is installed. Then do the following:

```shell
git clone https://github.com/TheDiscordian/rust-starfish
cd rust-starfish
cargo build
```

Usage
---------------

```
$ starfish -h            
starfish 1.0.0
*><> is a stack-based, reflective, two-dimensional esoteric programming language based directly off
of ><>.

USAGE:
    starfish [OPTIONS] <PATH>

ARGS:
    <PATH>    Path to *><> script

OPTIONS:
    -c, --output-codebox      Output codebox each tick
    -d, --delay <DELAY>       Delay between each tick in milliseconds [default: 0]
    -h, --help                Print help information
    -s, --stack <STACK>...    Initial stack
    -S, --output-stack        Output stack each tick
    -V, --version             Print version information
```