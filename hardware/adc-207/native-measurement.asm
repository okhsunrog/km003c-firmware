00006d68: bfi r1, r2, #0, #5
00006d6c: str r1, [r0, #0x2c]
00006d6e: ldr r1, [r0, #0x2c]
00006d70: bfi r1, r2, #8, #5
00006d74: str r1, [r0, #0x2c]
00006d76: bx lr
00006d78: push {r4, r5, r6, lr}
00006d7a: movs r0, #7
00006d7c: bl #0x6fac
00006d80: movw r5, #0xb1f4
00006d84: movt r5, #0x1fff
00006d88: lsls r1, r0, #8
00006d8a: ldrh r0, [r5]
00006d8c: mov r2, r1
00006d8e: it mi
00006d90: rsbmi r2, r1, #0
00006d92: lsls r3, r0, #0x1b
00006d94: bmi #0x6dea
00006d96: movw r3, #0x145f
00006d9a: movt r3, #0x1f3a
00006d9e: cmp r2, r3
00006da0: asr.w r4, r1, #6
00006da4: bhi #0x6e50
00006da6: movw r6, #0xb484
00006daa: movt r6, #0x1fff
00006dae: ldrb r1, [r6]
00006db0: adds r1, #1
00006db2: uxtb r2, r1
00006db4: cmp r2, #0x21
00006db6: strb r1, [r6]
00006db8: blo #0x6e50
00006dba: movs r1, #0
00006dbc: strb r1, [r6]
00006dbe: orr r1, r0, #0x10
00006dc2: movs r0, #0
00006dc4: strh r1, [r5]
00006dc6: bl #0x702c
00006dca: nop
00006dcc: movs r0, #1
00006dce: bl #0x4cffc
00006dd2: movs r0, #0
00006dd4: bl #0x6f50
00006dd8: lsls r1, r0, #0x1b
00006dda: bmi #0x6e3c
00006ddc: ldrb r1, [r6]
00006dde: adds r1, #1
00006de0: uxtb r2, r1
00006de2: cmp r2, #0x20
00006de4: strb r1, [r6]
00006de6: blo #0x6dcc
00006de8: b #0x6e3c
00006dea: movw r3, #0x7891
00006dee: movt r3, #0x7ce8
00006df2: cmp r2, r3
00006df4: asr.w r4, r1, #8
00006df8: blo #0x6e50
00006dfa: movw r6, #0xb484
00006dfe: movt r6, #0x1fff
00006e02: ldrb r1, [r6]
00006e04: adds r1, #1
00006e06: uxtb r2, r1
00006e08: cmp r2, #0x21
00006e0a: strb r1, [r6]
00006e0c: blo #0x6e50
00006e0e: movs r1, #0
00006e10: strb r1, [r6]
00006e12: bic r1, r0, #0x10
00006e16: movs r0, #0
00006e18: strh r1, [r5]
00006e1a: bl #0x702c
00006e1e: nop
00006e20: movs r0, #1
00006e22: bl #0x4cffc
00006e26: movs r0, #0
00006e28: bl #0x6f50
00006e2c: lsls r1, r0, #0x1b
00006e2e: bpl #0x6e3c
00006e30: ldrb r1, [r6]
00006e32: adds r1, #1
00006e34: uxtb r2, r1
00006e36: cmp r2, #0x20
00006e38: strb r1, [r6]
00006e3a: blo #0x6e20
00006e3c: strh r0, [r5]
00006e3e: movs r0, #1
00006e40: bl #0x4cffc
00006e44: movw r0, #0xb484
00006e48: movt r0, #0x1fff
00006e4c: movs r1, #0
00006e4e: strb r1, [r0]
00006e50: mov r0, r4
00006e52: pop {r4, r5, r6, pc}
00006e54: push {r7, lr}
00006e56: movs r0, #5
00006e58: bl #0x6fac
00006e5c: lsrs r0, r0, #4
00006e5e: pop {r7, pc}
00006e60: push {r4, lr}
00006e62: movw r0, #0xb484
00006e66: movt r0, #0x1fff
00006e6a: movs r1, #0
00006e6c: strb r1, [r0]
00006e6e: movs r0, #0xc4
00006e70: movt r0, #0x4290
00006e74: str r1, [r0]
00006e76: movw r0, #0xc404
00006e7a: movt r0, #0x4001
00006e7e: movs r2, #8
00006e80: str r2, [r0]
00006e82: mov.w r2, #0xe10
00006e86: str r2, [r0, #8]
00006e88: movw r2, #0x40d
00006e8c: str r2, [r0, #0x14]
00006e8e: ldr r2, [r0]
00006e90: movw r4, #0xb1f4
00006e94: orr r2, r2, #0x40
00006e98: str r2, [r0]
00006e9a: movw r0, #0x3bfc
00006e9e: movt r0, #0x4005
00006ea2: movw r2, #0xa501
00006ea6: strh r2, [r0]
00006ea8: movs r2, #0x2f
00006eaa: strh r2, [r0, #0x26]
00006eac: movs r2, #0x2c
00006eae: strh r2, [r0, #0x2e]
00006eb0: movs r2, #0x2d
00006eb2: strh.w r2, [r0, #0x46]
00006eb6: movw r2, #0x8380
00006eba: movt r2, #0x42a7
00006ebe: str r1, [r2, #0x3c]
00006ec0: movs r1, #1
00006ec2: str r1, [r2, #4]
00006ec4: str r1, [r2]
00006ec6: mov.w r1, #0xa500
00006eca: strh r1, [r0]
00006ecc: movt r4, #0x1fff
00006ed0: mov.w r0, #0x170
00006ed4: strh r0, [r4]
00006ed6: movs r0, #0
00006ed8: mov.w r1, #0x170
00006edc: bl #0x702c
00006ee0: movs r0, #1
00006ee2: movw r1, #0xfdb0
00006ee6: bl #0x702c
00006eea: movs r0, #2
00006eec: movw r1, #0x1062
00006ef0: bl #0x702c
00006ef4: movs r0, #3
00006ef6: movs r1, #0x1e
00006ef8: bl #0x702c
00006efc: ldrh r0, [r4]
00006efe: orr r1, r0, #0x10
00006f02: movs r0, #0
00006f04: strh r1, [r4]
00006f06: bl #0x702c
00006f0a: movs r0, #2
00006f0c: bl #0x4cffc
00006f10: movs r0, #0
00006f12: bl #0x6f50
00006f16: ldrh r0, [r4]
00006f18: bic r1, r0, #0x10
00006f1c: movs r0, #0
00006f1e: strh r1, [r4]
00006f20: bl #0x702c
00006f24: movs r0, #2
00006f26: bl #0x4cffc
00006f2a: movs r0, #0
00006f2c: bl #0x6f50
00006f30: ldrh r0, [r4]
00006f32: orr r1, r0, #0x10
00006f36: movs r0, #0
00006f38: strh r1, [r4]
00006f3a: bl #0x702c
00006f3e: movs r0, #2
00006f40: bl #0x4cffc
00006f44: movs r0, #0
00006f46: pop.w {r4, lr}
00006f4a: b.w #0x6f50
00006f4e: movs r0, r0
00006f50: movw ip, #0x3808
00006f54: movt ip, #0x4005
00006f58: movw r2, #0xc400
00006f5c: movs r3, #0x80
00006f5e: movt r2, #0x4001
00006f62: strh.w r3, [ip, #2]
00006f66: ldr r3, [r2, #0x18]
00006f68: movs r1, #4
00006f6a: bfi r3, r1, #8, #4
00006f6e: str r3, [r2, #0x18]
00006f70: ldr r1, [r2, #0x14]
00006f72: lsls r0, r0, #2
00006f74: bic r1, r1, #1
00006f78: adds r0, #1
00006f7a: str r1, [r2, #0x14]
00006f7c: str r0, [r2]
00006f7e: nop
00006f80: ldr r0, [r2, #0x14]
00006f82: lsls r0, r0, #0x1e
00006f84: bmi #0x6f80
00006f86: ldr r0, [r2]
00006f88: ldr r0, [r2, #0x18]
00006f8a: movs r1, #0xc
00006f8c: bfi r0, r1, #8, #4
00006f90: str r0, [r2, #0x18]
00006f92: movs r0, #0
00006f94: str r0, [r2]
00006f96: nop
00006f98: ldr r0, [r2, #0x14]
00006f9a: lsls r0, r0, #0x1e
00006f9c: bmi #0x6f98
00006f9e: ldr r0, [r2]
00006fa0: movs r1, #0x80
00006fa2: uxth r0, r0
00006fa4: strh.w r1, [ip]
00006fa8: bx lr
00006faa: movs r0, r0
00006fac: movw ip, #0x3808
00006ffe: strh.w r1, [ip]
00007002: bx lr
00007004: push {r7, lr}
00007006: movw r1, #0xfdb0
0000700a: cmp r0, #2
0000700c: mov.w r0, #1
00007010: it hi
00007012: movwhi r1, #0xbb68
00007016: bl #0x702c
0000701a: movs r0, #0
0000701c: pop {r7, pc}
0000701e: push {r7, lr}
00007020: movs r0, #6
00007022: bl #0x6f50
00007026: sxth r0, r0
00007028: pop {r7, pc}
0000702a: movs r0, r0
0000702c: push {r7, lr}
0000702e: movw ip, #0x3808
00007032: movt ip, #0x4005
00007036: movw r3, #0xc400
0000703a: movs r2, #0x80
0000703c: movt r3, #0x4001
00007040: strh.w r2, [ip, #2]
00007044: ldr.w lr, [r3, #0x18]
00007048: movs r2, #4
0000704a: bfi lr, r2, #8, #4
0000704e: str.w lr, [r3, #0x18]
00007052: ldr r2, [r3, #0x14]
00007054: lsls r0, r0, #2
00007056: bic r2, r2, #1
0000705a: str r2, [r3, #0x14]
0000705c: str r0, [r3]
0000705e: nop
00007060: ldr r0, [r3, #0x14]
00007062: lsls r0, r0, #0x1e
00007064: bmi #0x7060
00007066: ldr r0, [r3]
00007068: ldr r0, [r3, #0x18]
0000706a: movs r2, #0xc
0000706c: bfi r0, r2, #8, #4
00007070: str r0, [r3, #0x18]
00007072: str r1, [r3]
00007074: ldr r0, [r3, #0x14]
00007076: lsls r0, r0, #0x1e
00007078: bmi #0x7074
0000707a: ldr r0, [r3]
0000707c: movs r0, #0x80
0000707e: strh.w r0, [ip]
00007082: pop {r7, pc}
00007084: movs r0, #0
00007086: b.w #0x10038
0000708a: movw r0, #0x955c
0000708e: movt r0, #0x1fff
00007092: ldr r0, [r0, #4]
00007094: cbz r0, #0x7098
00007096: bx r0
00007098: bx lr
