In Rust/Cargo land, the most common convention is actually: don’t name two crates at all.

Most common: one package that builds both a library + the CLI

Make a single Cargo package named tool (often kebab-case on crates.io, e.g. my-tool) and include both:

src/lib.rs (library API)

src/main.rs (CLI that calls into the library)

Cargo treats that as “one package containing a library crate + a binary crate”, and both default to the package name. This is a very normal pattern (even Cargo itself is structured this way). 
Rust Documentation
+2
Rust 文档网
+2

This avoids “which one is the real name?” for users:

cargo install tool gives them the tool executable

tool = "x.y" gives them the library

When you do want two crates: tool + tool-cli

The main reason to split is dependency hygiene: Cargo package [dependencies] apply to all targets in that package, so if you put clap there, library users can end up pulling it in too. 
The Rust Programming Language Forum

In that case, the common move is a workspace:

Library crate: tool (the thing other Rust code depends on)

CLI crate: tool-cli (depends on tool)

People explicitly call out the -cli suffix as the standard-ish solution. 
The Rust Programming Language Forum
+1

And you can still name the produced binary tool:

# tool-cli/Cargo.toml
[package]
name = "tool-cli"
# ...

[[bin]]
name = "tool"
path = "src/main.rs"

What about tool-lib, tool-core, libtool?

tool-lib: less common in Rust; feels redundant because crates are libraries by default.

tool-core: used sometimes when there are multiple frontends (CLI, GUI, wasm, etc.) and “core” is intentionally internal-ish—but if you expect external users, they usually want tool, not tool-core.

libtool: not idiomatic for Cargo crate names.

Tiny naming gotcha: hyphens vs underscores

Package names are often kebab-case on crates.io (e.g. my-tool), while the crate is imported as my_tool (hyphens become underscores). 
