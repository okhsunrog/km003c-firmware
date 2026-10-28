//! Measured Cortex-M4 completed-burst decoder. The bounded Thumb-2 scanner
//! decodes BMC/4b5b; Rust checks EOP, NDO and CRC before delivering a packet.
use km003c_drivers::pd::{Error, Event, symbols};
core::arch::global_asm!(
    ".equ KM_PD_SOP0, {a}", ".equ KM_PD_SOP1, {b}", ".equ KM_PD_SOP2, {c}",
    ".equ KM_PD_SOP3, {d}", ".equ KM_PD_SOP4, {e}", ".equ KM_PD_SOP5, {f}", ".equ KM_PD_SOP6, {g}",
    a=const symbols::WORDS[0], b=const symbols::WORDS[1], c=const symbols::WORDS[2],
    d=const symbols::WORDS[3], e=const symbols::WORDS[4], f=const symbols::WORDS[5], g=const symbols::WORDS[6],
);
core::arch::global_asm!(include_str!("pd_symbols.S"), options(raw));
core::arch::global_asm!(
    ".equ KM_PD_B0,{a}",".equ KM_PD_B1,{b}",".equ KM_PD_B2,{c}",".equ KM_PD_B3,{d}",
    ".equ KM_PD_B4,{e}",".equ KM_PD_B5,{f}",".equ KM_PD_B6,{g}",".equ KM_PD_B7,{h}",
    ".equ KM_PD_B8,{i}",".equ KM_PD_B9,{j}",".equ KM_PD_B10,{k}",".equ KM_PD_B11,{l}",
    ".equ KM_PD_B12,{m}",".equ KM_PD_B13,{n}",".equ KM_PD_B14,{o}",".equ KM_PD_B15,{p}",
    a=const symbols::BATCH[0],b=const symbols::BATCH[1],c=const symbols::BATCH[2],d=const symbols::BATCH[3],
    e=const symbols::BATCH[4],f=const symbols::BATCH[5],g=const symbols::BATCH[6],h=const symbols::BATCH[7],
    i=const symbols::BATCH[8],j=const symbols::BATCH[9],k=const symbols::BATCH[10],l=const symbols::BATCH[11],
    m=const symbols::BATCH[12],n=const symbols::BATCH[13],o=const symbols::BATCH[14],p=const symbols::BATCH[15],
);
const fn nibble_word(i: usize) -> u32 {
    u32::from_le_bytes([
        symbols::NIBBLES[4 * i],
        symbols::NIBBLES[4 * i + 1],
        symbols::NIBBLES[4 * i + 2],
        symbols::NIBBLES[4 * i + 3],
    ])
}
core::arch::global_asm!(
    ".equ KM_PD_N0,{a}",".equ KM_PD_N1,{b}",".equ KM_PD_N2,{c}",".equ KM_PD_N3,{d}",
    ".equ KM_PD_N4,{e}",".equ KM_PD_N5,{f}",".equ KM_PD_N6,{g}",".equ KM_PD_N7,{h}",
    a=const nibble_word(0),b=const nibble_word(1),c=const nibble_word(2),d=const nibble_word(3),
    e=const nibble_word(4),f=const nibble_word(5),g=const nibble_word(6),h=const nibble_word(7),
);
unsafe extern "C" {
    fn km_pd_bytes_preamble(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
}

/// Native capture intervals after the unknown first duration, in 12 MHz timer
/// ticks. The scanner has fixed short=15..25 and long=30..50 timing windows.
/// Output is bounded to 34 bytes plus EOP. Profiling stays outside tight loops.
#[unsafe(link_section = ".pd_fast.accepted")]
#[inline(never)]
pub fn decode_observed(data: &[u16], stamp: &mut [u32; 5]) -> Result<Option<Event>, Error> {
    let mut output = [0; 35];
    // The leaf kernel writes only output[0..35] and stamp[0..2]. It checks all
    // input loads, including worst-case guards before unrolled five-bit groups.
    // Halfword loads work with any u16 slice; no unaligned word access is used.
    let status = unsafe {
        km_pd_bytes_preamble(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            output.as_mut_ptr(),
            stamp.as_mut_ptr(),
        )
    };
    let result = symbols::validate_bytes_observed(status, &output, |phase| {
        stamp[phase as usize] = unsafe { (0xe0001004 as *const u32).read_volatile() };
    });
    stamp[4] = unsafe { (0xe0001004 as *const u32).read_volatile() };
    result
}

#[cfg(feature = "pd-bench")]
mod bench;
#[cfg(feature = "pd-bench")]
pub use bench::{CANDIDATES, unaligned_enabled};
