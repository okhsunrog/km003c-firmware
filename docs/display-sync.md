# Remaining display tearing

The owner observed a full-screen diagonal band on button transitions in the
original test UI. The dirty-row tile update removed that full-screen band,
but a slight intermittent tear remained inside the rectangle being changed.
That is partial improvement. A RAM tile precomposes the update; it does not
synchronize writes against the panel's live scanout.

The ST7789V Linux driver provides a concrete synchronization reference: optional
TE GPIO input, rising-edge interrupt, `SET_TEAR_ON`, then a bounded wait for the
panel event before updating display memory. See the
[upstream driver](https://github.com/torvalds/linux/blob/master/drivers/staging/fbtft/fb_st7789v.c).
This establishes a supported approach, not the wiring of this KM003C board.

## What the recovered stock code shows

In the recovered 1.9.9 RAM blit at `0x1fff8000`, the pixel loop at `0x1fff8110`
reads two source bytes and writes them directly to QSPI.DCOM. The inspected path
locks the shared bus, waits for QSPI idle and flash CS high, sets the LCD address
window and streams RGB565. No TE wait was identified in that path. This does not
rule out synchronization in callers or elsewhere in another firmware build.

Do not mislabel the following native waits as panel synchronization:

- QSPI.SR bit 15 is PFAN (prefetch-related status), not a TE pin.
- Bit-band alias `0x42a788a0` is GPIO.PCRB1.PIN: flash CS input, not LCD TE.

The inspected 2.0.7 immediate-command sites did not reveal an LCD `0x35` TEON
call. This limited search does not prove that no table-driven setup or external
synchronization exists. Stock smoothness alone is not evidence of a TE wire.

## Next requirements for a synchronization implementation

Establish whether panel TE reaches a bonded, available MCU input. If so, enable
TE and wait asynchronously for its event, with a timeout and measured transfer
budget. A TE edge alone does not guarantee that a long transfer fits the safe
window. If TE is unavailable, investigate whether the panel's scanline register
can actually be read through this board's serial wiring; that readback path is
not established. Fixed sleeps cannot maintain phase against an independent LCD
oscillator and are not a guaranteed fix.

No TE GPIO, blind readback wiring or arbitrary panel clock changes were enabled
in the accepted dirty-row image. The manual test and its remaining limitation
are recorded in [hardware evidence](../hardware/dirty-rows/README.md).
