SECTIONS
{
  .km_metadata ORIGIN(FLASH) + 0x420 : { KEEP(*(.km_metadata)); } > FLASH
}
INSERT BEFORE .text;
ASSERT(SIZEOF(.km_metadata) == 0x40, "KM003C metadata must occupy 64 bytes");
ASSERT(SIZEOF(.vector_table) <= 0x420, "Vectors overlap metadata");

/* CPU-only PD decode loop, copied explicitly before use. DMA stays in RAM. */
SECTIONS
{
  .pd_fast : ALIGN(4) {
    __spd_fast = .;
    *(.pd_fast .pd_fast.*);
    . = ALIGN(4);
    __epd_fast = .;
  } > SRAMH AT > FLASH
  __sipd_fast = LOADADDR(.pd_fast);
}
/* Keep cortex-m-rt's __edata assignment adjacent to .data. Inserting another
 * RAM bank after it would make CRT startup copy until the wrong address. */
INSERT BEFORE .data;
ASSERT(__edata == ADDR(.data) + SIZEOF(.data), "Invalid CRT data-copy boundary");
