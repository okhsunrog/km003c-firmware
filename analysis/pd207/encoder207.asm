1fff8184: push.w {r4, r5, r6, r7, r8, sb, lr}
1fff8188: sub sp, #4
1fff818a: mov r6, r1
1fff818c: movw r1, #0x4b84
1fff8190: mov r5, r2
1fff8192: movt r1, #5
1fff8196: movs r2, #0xc0
1fff8198: mov sb, r3
1fff819a: mov r8, r0
1fff819c: bl #0x1fff8776
1fff81a0: movw r0, #0x10da
1fff81a4: movt r0, #7
1fff81a8: add.w r4, r8, #0xc0
1fff81ac: ldrh.w r1, [r0, r6, lsl #3]
1fff81b0: add.w r7, r0, r6, lsl #3
1fff81b4: mov r0, r4
1fff81b6: bl #0x1fff8780
1fff81ba: ldrh r1, [r7, #2]
1fff81bc: add.w r4, r4, r0, lsl #1
1fff81c0: mov r0, r4
1fff81c2: bl #0x1fff8780
1fff81c6: ldrh r1, [r7, #4]
1fff81c8: add.w r4, r4, r0, lsl #1
1fff81cc: mov r0, r4
1fff81ce: bl #0x1fff8780
1fff81d2: ldrh r1, [r7, #6]
1fff81d4: add.w r4, r4, r0, lsl #1
1fff81d8: mov r0, r4
1fff81da: bl #0x1fff8780
1fff81de: cmp r6, #5
1fff81e0: add.w r6, r4, r0, lsl #1
1fff81e4: blo #0x1fff81ee
1fff81e6: sub.w r0, r6, r8
1fff81ea: asrs r0, r0, #1
1fff81ec: b #0x1fff826c
1fff81ee: mov r0, r5
1fff81f0: mov r1, sb
1fff81f2: bl #0x1fff878a
1fff81f6: add.w r7, sb, #4
1fff81fa: lsls r1, r7, #0x10
1fff81fc: str.w r0, [r5, sb]
1fff8200: beq #0x1fff823a
1fff8202: movw sb, #0x6c9c
1fff8206: movt sb, #5
1fff820a: nop
1fff820c: ldrb r0, [r5]
1fff820e: subs r7, #1
1fff8210: and r0, r0, #0xf
1fff8214: ldrb.w r1, [sb, r0]
1fff8218: mov r0, r6
1fff821a: bl #0x1fff8780
1fff821e: ldrb r1, [r5], #1
1fff8222: add.w r4, r6, r0, lsl #1
1fff8226: lsrs r1, r1, #4
1fff8228: ldrb.w r1, [sb, r1]
1fff822c: mov r0, r4
1fff822e: bl #0x1fff8780
1fff8232: lsls r1, r7, #0x10
1fff8234: add.w r6, r4, r0, lsl #1
1fff8238: bne #0x1fff820c
1fff823a: mov.w r0, #0x130013
1fff823e: str r0, [r6]
1fff8240: movs r1, #0x26
1fff8242: str.w r0, [r6, #0xa]
1fff8246: sub.w r0, r6, r8
1fff824a: movt r1, #0x13
1fff824e: add.w r2, r0, #0x10
1fff8252: str r1, [r6, #4]
1fff8254: movs r1, #0x13
1fff8256: lsls r0, r2, #0x1e
1fff8258: strh r1, [r6, #8]
1fff825a: mov.w r1, #0x26
1fff825e: asr.w r0, r2, #1
1fff8262: strh r1, [r6, #0xe]
1fff8264: itt mi
1fff8266: addmi r0, #1
1fff8268: strhmi.w r1, [r8, r2]
1fff826c: add sp, #4
