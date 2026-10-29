# SC7A20-family accelerometer and automatic screen rotation

On the tested KM003C HW2.1, I2C address `0x19` returns WHO_AM_I (`0x0f`) =
`0x11`. The register map, working XYZ data and 6D orientation events match the
Silan SC7A20 family. This identifies the family; it does not resolve an H suffix,
silicon revision or physical package marking from the shared ID alone.

The manufacturer's [SC7A20 page](https://www.silan.com.cn/en/product/details/47.html)
describes a three-axis accelerometer with 6D/4D orientation detection. The full
manufacturer-authored SC7A20 datasheet, Rev1.3, was obtained from a
[mirror](https://www.unikeyic.com/media/datasheet/0d/29/07e1/0d2907e1144f1d3e98f0a7bde9391acf.pdf).
Selected pages were read through the user's `mistral_ocr` CLI, including the I2C
protocol, identity, output sensitivity and control registers.

## Configuration and shared bus

```sh
cargo build --release --locked --features cdc-telemetry,orientation,manual-ui-test
```

`orientation` is opt-in. SC7A20 and AW9523B share I2C3, PC13 SDA / PH2 SCL,
100 kHz. One async mutex owns the physical controller; the accelerometer and
AW9523 use separate Embassy `I2cDevice` handles. A second mutex protects complete
AW9523 register-update sequences, including CC2 Rd changes from the PD task.
Sensor transactions can run between expander transactions without interleaving
another expander read-modify-write sequence. No second controller owner is
created. [CC2 acceptance](../hardware/cc2/README.md) exercises this sharing.

The driver checks ID before all configuration writes, powers down acquisition,
sets 10 Hz, high-resolution ±2 g and BDU, then reads back the configuration.
CTRL1/2/4 are `27/c0/88`; INT1_CFG/THS are `ff/0f`. CTRL3/5/6 are zero: no sensor
interrupt pin is routed. Calibration, NVM, self-test and reserved registers are
untouched. A wrong ID, bus failure or readback mismatch disables orientation;
measurement, USB and display remain available in a fixed orientation.

Each poll bursts STATUS and six XYZ bytes using I2C auto-increment subaddress
`0xa7`. Signed, left-aligned 12-bit values convert at 1 mg/count for this profile.
BDU protects axis byte pairs; this does not establish an atomic simultaneous
measurement across the accelerometer and INA229. Polling occurs at most every
100 ms. There is no claim of calibrated acceleration or high-rate motion capture.

## Screen direction

Catalog stock 1.9.9 reads INT1_SOURCE `0x31` and maps its low-nibble XL/XH/YL/YH
flags to directions `0/2/1/3`. The Rust filter requires three fresh data-ready
observations of the same valid direction. Flat/unasserted/ambiguous orientation
resets the pending change and retains the current screen direction.

The owner confirmed a fixed mounting offset: all directions need an additional
90 degrees counterclockwise. The board applies `(native_direction + 3) % 4`,
and uses the same offset on startup. The owner then confirmed the corrected
screen is readable in all four positions.

Each dirty tile is transformed in the MCU and streamed to a single rotated
rectangle. MADCTL, panel offsets and working panel initialization stay unchanged.
There is no additional framebuffer or per-pixel window setup. A direction change
clears/repaints the full layout once; ordinary meter updates remain dirty regions.
This does not add TE/VSYNC synchronization.

With CDC telemetry, `accel` records contain `id`, `source`, three `xyz_mg` values,
rendered quarter-turn `rotation`, poll `count`, and `error`. These are diagnostics;
no accelerometer-specific USB command is required. Physical Exit stays the same
button in every orientation.

See [hardware acceptance](../hardware/orientation/README.md). Host tests cover
wrong-ID write gating, signed burst decoding, native direction mapping, debounce
and asymmetric tile transforms. Visual/hardware acceptance is separate.
