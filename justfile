dump:
    fasm test.S test
    objdump -d test
#objcopy -Obinary test test.bin
#xxd test.bin

mandel:
    cargo run --release -- mandel.bf
