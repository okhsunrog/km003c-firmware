/* Bounded BMC/4b5b decoder with confirmed-preamble skipping.
 * r0=start, r1=end exclusive, r2=35-byte output, r3=optional DWT timestamps.
 * Rust validates packet shape and CRC before delivery. */
.syntax unified
.thumb
.p2align 2

.macro stamp offset
    cmp r11, #0
    beq .Lnostamp\@
    movw r3, #0x1004
    movt r3, #0xe000
    ldr r3, [r3]
    str r3, [r11, #\offset]
.Lnostamp\@:
.endm


.macro next_bit done, timing, empty
    cmp r0, r1
    bhs \empty
    ldrh r3, [r0], #2
    sub.w r12, r3, #15
    cmp r12, #10
    bls .Lshort\@
    sub.w r12, r3, #30
    cmp r12, #20
    bhi \timing
    movs r3, #0
    b \done
.Lshort\@:
    cmp r0, r1
    bhs \empty
    ldrh r3, [r0], #2
    sub.w r12, r3, #15
    cmp r12, #10
    bhi \timing
    movs r3, #1
.endm


.macro direct_bit mask, bounded, timing, empty
.if \bounded
    cmp r0, r1
    bhs \empty
.endif
    ldrh r3, [r0], #2
    sub.w r12, r3, #15
    cmp r12, #10
    bls .Ldirect_short\@
    sub.w r12, r3, #30
    cmp r12, #20
    bhi \timing
    b .Ldirect_done\@
.Ldirect_short\@:
.if \bounded
    cmp r0, r1
    bhs \empty
.endif
    ldrh r3, [r0], #2
    sub.w r12, r3, #15
    cmp r12, #10
    bhi \timing
    orr r4, r4, #\mask
.Ldirect_done\@:
.endm


.macro preamble_pair first, second, third, repeat, failed

    sub.w r12, r1, r0
    cmp r12, #6
    blo \failed
    ldrh r3, [r0]
    sub.w r12, r3, #\first
    cmp r12, #(\first / 3 * 2)
    bhi \failed
    ldrh r3, [r0, #2]
    sub.w r12, r3, #\second
    cmp r12, #(\second / 3 * 2)
    bhi \failed
    ldrh r3, [r0, #4]
    sub.w r12, r3, #\third
    cmp r12, #(\third / 3 * 2)
    bhi \failed
    adds r0, #6
    b \repeat
.endm

.macro scanner name
.section .pd_fast.\name,"ax",%progbits
.p2align 2
.global \name
.hidden \name
.type \name,%function
.thumb_func
\name:
    push {r4-r11, lr}
    mov r11, r3
    stamp 0
    movw r6, #0x0040
    movt r6, #0x0202
.Lreset\@:
    movs r4, #0
    movs r5, #20
.Lprefix\@:
    next_bit .Lprefix_bit\@, .Lreset\@, .Lnone\@
.Lprefix_bit\@:
    lsr.w r4, r4, #1
    orr.w r4, r4, r3, lsl #19
    cbz r5, .Lcheck\@
    subs r5, #1
    bne .Lprefix\@
    movw r3, #0x5555
    movt r3, #0x0005
    cmp r4, r3
    beq .Lssl\@
    movw r3, #0xaaaa
    movt r3, #0x000a
    cmp r4, r3
    bne .Lcheck\@
.Llss\@:
    preamble_pair 30, 15, 15, .Llss\@, .Lprefix\@
.Lssl\@:
    preamble_pair 15, 15, 30, .Lssl\@, .Lprefix\@
.Lcheck\@:
    lsr.w r3, r4, #15
    lsr.w r12, r6, r3
    tst r12, #1
    beq .Lprefix\@
    adr r12, .Lwords\@
    mov.w r10, #0
.Lfind\@:
    ldr.w r3, [r12, r10, lsl #2]
    cmp r4, r3
    beq .Lfound\@
    add.w r10, r10, #1
    cmp r10, #7
    blo .Lfind\@
    b .Lprefix\@
.Lfound\@:
    cmp r10, #5
    bhs .Lreset_event\@
    stamp 4
    movs r4, #16
    movs r7, #0
    movs r5, #0
.Lpayload\@:

    sub.w r12, r1, r0
    cmp r12, #40
    blo .Lsymbol_fast\@
    movs r4, #0
    direct_bit 1, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 2, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 4, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 8, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 16, 0, .Ltiming\@, .Ltruncated\@
    adds r7, #1
    cmp r4, #13
    beq .Lbyte_eop\@
    adr r12, .Lnibbles\@
    ldrb r6, [r12, r4]
    cmp r6, #255
    beq .Lbad_symbol\@
    cmp r7, #69
    bhs .Lfull\@
    movs r4, #0
    direct_bit 1, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 2, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 4, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 8, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 16, 0, .Ltiming\@, .Ltruncated\@
    adds r7, #1
    cmp r4, #13
    beq .Lbyte_eop\@
    adr r12, .Lnibbles\@
    ldrb r3, [r12, r4]
    cmp r3, #255
    beq .Lbad_symbol\@
    orr.w r3, r6, r3, lsl #4
    strb r3, [r2], #1
    cmp r7, #69
    bhs .Lfull\@
    movs r4, #16
    b .Lpayload\@
.Lsymbol_fast\@:
    sub.w r12, r1, r0
    cmp r12, #20
    blo .Lbounded_tail\@
    movs r4, #0
    direct_bit 1, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 2, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 4, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 8, 0, .Ltiming\@, .Ltruncated\@
    direct_bit 16, 0, .Ltiming\@, .Ltruncated\@
    b .Lcomplete\@
.Lbounded_tail\@:
    next_bit .Lpayload_bit\@, .Ltiming\@, .Ltruncated\@
.Lpayload_bit\@:

    lsrs r4, r4, #1
    orr.w r4, r4, r3, lsl #4
    bcc .Lpayload\@
.Lcomplete\@:
    adds r7, #1
    cmp r4, #13
    beq .Lbyte_eop\@
    adr r12, .Lnibbles\@
    ldrb r3, [r12, r4]
    cmp r3, #255
    beq .Lbad_symbol\@
    cbnz r5, .Lhigh\@
    mov r6, r3
    movs r5, #1
    b .Lbyte_ready\@
.Lhigh\@:
    orr.w r3, r6, r3, lsl #4
    strb r3, [r2], #1
    movs r5, #0
.Lbyte_ready\@:
    cmp r7, #69
    bhs .Lfull\@
    movs r4, #16
    b .Lpayload\@
.Lbyte_eop\@:
    strb r4, [r2]
    b .Lpacket\@
.Lbad_symbol\@:
    movw r0, #0
    movt r0, #0x8001
    orr r0, r0, r4
    pop {r4-r11, pc}
.Lpacket\@:
    orr.w r0, r7, r10, lsl #8
    pop {r4-r11, pc}
.Lreset_event\@:
    lsl.w r0, r10, #8
    pop {r4-r11, pc}
.Lnone\@:
    mov.w r0, #0x40000000
    pop {r4-r11, pc}
.Ltiming\@:
    movw r0, #0
    movt r0, #0x8000
    pop {r4-r11, pc}
.Ltruncated\@:
    movw r0, #1
    movt r0, #0x8000
    pop {r4-r11, pc}
.Lfull\@:
    movw r0, #2
    movt r0, #0x8000
    pop {r4-r11, pc}
.p2align 2
.Lwords\@:
    .word KM_PD_SOP0, KM_PD_SOP1, KM_PD_SOP2, KM_PD_SOP3, KM_PD_SOP4, KM_PD_SOP5, KM_PD_SOP6
.Lnibbles\@:
    .word KM_PD_N0, KM_PD_N1, KM_PD_N2, KM_PD_N3, KM_PD_N4, KM_PD_N5, KM_PD_N6, KM_PD_N7
.size \name, .-\name
.endm

scanner km_pd_bytes_preamble
