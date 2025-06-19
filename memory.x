/* linker_memory.x — works for nRF52840 with SoftDevice S140 present */
MEMORY
{
  FLASH : ORIGIN = 0x26000, LENGTH = 0x5A000  /* App region */
  RAM   : ORIGIN = 0x20002000, LENGTH = 0x3E000  /* 252 KB RAM */
}
