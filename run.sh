pushd tests
./headless-bin.sh add-addi.s
cargo run ./target/a.out
popd