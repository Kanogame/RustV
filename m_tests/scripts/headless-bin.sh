clang -Wl,-Ttext=0x0 -nostdlib --target=riscv64 -march=rv64g -mno-relax -o target/a.out $1
llvm-objcopy -O binary target/a.out target/a.out