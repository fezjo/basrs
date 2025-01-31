# BasRs

Rewrite of the [Bass](https://github.com/edc/bass/) script in Rust.

I want to have single `.profile` for all shells. Since Fish has different syntax, I wanted to source this config using Bass. However, this slowed down each shell startup significantly (~150ms). With BasRs, the startup time is reduced to ~20ms, which is acceptable.

Original introduction from Bass:

> Bass makes it easy to use utilities written for Bash in [fish shell](https://github.com/fish-shell/fish-shell/).

> Regular bash scripts can be used in fish shell just as scripts written in any language with proper shebang or explicitly using the interpreter (i.e. using `bash script.sh`). However, many utilities, such as virtualenv, modify the shell environment and need to be sourced, and therefore cannot be used in fish. Sometimes, counterparts (such as the excellent [virtualfish](http://virtualfish.readthedocs.org/en/latest/)) are created, but that's often not the case.

> Bass is created to make it possible to use bash utilities in fish shell without any modification. It works by capturing what environment variables are modified by the utility of interest, and replay the changes in fish.

> You might not need Bass for simple use cases. A great simple alternative (suggested by @jorgebucaran) is to just use `exec bash -c "source some-bash-setup.sh; exec fish"`.

## Changes from Bass

- Bass keeps the stdout and stderr of the sourced script. BasRs does not.
- Bass would automatically source all the changes, while BasRs will only print the changes (and is thus missing the `-d` _debug mode_).
- We also output what functions have been added or removed. (The ability to detect function definition changes or adding these functions to fish is not implemented yet.)

These changes are subject to change.

## Installation

```sh
cargo install basrs
```

## Usage

```sh
# if we want to source .profile
basrs source ~/.profile | source
```