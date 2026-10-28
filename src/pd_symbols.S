/* Bounded Thumb-2 scanners. Input: r0=start, r1=end (exclusive), r2=69-byte
 * symbol output (35-byte decoded output for modes >=5), r3=optional
 * [start, after-SOP] DWT timestamps. Return scanner
 * status (see pd::symbols). No packet is delivered before Rust checks its CRC.
 * All loads are bounded before access; only DSP-word uses unaligned LDR, which
 * requires CCR.UNALIGN_TRP=0 and is checked by its Rust caller.
 * Modes 0..2 compare scalar / packed two-lane symbols, 3..4 four-lane DSP,
 * 5 fuses bytes, 6 unrolls symbols, 7 hoists bounds, 8 unrolls bytes,
 * 9 adds confirmed-preamble skipping (production), 10 skips preamble with 7.
 * Each section is collected only if used: production links only mode 9.
 */
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

/* Produce bit in r3. DSP modes first test two halfwords in parallel.
 * Scratch r12/lr; r8/r9 hold lane-wise short thresholds 15 and 25. */
.macro next_bit mode, done, timing, empty
.if (\mode == 1) || (\mode == 2)
    sub.w r12, r1, r0
    cmp r12, #4
    blo .Lscalar\@
.if \mode == 1
    ldr r3, [r0]
