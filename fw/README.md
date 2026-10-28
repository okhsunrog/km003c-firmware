# Preserved analysis inputs

These immutable inputs were copied byte-for-byte from the protocol research
checkout during the hardware-documentation migration on 2026-10-28. They are
analysis/recovery evidence, not automatically selected firmware-update images.

| File | Meaning | SHA-256 |
| --- | --- | --- |
| `KM003C_V1.9.9_key0_ecb.bin` | Older decrypted catalog application; mapped at runtime `0x4000` | `9ce6125da454585fde1b5744018f94edc697a23941292cbf55d2a411d4df7517` |
| `bootloader_hw2.1_00000004-00003fff.bin` | HW2.1 loader bytes `[4,0x4000)`; initial SP word was not readable | `17d51eeff9fb6abe1a7024a108006248e0aaebdaae52de69a3e26c000565d4bf` |

The installed stock 2.0.7 reference has a different hash and layout, documented
with its hardware comparisons. Do not substitute the older catalog image for
that verified per-device backup.
