SECTIONS
{
  .km_metadata ORIGIN(FLASH) + 0x420 : { KEEP(*(.km_metadata)); } > FLASH
}
INSERT BEFORE .text;
ASSERT(SIZEOF(.km_metadata) == 0x40, "KM003C metadata must occupy 64 bytes");
ASSERT(SIZEOF(.vector_table) <= 0x420, "Vectors overlap metadata");
