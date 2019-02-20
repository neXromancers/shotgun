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

To use with slop (as a replacement for `maim -s`):
```bash
#!/usr/bin/env bash

set -e

sel=$(slop -f "-i %i -g %g")
shotgun $sel $*
```

## shotgun vs maim

- Only PNG and [PAM](#going-faster) are supported
- Does not attempt to wrap slop
- No cursor blending
- Defaults to a time-stamped file instead of dumping raw PNG data into your
  terminal (use `-` as the file name if you want to pipe output to something
  else)
- Most command-line flags were omitted
- The XShape extension is not supported
- shotgun is written in Rust, maim in C++
- The code base is kept as small and simple as possible (as much as Rust
  permits)

There are several reasons for omitting these features:
- Features that can be replaced trivially by external programs and wrapper
  scripts:
  - Use ImageMagick's `convert` and [shotgun's `-f pam`](#going-faster) for JPEG output
  - slop output is easy to process in a shell script
  - Use `sleep` instead of `-d`, since slop has to be called separately, this
    flag is not necessary
  - `-x` shouldn't even exist in the first place, set `$DISPLAY` instead
- I never use cursor blending, and I know that most users do not actually care
  for it
- `-w` (geometry relative to another window) is difficult to use and hardly
  useful, instead, shotgun always interprets the input geometry relative to the
  root window (maim's default is the captured window itself)
- There is rarely a reason to take a screenshot of an XShape window, most of
  them are special like slop's selection window or keynav's crosshair.
  Supporting XShape properly could add a significant amount of overhead, both in
  code length and performance, which are not desirable.

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

### Going faster

The PNG encoder bottleneck can be avoided by using `-f pam`. This sets the output format to
[Netpbm PAM](https://en.wikipedia.org/wiki/Netpbm#PAM_graphics_format) - an uncompressed binary image format.

By using an uncompressed format both encoding and decoding performance is improved:

#### Encoding

```
>>> for i in {1..5}; do time ./target/release/shotgun -f png - > /dev/null; done
./target/release/shotgun -f png - > /dev/null  0.32s user 0.11s system 99% cpu 0.434 total
./target/release/shotgun -f png - > /dev/null  0.31s user 0.05s system 99% cpu 0.369 total
./target/release/shotgun -f png - > /dev/null  0.32s user 0.06s system 99% cpu 0.382 total
./target/release/shotgun -f png - > /dev/null  0.31s user 0.06s system 99% cpu 0.369 total
./target/release/shotgun -f png - > /dev/null  0.26s user 0.08s system 99% cpu 0.343 total

>>> for i in {1..5}; do time ./target/release/shotgun -f pam - > /dev/null; done
./target/release/shotgun -f pam - > /dev/null  0.09s user 0.12s system 98% cpu 0.210 total
./target/release/shotgun -f pam - > /dev/null  0.07s user 0.07s system 99% cpu 0.141 total
./target/release/shotgun -f pam - > /dev/null  0.10s user 0.05s system 99% cpu 0.148 total
./target/release/shotgun -f pam - > /dev/null  0.08s user 0.08s system 99% cpu 0.152 total
./target/release/shotgun -f pam - > /dev/null  0.08s user 0.07s system 99% cpu 0.148 total
```

#### Decoding (using ImageMagick to convert to jpg)

```
>>> for i in {1..5}; do time ./target/release/shotgun -f png - | convert - jpg:- > /dev/null; done
convert - jpg:- > /dev/null  0.59s user 0.16s system 89% cpu 0.842 total
convert - jpg:- > /dev/null  0.58s user 0.16s system 95% cpu 0.763 total
convert - jpg:- > /dev/null  0.55s user 0.20s system 97% cpu 0.764 total
convert - jpg:- > /dev/null  0.53s user 0.15s system 89% cpu 0.758 total
convert - jpg:- > /dev/null  0.61s user 0.16s system 100% cpu 0.762 total

>>> for i in {1..5}; do time ./target/release/shotgun -f pam - | convert - jpg:- > /dev/null; done
convert - jpg:- > /dev/null  0.24s user 0.11s system 63% cpu 0.557 total
convert - jpg:- > /dev/null  0.23s user 0.09s system 70% cpu 0.449 total
convert - jpg:- > /dev/null  0.22s user 0.10s system 66% cpu 0.490 total
convert - jpg:- > /dev/null  0.21s user 0.11s system 72% cpu 0.434 total
convert - jpg:- > /dev/null  0.22s user 0.10s system 69% cpu 0.459 total
```

## Installation

- Manual: Make sure you have a recent Rust toolchain. Clone this repo, then run
  `cargo install`.
- [crates.io](https://crates.io/crates/shotgun): `cargo install shotgun`
- [Arch Linux](https://www.archlinux.org/packages/?name=shotgun): `pacman -S shotgun`
- Other distros: make a pull request to add your package or build script!
