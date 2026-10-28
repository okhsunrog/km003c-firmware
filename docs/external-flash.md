# External NOR flash: verified access and limits

The external serial NOR shares HC32 QSPI with the display. Native firmware uses
a 4 MiB storage layout; the owner also reports `ROM 4MB` in the stock device
information menu. This refers to external storage rather than the MCU
application flash. The MCU has 512 KiB internal flash; the factory settings-A
copy is at offset
`0x3ff100`. The tested board returned legacy command-90 manufacturer/device ID
`85 15`. Manufacturer `85` is consistent with Puya's documented REMS response.
The menu confirms the advertised 4 MB storage size; exact part, voltage suffix,
revision and independent capacity identification still require JEDEC/SFDP or
a readable package marking. Do not assign a specific P25Q32
variant from this legacy ID alone.

The custom firmware currently supports a narrow **read-only startup path**:

1. Before panel init, its exclusive QSPI owner deselects the LCD (PB15).
2. It exits inherited QPI with `FF`, then selects standard SPI.
3. Command `90` reads the legacy ID; command `03` reads 96 bytes at `3ff100`.
4. CRC-32 and coefficient/format bounds validate factory calibration.
5. Later display transactions deselect flash through PB1 and preserve the
   controller configuration when restoring the shared bus.

This is sufficient for the working CAL measurements. There is no general storage
API, filesystem, recording writer, flash erase/program support or wear levelling.
No write-enable, program or erase opcode is sent by this path. Factory settings
are never rewritten by the custom image. Native mapped flash starts at
`0x98000000`, but concurrent access is forbidden while the controller drives LCD.

Next identification should read JEDEC `9F` and SFDP `5A` when this owner has the
bus. Combine that with native sector/page operations and, if available, package
marking before implementing a writable driver. A storage task would need proper
serialization with the display and protection of calibration/settings regions.

Application flashing in our hardware tests targets the **MCU's internal flash**,
starting at `0x4000`. That is separate from this external NOR. Native loader and
ICG are preserved. To reduce unnecessary internal-flash erase cycles, keep a
working custom image installed between tests; restore stock only for a concrete
reference comparison, recovery or explicit request. Returning to native DFU is
a software reset, not a flash erase.

Evidence: [startup/calibration](../hardware/cold-adc/README.md). Puya's
[manufacturer-authored REMS description](https://www.puyasemi.com/download_path/%E6%95%B0%E6%8D%AE%E6%89%8B%E5%86%8C/Flash/P25Q32SL_Datasheet_V1.9.pdf)
was checked using `mistral_ocr` on selected pages; this is an identification
reference, not proof that the installed chip is P25Q32SL.
