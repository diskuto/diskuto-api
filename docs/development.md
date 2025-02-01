Local Development
===========

Implementation Details
----------------------

Some basic concepts to get you familiar with how things are organized:

* [URL Layout]
* [Data Format]
* [Cryptography]

[URL Layout]: ./url_layout.md
[Data Format]: ./data_format.md
[Cryptography]: ./crypto.md


Initial Setup
-------------

Install these dependencies:
 * [Rust]
 * [protoc]

[rust]: https://rustup.rs/
[protoc]: https://developers.google.com/protocol-buffers/


Run `cargo build` in the project root to build the web server portion.

Run `cargo run db init` to initialize an empty database for development.

Run `cargo run serve` to start the API server.

Alternatively, if you use Docker or Podman, you can follow the
instructions in [examples/full-stack] to build and run this
code in a container.

[examples/full-stack]: ../examples/full-stack/




