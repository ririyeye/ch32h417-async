MEMORY
{
    FLASH  (rx) : ORIGIN = 0x08000000, LENGTH = 480K
    RAM    (rw) : ORIGIN = 0x200A0000, LENGTH = 128K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
    .init :
    {
        *(.init)
    } > FLASH

    .vector ALIGN(1024) :
    {
        *(.vector)
    } > FLASH

    .text :
    {
        *(.trap .trap.*)
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        *(.sdata2.*)
    } > FLASH

    _sidata = LOADADDR(.data);

    .data :
    {
        _sdata = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        . = ALIGN(4);
        _edata = .;
    } > RAM AT > FLASH

    .rtt (NOLOAD) :
    {
        *(.rtt)
        *(.rtt_buf)
        . = ALIGN(4);
    } > RAM

    .bss (NOLOAD) :
    {
        _sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(4);
        _ebss = .;
    } > RAM

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
    }
}
