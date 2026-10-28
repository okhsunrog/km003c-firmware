# Display and buttons hardware acceptance

Tested on KM003C HW2.1 on 2026-10-28. The owner confirmed the visible
“KM003C / Rust” screen and all four button rows responding correctly.

- Own Rust QSPI direct-command transport drives the ST7789-compatible 240×240
  panel through an `embedded-graphics` RGB565 draw target. No native loader
  functions are called. Initialization parameters follow the native panel code.
- AW9523B ID register returned `0x23` at I2C address `0x5b`. I2C3 uses PC13
  SDA / PH2 SCL. P1_1 supplies panel reset; registers `0x22` / `0x23` set
  the two backlight channels. Unrelated expander bits are preserved.
- Display SCK is PB14, MOSI PB13, D/C PB10 and CS PB15. Shared flash CS PB1
  is deselected and checked high before each LCD transaction.
- Active-low buttons are Exit PB7, OK PB11, Left PB8 and Right PB9.
  Retention counters were 5 / 6 / 6 / 8, with released mask 0.
- Concurrent USB echo passed all nine payload sizes, including 512 bytes,
  using the checked-in `tools/usb_echo.py` helper.
- The 90-second deadline returned to native DFU. Holding Exit for two seconds
  is implemented as another return path, but was not separately accepted.
- Stock 1.9.9 was restored and verified by full 453828-byte readback.
  A subsequent loader dump over `[0x4,0x4000)` matched the preserved original.

`hardware-run.log` contains the maintenance run, retained counters and normal
reset result. `verification.json` records the acceptance and restoration.
`tested-image.bin` is the exact tested image; subsequent source cleanup only
added comments and removed an unused read-only probe task. It is not a claim
that rebuilding the final source produces this exact hash.

An earlier attempt tripped the flash-CS guard because the code read reserved
address `0x40053812` instead of PIDRB at `0x40053810`. The retained formatted
panic identified “Flash CS not high” and the source location; reset returned
to the native loader. The corrected address was used for this successful run.
This exercised panic-message retention and the guard before LCD commands.