.else
    ldrh r3, [r0]
    ldrh r12, [r0, #2]
    orr.w r3, r3, r12, lsl #16
.endif
    uqsub16 lr, r3, r9
    uqsub16 r12, r8, r3
    orrs.w r12, r12, lr
    bne .Lnotpair\@
    adds r0, #4
    movs r3, #1
    b \done
.Lnotpair\@:
    uxth r3, r3
    sub.w r12, r3, #30
    cmp r12, #20
    bhi .Lbadpair\@
    adds r0, #2
    movs r3, #0
    b \done
.Lbadpair\@:
    sub.w r12, r3, #15
    cmp r12, #10
    ite ls
    addls r0, #4
    addhi r0, #2
    b \timing
.Lscalar\@:
.endif
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

/* One unrolled BMC bit: only SS needs to set a symbol bit. Bounded mode checks
 * every halfword; fast mode is entered only with 20 bytes left for five bits. */
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

/* Skip two confirmed alternating preamble bits (SS,L or L,SS). All loads are
 * bounded and speculative: failure resumes normal scanning without consuming
 * anything. The last 20 bits remain exactly 0x55555 / 0xaaaaa after each pair. */
.macro preamble_pair first, second, third, repeat, failed
    /* Lower bounds 15/30 imply window widths 10/20 respectively. */
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

.macro scanner name, mode
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
.if (\mode == 1) || (\mode == 2)
    mov.w r8, #0x000f000f
    mov.w r9, #0x00190019
.endif
    movw r6, #0x0040
    movt r6, #0x0202
.Lreset\@:
    movs r4, #0
    movs r5, #20
.Lprefix\@:
.if \mode < 3
    next_bit \mode, .Lprefix_bit\@, .Lreset\@, .Lnone\@
.else
    next_bit 0, .Lprefix_bit\@, .Lreset\@, .Lnone\@
.endif
.Lprefix_bit\@:
    lsr.w r4, r4, #1
    orr.w r4, r4, r3, lsl #19
    cbz r5, .Lcheck\@
    subs r5, #1
    bne .Lprefix\@
.if \mode >= 9
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
.endif
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
.if (\mode == 3) || (\mode == 4)
    /* Four short/long classifications per pass. r4 is a bit reservoir,
     * r5 its length. A LUT parses L / SS without crossing a partial bit. */
    movs r4, #0
    movs r5, #0
.Lbatch\@:
    sub.w r12, r1, r0
    cmp r12, #8
    blo .Lbatch_tail\@
.if \mode == 3
    ldr r3, [r0]
    ldr r8, [r0, #4]
.else
    ldrh r3, [r0]
    ldrh r12, [r0, #2]
    orr.w r3, r3, r12, lsl #16
    ldrh r8, [r0, #4]
    ldrh r12, [r0, #6]
    orr.w r8, r8, r12, lsl #16
.endif
    orr.w r12, r3, r8
    tst r12, #0xff00ff00
    bne .Lbatch_tail\@
    /* Byte order is a,c,b,d. First reject values outside [15,50]. */
    orr.w r3, r3, r8, lsl #8
    mov.w r12, #0x0f0f0f0f
    uqsub8 lr, r12, r3
    mov.w r12, #0x32323232
    uqsub8 r12, r3, r12
    orrs.w r12, r12, lr
    bne .Lbatch_tail\@
    /* GE lanes encode long >=30 and short <=25; reject the gap. */
    mov.w r12, #0x1e1e1e1e
    usub8 r12, r3, r12
    mrs lr, APSR
    mov.w r12, #0x19191919
    usub8 r12, r12, r3
    mrs r8, APSR
    orr.w r12, lr, r8
    ubfx r12, r12, #16, #4
    cmp r12, #15
    bne .Lbatch_tail\@
    ubfx r8, r8, #16, #4
    adr r12, .Lbatch_lut\@
    ldrh.w r9, [r12, r8, lsl #1]
    cmp r9, #0
    beq .Lbatch_tail\@
    lsr.w r12, r9, #8
    add r0, r0, r12
    ubfx r8, r9, #4, #3
    and r3, r9, #15
    b .Lappend\@
.Lbatch_tail\@:
    next_bit 0, .Lone_bit\@, .Ltiming\@, .Ltruncated\@
.Lone_bit\@:
    mov.w r8, #1
.Lappend\@:
    lsl.w r3, r3, r5
    orrs r4, r3
    add r5, r5, r8
    cmp r5, #5
    blo .Lbatch\@
    and r6, r4, #31
    strb r6, [r2], #1
    adds r7, #1
    lsr.w r4, r4, #5
    subs r5, #5
    cmp r6, #13
    beq .Lpacket\@
    cmp r7, #69
    bhs .Lfull\@
    b .Lbatch\@
.else
.if \mode >= 5
    movs r5, #0 /* nibble parity; r6 holds the low nibble */
.endif
.Lpayload\@:
.if (\mode == 8) || (\mode == 9)
    /* Two unrolled nibbles eliminate the low/high parity state. The 40-byte
     * guard covers ten SS bits. The ordinary symbol path handles the tail. */
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
.endif
.if \mode >= 6
.if \mode >= 7
    sub.w r12, r1, r0
    cmp r12, #20
    blo .Lbounded_tail\@
.endif
    movs r4, #0
    direct_bit 1, (\mode == 6), .Ltiming\@, .Ltruncated\@
    direct_bit 2, (\mode == 6), .Ltiming\@, .Ltruncated\@
    direct_bit 4, (\mode == 6), .Ltiming\@, .Ltruncated\@
    direct_bit 8, (\mode == 6), .Ltiming\@, .Ltruncated\@
    direct_bit 16, (\mode == 6), .Ltiming\@, .Ltruncated\@
    b .Lcomplete\@
.Lbounded_tail\@:
.endif
.if \mode < 3
    next_bit \mode, .Lpayload_bit\@, .Ltiming\@, .Ltruncated\@
.else
    next_bit 0, .Lpayload_bit\@, .Ltiming\@, .Ltruncated\@
.endif
.Lpayload_bit\@:
    /* The sentinel reaches carry after five bits. ORR.W preserves carry. */
    lsrs r4, r4, #1
    orr.w r4, r4, r3, lsl #4
    bcc .Lpayload\@
.Lcomplete\@:
    adds r7, #1
.if \mode >= 5
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
.else
    strb r4, [r2], #1
    cmp r4, #13
    beq .Lpacket\@
.endif
    cmp r7, #69
    bhs .Lfull\@
    movs r4, #16
    b .Lpayload\@
.if \mode >= 5
.Lbyte_eop\@:
    strb r4, [r2]
    b .Lpacket\@
.Lbad_symbol\@:
    movw r0, #0
    movt r0, #0x8001
    orr r0, r0, r4
    pop {r4-r11, pc}
.endif
.endif
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
.if (\mode == 3) || (\mode == 4)
.Lbatch_lut\@:
    .hword KM_PD_B0, KM_PD_B1, KM_PD_B2, KM_PD_B3, KM_PD_B4, KM_PD_B5, KM_PD_B6, KM_PD_B7
    .hword KM_PD_B8, KM_PD_B9, KM_PD_B10, KM_PD_B11, KM_PD_B12, KM_PD_B13, KM_PD_B14, KM_PD_B15
.endif
.if \mode >= 5
.Lnibbles\@:
    .word KM_PD_N0, KM_PD_N1, KM_PD_N2, KM_PD_N3, KM_PD_N4, KM_PD_N5, KM_PD_N6, KM_PD_N7
.endif
.size \name, .-\name
.endm

scanner km_pd_symbols_scalar, 0
scanner km_pd_symbols_dsp_word, 1
scanner km_pd_symbols_dsp_half, 2
scanner km_pd_symbols_dsp4_word, 3
scanner km_pd_symbols_dsp4_half, 4
scanner km_pd_bytes_scalar, 5
scanner km_pd_bytes_unroll, 6
scanner km_pd_bytes_hoist, 7
scanner km_pd_bytes_pair, 8
scanner km_pd_bytes_preamble, 9
scanner km_pd_bytes_hoist_preamble, 10
