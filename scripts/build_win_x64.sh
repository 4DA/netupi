#/bin/sh
cargo install cross
cross build --target x86_64-pc-windows-gnu --release
cp target/x86_64-pc-windows-gnu/release/netupi.exe .
