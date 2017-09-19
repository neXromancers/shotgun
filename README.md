# shotgun

A minimal screenshot utility for X11. Shotgun was written to replace
[maim](https://github.com/naelstrof/maim) in my workflow.

Features:
- Exports PNG screenshots to file or stdout
- Masks off-screen areas on multi-head setups
- Supports selections by window ID and geometry
- *On average, shotgun is more than twice as fast as maim*

## Usage

```
Usage: shotgun [options] [file]

Options:
    -i, --id ID         Window to capture
    -g, --geometry WxH+X+Y
                        Area to capture
    -h, --help          Print help and exit
```
## shotgun vs maim

- Only PNG is supported
- Does not attempt to wrap slop
- No cursor blending
- Properly detects that stdout is a TTY, and defaults to a time-stamped file
  instead of dumping raw PNG data into your terminal (unless `-` is specified as
  the output file name)
- Most command-line flags were omitted
- The XShape extension and `_NET_FRAME_EXTENTS` are not supported
- shotgun is written in Rust, maim in C++
- The code base is kept as small and simple as possible (as much as Rust
  permits)

There are several reasons for omitting these features:
- Features that can be replaced trivially by external programs and wrapper
  scripts:
  - Use ImageMagick's `convert` for JPEG output
  - slop output is easy to process in a shell script
  - Use `sleep` instead of `-d`, since slop has to be called separately, this
    flag is not necessary
  - `-x` shouldn't even exist in the first place, set `$DISPLAY` instead
- I never use cursor blending, and I know that most users do not actually care
  for it
- `-w` is difficult to use in scripts, rarely useful, and can be replaced by the
  calling script
- XShape is rarely used, and the proper fix for Mutter is not to use GNOME 3 in
  the first place

Please note that I do not hold anything against maim nor its author, this is
simply my rationale for writing my own replacement.

## Performance

I've claimed that shotgun is twice as fast as maim, here's some supporting
evidence:

```
streetwalrus@Akatsuki:~/source/shotgun(master)
>>> xrandr --fb 3840x2160
streetwalrus@Akatsuki:~/source/shotgun(master)
>>> for i in {1..10}; do time maim > /dev/null; done
maim > /dev/null  0.78s user 0.00s system 107% cpu 0.731 total
maim > /dev/null  0.79s user 0.01s system 104% cpu 0.764 total
maim > /dev/null  0.75s user 0.02s system 104% cpu 0.727 total
maim > /dev/null  0.74s user 0.01s system 110% cpu 0.678 total
maim > /dev/null  0.76s user 0.01s system 108% cpu 0.717 total
maim > /dev/null  0.73s user 0.02s system 104% cpu 0.711 total
maim > /dev/null  0.74s user 0.01s system 106% cpu 0.703 total
maim > /dev/null  0.74s user 0.01s system 109% cpu 0.682 total
maim > /dev/null  0.82s user 0.02s system 104% cpu 0.799 total
maim > /dev/null  0.75s user 0.01s system 105% cpu 0.719 total
streetwalrus@Akatsuki:~/source/shotgun(master)
>>> for i in {1..10}; do time ./target/release/shotgun > /dev/null; done
./target/release/shotgun > /dev/null  0.31s user 0.01s system 99% cpu 0.320 total
./target/release/shotgun > /dev/null  0.33s user 0.01s system 108% cpu 0.311 total
./target/release/shotgun > /dev/null  0.35s user 0.01s system 109% cpu 0.322 total
./target/release/shotgun > /dev/null  0.35s user 0.01s system 111% cpu 0.327 total
./target/release/shotgun > /dev/null  0.31s user 0.01s system 107% cpu 0.296 total
./target/release/shotgun > /dev/null  0.32s user 0.01s system 109% cpu 0.302 total
./target/release/shotgun > /dev/null  0.36s user 0.00s system 105% cpu 0.338 total
./target/release/shotgun > /dev/null  0.32s user 0.01s system 102% cpu 0.322 total
./target/release/shotgun > /dev/null  0.34s user 0.00s system 105% cpu 0.318 total
./target/release/shotgun > /dev/null  0.33s user 0.01s system 111% cpu 0.303 total
```

Further profiling has shown that the bottleneck in shotgun lies fully within the
PNG encoder.

## Installation

- Manual: Make sure you have a recent Rust toolchain. Clone this repo, then run
  `cargo install`.
- Arch Linux: [AUR package](https://aur.archlinux.org/packages/shotgun/)
- Other distros: make a pull request to add your package or build script!
