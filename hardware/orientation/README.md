# SC7A20-family and automatic display rotation acceptance

KM003C HW2.1, 2026-10-28. The read-only probe returned I2C `0x19`, WHO_AM_I
`0x11`, native CTRL1/2/4 `27/c0/08`, INT1_CFG `ff`, changing XYZ and 6D source.
The stationary probe had Z near -925 mg after interpreting the native HR ±2 g
profile. This identifies the SC7A20 family; suffix/revision is not established.

The final driver explicitly configured and read back 10 Hz, HR ±2 g with BDU,
and polled STATUS/XYZ plus INT1_SOURCE. No calibration/NVM, reserved register,
self-test or interrupt output was written. AW9523 and the sensor share I2C3 under
the UI task's exclusive ownership.

The initial screen mapping required a fixed 90-degree counterclockwise mounting
offset. After that correction the owner confirmed all four screen orientations
were correct. The final 90-second CDC record contains 360 acceleration records
and 360 calibrated ADC records, with zero sensor errors, CDC drops or malformed
JSON. The telemetry file captures only a subset of the four directions; complete
four-position visual acceptance is the owner's report, not inferred from that
subset. Flat/ambiguous input retains the prior direction.

`probe.ndjson` is the original read-only identity/configuration evidence.
`corrected.ndjson` records the final image's raw source flags, mg values, direction
and poll count alongside INA229 measurements. `tested-image.bin` is the exact
image used for the corrected visual test. `hardware-run.log` preserves image
readback, unchanged loader comparison and normal native-DFU return.
`verification.json` records counts and hashes.

The manufacturer-authored SC7A20 Rev1.3 datasheet was read through `mistral_ocr`,
selecting PDF pages 2 and 9–15 with HTML tables. Only selected OCR output was used
locally; the full datasheet is not republished here. Its WHO_AM_I, I2C
subaddress-increment, BDU and 1 mg/LSB HR ±2 g definitions match the driver.
See [sources and implementation details](../../docs/orientation.md).

Software tests cover identity gating before writes, signed burst decoding, native
orientation flags, fresh-data debounce and non-square streamed tile transforms.
All 20 driver tests passed; default, orientation-only and all-feature clippy
checks also passed. This does not establish acceleration metrology accuracy,
exact sensor revision, power consumption, cold power-cycle acceptance or TE sync.

The same verified stock 2.0.7 rollback was restored with full 456896-byte readback;
`restore.log` preserves success. Final loader SHA-256 still matched the original
`17d51eeff9fb6abe1a7024a108006248e0aaebdaae52de69a3e26c000565d4bf`.
No new stock backup was acquired. Restoration was already underway when the
owner requested keeping custom firmware between tests. Future runs follow that
policy: no automatic stock restore after each test.
