0001df4c: and r0, r0, #3
0001df50: cmp r0, #3
0001df52: bne #0x1dfe8
0001df54: movs r0, #1
0001df56: bl #0x4cffc
0001df5a: bl #0x6e54
0001df5e: mov r6, r0
0001df60: bl #0x6d78
0001df64: mov r7, r0
0001df66: movs r0, #0x64
0001df68: muls r0, r6, r0
0001df6a: lsrs r4, r0, #9
0001df6c: movw r0, #0x4dd3
0001df70: movt r0, #0x1062
0001df74: smmul r0, r7, r0
0001df78: asrs r1, r0, #6
0001df7a: add.w fp, r1, r0, lsr #31
0001df7e: bl #0x484ce
0001df82: bl #0x483ca
0001df86: mov r5, r0
0001df88: bl #0x483d6
0001df8c: movw r1, #0xb22d
0001df90: movt r1, #0xef9d
0001df94: smmul r0, r0, r1
0001df98: asrs r1, r0, #6
0001df9a: add.w r0, r1, r0, lsr #31
0001df9e: ldr r1, [r5]
0001dfa0: movw r2, #0x851f
0001dfa4: movt r2, #0x51eb
0001dfa8: smmul r1, r1, r2
0001dfac: asrs r2, r1, #5
0001dfae: add r0, fp
0001dfb0: add.w r1, r2, r1, lsr #31
0001dfb4: muls r0, r1, r0
0001dfb6: movw r1, #0x8bad
0001dfba: movt r1, #0x68db
0001dfbe: smmul r0, r0, r1
0001dfc2: lsrs r1, r0, #0xc
0001dfc4: add.w r0, r1, r0, lsr #31
0001dfc8: strh.w r0, [sb, #0x6a]
0001dfcc: movw r0, #0xc35
0001dfd0: muls r0, r6, r0
0001dfd2: asrs r0, r0, #4
0001dfd4: mov r1, r7
0001dfd6: strh.w r4, [sb, #0x68]
0001dfda: bl #0x17b1c
0001dfde: mov.w sl, #0
0001dfe2: mov r5, sb
0001dfe4: b #0x1e124
0001dfe6: nop 
0001dfe8: movs r0, #2
0001dfea: bl #0x4cffc
0001dfee: bl #0x6e54
0001dff2: mov r6, r0
0001dff4: bl #0x6d78
0001dff8: mov r7, r0
0001dffa: movs r0, #0x64
0001dffc: muls r0, r6, r0
0001dffe: lsrs r4, r0, #9
0001e000: movw r0, #0x4dd3
0001e004: movt r0, #0x1062
0001e008: smmul r0, r7, r0
0001e00c: asrs r1, r0, #6
0001e00e: add.w fp, r1, r0, lsr #31
0001e012: bl #0x484ce
0001e016: bl #0x483ca
0001e01a: mov r5, r0
0001e01c: bl #0x483d6
0001e020: movw r1, #0xb22d
0001e024: movt r1, #0xef9d
0001e028: smmul r0, r0, r1
0001e02c: asrs r1, r0, #6
0001e02e: add.w r0, r1, r0, lsr #31
0001e032: ldr r1, [r5]
0001e034: movw r2, #0x851f
0001e038: movt r2, #0x51eb
0001e03c: smmul r1, r1, r2
0001e040: asrs r2, r1, #5
0001e042: add r0, fp
0001e044: add.w r1, r2, r1, lsr #31
0001e048: muls r0, r1, r0
0001e04a: movw r1, #0x8bad
0001e04e: movt r1, #0x68db
0001e052: smmul r0, r0, r1
0001e056: strh.w r4, [sb, #0x68]
0001e05a: lsrs r1, r0, #0xc
0001e05c: movw r4, #0xb064
0001e060: add.w r0, r1, r0, lsr #31
0001e064: movt r4, #0x1fff
0001e068: strh.w r0, [sb, #0x6a]
0001e06c: mov r0, r4
0001e06e: mov r1, r6
0001e070: bl #0x20c0e
0001e074: asrs r0, r7, #0x1f
0001e076: movw r5, #0xb044
0001e07a: add.w r0, r7, r0, lsr #30
0001e07e: movt r5, #0x1fff
0001e082: asrs r1, r0, #2
0001e084: mov r0, r5
0001e086: bl #0x20c0e
0001e08a: ldr r1, [r4]
0001e08c: mov r0, r4
0001e08e: bl #0x20c48
0001e092: ldr r1, [r5]
0001e094: mov r0, r5
0001e096: bl #0x20c48
0001e09a: ldrb r0, [r4, #0x1c]
0001e09c: ldrb r1, [r4, #0x1d]
0001e09e: mov.w sl, #0
