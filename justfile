dump:
    fasm test.S test
    objdump -d test
#objcopy -Obinary test test.bin
#xxd test.bin

dump2:
    nasm -gdwarf -f elf64 -Ox -o dump.o dump.asm
    ld -fuse-ld=mold -o dump dump.o
    objdump -d dump -M intel | bat -l asm
#dump2:
#fasm dump.asm dump
#objdump -d dump -M intel | bat -l asm



mandel:
    cargo run --release -- mandel.bf

test-jit FILE:
    cargo run -- --run jit {{FILE}}

rrun MODE FILE:
    cargo run --release -- --run {{MODE}} {{FILE}}

rdrun MODE FILE:
    cargo run --profile rel-with-debug -- --run {{MODE}} {{FILE}}

jit-release FILE:
    cargo run --release -- --run jit {{FILE}}

dbench MODE COUNT FILE:
    cargo run -- --run {{MODE}} --meassure {{COUNT}} {{FILE}}

bench MODE COUNT FILE:
    cargo run --release -- --run {{MODE}} --meassure {{COUNT}} {{FILE}}
