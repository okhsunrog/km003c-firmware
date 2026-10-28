0001e124: movs r4, #0
0001e126: movt r4, #0x4004
0001e12a: ldrh r0, [r4, #4]
0001e12c: mov sb, r8
0001e12e: cmp r0, #0
0001e130: beq.w #0x1df3c
0001e134: ldrb.w r0, [r4, #0x46]
0001e138: lsls r0, r0, #0x1f
0001e13a: beq #0x1e134
0001e13c: ldrb.w r0, [r4, #0x46]
0001e140: and r0, r0, #0xfe
0001e144: strb.w r0, [r4, #0x46]
0001e148: ldrh.w r0, [r4, #0x50]
0001e14c: cmp.w r0, #0x1f4
0001e150: ldrh.w r0, [r4, #0x52]
0001e154: ldr r1, [sp, #0x24]
0001e156: it hi
0001e158: addhi r1, #1
0001e15a: str r1, [sp, #0x24]
0001e15c: cmp.w r0, #0x1f4
0001e160: ldrh.w r0, [r4, #0x70]
0001e164: ldr.w r8, [sp, #0x1c]
0001e168: mov.w r1, #0x438000
0001e16c: it hi
0001e16e: addhi.w r8, r8, #1
0001e172: udiv fp, r1, r0
0001e176: ldr r0, [sp, #0x20]
0001e178: cbnz r0, #0x1e1ae
0001e17a: movw r0, #0xacf0
0001e17e: movt r0, #0x1fff
0001e182: ldr r1, [sp, #0x10]
0001e184: ldr r2, [sp, #0x14]
0001e186: adds r0, #0x2e
0001e188: ldr r3, [r1, #-0x8]
0001e18c: subs r2, #1
0001e18e: ldrh r7, [r3]
0001e190: ldr r6, [r1]
0001e192: add r7, r6
0001e194: str r7, [r1]
0001e196: ldrh r3, [r3]
0001e198: add.w r1, r1, #0xc
0001e19c: mul r3, fp, r3
0001e1a0: lsr.w r3, r3, #0xc
0001e1a4: strh r3, [r0, #0xa]
0001e1a6: strh r3, [r0]
0001e1a8: add.w r0, r0, #2
0001e1ac: bne #0x1e188
0001e1ae: bl #0x21cb4
0001e1b2: mov r5, r0
0001e1b4: bl #0x21c94
0001e1b8: movs r1, #0xc8
0001e1ba: movt r1, #0x42a7
0001e1be: ldr r1, [r1]
0001e1c0: mov lr, r0
0001e1c2: cbz r1, #0x1e1e2
0001e1c4: cmp.w r5, #-1
0001e1c8: ble #0x1e1e2
0001e1ca: ldr r0, [sp, #8]
0001e1cc: movs r1, #0x64
0001e1ce: ldrh r0, [r0]
0001e1d0: and r0, r0, #0xf
0001e1d4: smlabb r0, r5, r1, r0
0001e1d8: movw r1, #0xacf0
0001e1dc: movt r1, #0x1fff
0001e1e0: strh r0, [r1, #0x3c]
0001e1e2: movs r0, #0xc8
0001e1e4: str r5, [sp, #0x1c]
0001e1e6: movt r0, #0x42a7
0001e1ea: ldr r0, [r0, #4]
0001e1ec: cbz r0, #0x1e20c
0001e1ee: cmp.w lr, #0
0001e1f2: bmi #0x1e20c
0001e1f4: ldr r0, [sp, #4]
0001e1f6: movs r1, #0x64
0001e1f8: ldrh r0, [r0]
0001e1fa: and r0, r0, #0xf
0001e1fe: smlabb r0, lr, r1, r0
0001e202: movw r1, #0xacf0
0001e206: movt r1, #0x1fff
0001e20a: strh r0, [r1, #0x3e]
0001e20c: movw r5, #0xacf0
0001e210: movt r5, #0x1fff
0001e214: ldrh.w r0, [r5, #0x4e]
0001e218: ldr r1, [sp, #0x20]
0001e21a: ldr.w ip, [sp, #0x18]
0001e21e: orr r0, r0, #8
0001e222: add.w ip, ip, #1
0001e226: strh.w fp, [r5, #0x40]
0001e22a: strh.w r0, [r5, #0x4e]
0001e22e: cbz r1, #0x1e250
0001e230: movw r1, #0x5c29
0001e234: movt r1, #0xc28f
0001e238: mul r1, ip, r1
0001e23c: movw r2, #0xb852
0001e240: ror.w r1, r1, #1
0001e244: movt r2, #0x51e
0001e248: cmp r1, r2
0001e24a: bhs #0x1e2c2
0001e24c: b #0x1e2ba
0001e24e: nop 
0001e250: ldr r7, [sp, #0x14]
0001e252: movw r6, #0x851f
0001e256: movs r1, #0
0001e258: movt r6, #0x51eb
0001e25c: add.w r2, r5, r1, lsl #1
0001e260: ldrh r2, [r2, #0x38]
0001e262: ldr.w r3, [sb, r1, lsl #2]
0001e266: add r2, r3
0001e268: str.w r2, [sb, r1, lsl #2]
0001e26c: adds r1, #1
0001e26e: cmp r7, r1
0001e270: bne #0x1e25c
0001e272: movw r1, #0x5c29
0001e276: movt r1, #0xc28f
0001e27a: mul r1, ip, r1
0001e27e: movw r2, #0xb852
0001e282: ror.w r1, r1, #1
0001e286: movt r2, #0x51e
0001e28a: cmp r1, r2
0001e28c: bhs #0x1e2c2
0001e28e: ldr r1, [sp, #0x20]
0001e290: cbnz r1, #0x1e2ba
0001e292: ldr r2, [sp, #0x14]
0001e294: add.w r0, r5, #0x24
0001e298: mov r1, sb
0001e29a: nop 
0001e29c: ldr r3, [r1]
0001e29e: subs r2, #1
0001e2a0: smmul r3, r3, r6
0001e2a4: lsr.w r7, r3, #4
0001e2a8: add.w r3, r7, r3, lsr #31
0001e2ac: strh r3, [r0], #2
0001e2b0: str sl, [r1], #4
0001e2b4: bne #0x1e29c
0001e2b6: ldrh.w r0, [r5, #0x4e]
0001e2ba: orr r0, r0, #0x30
0001e2be: strh.w r0, [r5, #0x4e]
0001e2c2: cmp.w ip, #0xfa
0001e2c6: bne.w #0x1df34
0001e2ca: ldr r0, [sp, #0x60]
0001e2cc: movw r1, #0xcccd
0001e2d0: movt r1, #0xcccc
0001e2d4: umull r0, r1, r0, r1
0001e2d8: lsrs r0, r1, #3
0001e2da: movs r1, #0
0001e2dc: movt r1, #0x41eb
0001e2e0: udiv sl, r1, r0
0001e2e4: ldr r0, [sp, #0xc]
0001e2e6: movs r5, #0
0001e2e8: str.w lr, [sp, #0x18]
0001e2ec: cbz r0, #0x1e31e
0001e2ee: movw r0, #0xacf0
0001e2f2: movt r0, #0x1fff
0001e2f6: ldr r7, [sp, #0x10]
0001e2f8: ldr.w fp, [sp]
0001e2fc: add.w r6, r0, #0x1a
0001e300: ldr r0, [r7]
0001e302: mov.w r2, #0xfa000
0001e306: umull r0, r1, r0, sl
0001e30a: movs r3, #0
0001e30c: bl #0x4474
0001e310: strh r0, [r6], #2
0001e314: subs.w fp, fp, #1
0001e318: str r5, [r7], #0xc
0001e31c: bne #0x1e300
0001e31e: movs r0, #0xc8
0001e320: movt r0, #0x42a7
0001e324: ldr r0, [r0]
0001e326: movw r5, #0xacf0
0001e32a: movt r5, #0x1fff
0001e32e: cbz r0, #0x1e342
0001e330: ldr r0, [sp, #0x1c]
0001e332: cmp.w r0, #-1
0001e336: ble #0x1e342
0001e338: ldrh r0, [r5, #0x3c]
0001e33a: add.w r0, r0, r0, lsl #2
0001e33e: lsls r0, r0, #1
0001e340: strh r0, [r5, #0x1e]
0001e342: movs r0, #0xc8
0001e344: movt r0, #0x42a7
0001e348: ldr r0, [r0, #4]
0001e34a: cmp r0, #0
0001e34c: ldr r0, [sp, #0x18]
0001e34e: beq #0x1e35e
0001e350: cmp r0, #0
0001e352: bmi #0x1e35e
0001e354: ldrh r0, [r5, #0x3e]
0001e356: add.w r0, r0, r0, lsl #2
0001e35a: lsls r0, r0, #1
0001e35c: strh r0, [r5, #0x20]
0001e35e: ldrh.w r0, [r5, #0x4e]
0001e362: mov.w ip, #0
0001e366: orr r0, r0, #0x40
0001e36a: strh.w r0, [r5, #0x4e]
0001e36e: ldr r0, [sp, #0x24]
0001e370: strh.w sl, [r5, #0x22]
0001e374: strb.w r0, [r5, #0x65]
0001e378: movs r0, #0
0001e37a: str.w ip, [sp, #0x60]
0001e37e: strb.w r8, [r5, #0x66]
0001e382: mov.w r8, #0
0001e386: str r0, [sp, #0x24]
