# Passive RPC capture with Pixel 8 Pro

KM003C HW2.1, stock charger and Pixel 8 Pro connected through the meter; HID
connected to the host. The installed custom firmware reported RPC protocol 3.
Only passive monitoring was requested; the meter did not attach Rd or send PD
messages. These captures do not test active sink negotiation.

In [cc1.ndjson](cc1.ndjson), 30 seconds of monitoring delivered 24 decoded
packets on input 1, with consecutive capture sequences 1–24. Final counters
were 24 bursts, zero overruns, noise, full buffers, DMA errors, mismatches,
stop faults, input switches and switch discards; maximum handler work was 902
cycles. Measured VBUS was about 9.19 V. This validates one passive CC1 session,
not unconditional lossless capture.

[flip.ndjson](flip.ndjson) records the next 90-second session. It ended as the
cable was being flipped: 66 packets on input 1, one input switch, 195 short
bursts and zero reported overruns or DMA faults. It did not record an input 2
packet. In [cc2-idle.ndjson](cc2-idle.ndjson), CC2 measured about 1.66 V and
VBUS about 9.18 V, but 35 seconds produced no capture events. This alone does
not distinguish absence of bus activity from a receiver problem.

In [cc2-reconnect.ndjson](cc2-reconnect.ndjson), monitoring was fixed to input 2
before the owner disconnected and reconnected the cable without flipping it.
The capture reported 23 decoded packets on input 2 and 67 total bursts, including
44 short/noise bursts; overruns, full, DMA errors, mismatches and stop faults were
zero. A host interrupt requested a normal Stop and the record finished with
the PD owner reporting Detached. VBUS returned to about 9.18 V. This confirms reception on both
CC inputs in these sessions; automatic CC1-to-CC2 switching with actual
packets after the flip has not yet been demonstrated in the RPC image.
