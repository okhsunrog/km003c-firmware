# Explicit INA229 startup, factory calibration and meter UI

Tested on HW2.1 on 2026-10-28. This stage replaces the button diagnostic screen
with large voltage, signed current and power, plus temperature and CAL/RAW.
Buttons remain active; Exit held about two seconds requests the native loader.
The exact 115472-byte meter image is `tested-meter.bin`.

## Hardware observations

The sensor-reset test explicitly reset only INA229, waited 20 ms, and observed
CONFIG/ADC_CONFIG/SHUNT_CAL/SHUNT_TEMPCO defaults `0000/fb68/1000/0000`.
The normal configuration path installed and read back `0170/fdb0/1062/001e`,
then waited for conversion-ready. It collected 360 samples without SPI errors.
Manufacturer/device IDs were `5449/2291`.

The shared QSPI owner read flash legacy ID `8515` and settings at `0x3ff100`.
The persisted 96-byte block's stored/calculated CRC was `52677441`.
Five voltage offsets are -6 µV; all ten gains are 1002221 ppm; current offset
is 94 µA. No write-enable/program/erase command was sent. The exact flash model
remains unknown. The last reset-test raw current was -570 counts (-9120 µA),
corrected to -9234 µA. Raw per-device settings are not needed as a test fixture;
checked-in calibration fixtures use synthetic coefficients.

The following normal meter run collected 2400 samples over ten minutes, with
valid factory correction and no SPI errors. Concurrent USB echo passed all nine
payload sizes. Both images returned to native DFU and preserved bootloader bytes.
`reset-*.bin` and `meter-*.bin` preserve the retained records; logs and
`verification.json` preserve run facts and hashes.

The normal meter image did not reset the sensor; its initial configuration was
already native. Therefore these runs establish sensor configuration from reset
and operation with persisted calibration, **not whole-board cold-start acceptance**.
Physical power-cycle and visual confirmation of the new screen are pending.
A rerun uses the same verified image without flash writing.

## Native-code comparison

The stock 2.0.7 reader `0x21818` adds `0x3ff000`; startup `0x48520` reads offset
`0x100`. Flash reads use `0xce24`; `0xcd88` exits QPI. The Rust reader uses only
`FF`, `90` and `03`, before panel initialization while it owns the entire bus.
It leaves flash in standard SPI. Panel transactions preserve that configuration.

Correction entry `0x17b1c` writes instantaneous V/I to `0x1fffacf0/f4` before
queue/timestamp work at `0x4d244`. The host fixture generator executes these
instructions using a synthetic settings profile. All 324 voltage/current
boundary and sign vectors match Rust. This is algorithm comparison, not an
absolute accuracy certification. Wide ADC range and passive PD remain untested.

The stock live RAM settings-A copy at `0x1fffb5a8` changes word `+0x10` without
updating its CRC. The persisted block is valid. The custom firmware validates
that persisted block, rejecting CRC/format/flag/bounds errors and showing RAW.
It never migrates or overwrites calibration.

See [implementation and retained layout](../../docs/adc-pd.md),
[earlier stock comparison](../adc-207/README.md), and the fixture generator
at `tools/native_calibration_vectors.py`.
