# cargo-group-imports

Group imports in workspace source files as:

1. Module imports and declarations
2. Standard library
3. External crates
4. Workspace crates
5. Crate modules

For example (see also the before/after files in `test-data`):

```rust
mod module;
use module::Client;

use std::sync::Arc;

use tokio::sync::Mutex;

use other_crate::Flags;

use crate::Options;

```

This roughly corresponds to the [`group_imports` unstable rustfmt option](https://rust-lang.github.io/rustfmt/?version=v1.4.38&search=#group_imports), with the difference
that `rustfmt` does not distinguish workspace crates from external ones.

## Installation

```
$ cargo install --git https://github.com/cpg314/cargo-group-imports
```

Alternatively, see the binaries on the [Releases](https://github.com/cpg314/cargo-group-imports/releases) page.

## Usage

```
cargo group-imports [OPTIONS] [WORKSPACE]

Arguments:
  [WORKSPACE]  [default: current folder]

Options:
      --fix            Apply changes
      --color <COLOR>  [default: auto] [possible values: auto, always, never]
  -h, --help           Print help
  -V, --version        Print version
```

By default, the tool checks that the imports are correctly grouped and displays a diff otherwise. The `--fix` flag applies the necessary changes, if any. This matches the behaviour of `cargo clippy`.

![Screenshot](screenshot.png)

```
$ cargo group-imports
$ cargo group-imports --fix
```

The return code is `0` when no changes are necessary, `1` otherwise. This can be used in CI checks.
