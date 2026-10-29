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
