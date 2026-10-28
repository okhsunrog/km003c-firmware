# Dirty-row display smoke test

The previous UI erased the entire 240×240 panel on every button transition,
then filled four button backgrounds and drew text through per-pixel QSPI
transactions. The owner reported a diagonal tearing-like band during presses.
This write pattern is the leading explanation; stock display synchronization
has not been established and no TE/VSYNC behavior is assumed.

The new UI draws backgrounds/text into a reusable 216×34 RGB565 RAM tile,
then uploads only changed rows as continuous pixel streams. Header/footer are
static; ADC updates compare the rendered string and replace only a 216×14 tile.
Backlight is enabled after the initial composed UI has been sent. The tile uses
14688 bytes rather than a 115200-byte full framebuffer. This removes separate
visible erase/text phases but does not make live panel GRAM updates atomic.

The exact image here ran on HW2.1 for 90 seconds, collected 360 INA229 samples
without SPI errors and passed all nine USB echo sizes. No button presses were
observed in the retained counters; disappearance of the reported band has not
yet been visually confirmed. The host test checks tile clipping and row order.

Stock 2.0.7 was restored with full 456896-byte readback and unchanged loader.
The upload reused the previously verified stock backup, checking live model,
hardware and metadata instead of acquiring two more full APP reads. Readback
of the newly written custom image and stock restoration was retained.
