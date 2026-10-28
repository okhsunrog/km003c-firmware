//! Alternative complete decoders, compiled only for hardware comparisons.
use super::*;
use km003c_drivers::pd;
pub type Decode = fn(&[u16]) -> Result<Option<Event>, Error>;
unsafe extern "C" {
    fn km_pd_symbols_scalar(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
    fn km_pd_symbols_dsp_word(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
    fn km_pd_symbols_dsp_half(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
    fn km_pd_symbols_dsp4_word(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
    fn km_pd_symbols_dsp4_half(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
    fn km_pd_bytes_scalar(start: *const u16, end: *const u16, out: *mut u8, times: *mut u32)
    -> u32;
    fn km_pd_bytes_unroll(start: *const u16, end: *const u16, out: *mut u8, times: *mut u32)
    -> u32;
    fn km_pd_bytes_hoist(start: *const u16, end: *const u16, out: *mut u8, times: *mut u32) -> u32;
}
// Three additional kernels isolate byte unrolling and preamble skipping.
unsafe extern "C" {
    fn km_pd_bytes_pair(start: *const u16, end: *const u16, out: *mut u8, times: *mut u32) -> u32;
    fn km_pd_bytes_preamble(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
    fn km_pd_bytes_hoist_preamble(
        start: *const u16,
        end: *const u16,
        out: *mut u8,
        times: *mut u32,
    ) -> u32;
}
pub fn unaligned_enabled() -> bool {
    unsafe { (0xe000ed14 as *const u32).read_volatile() & 8 == 0 }
}
#[inline(always)]
fn dsp_pair(x: u32) -> bool {
    let (above, below, diff): (u32, u32, u32);
    unsafe {
        core::arch::asm!("uqsub16 {a}, {x}, {hi}", "uqsub16 {b}, {lo}, {x}", "orr {d}, {a}, {b}",
            x=in(reg) x, hi=in(reg) 0x00190019u32, lo=in(reg) 0x000f000fu32,
            a=out(reg) above, b=out(reg) below, d=out(reg) diff, options(pure, nomem, nostack));
    }
    let _ = (above, below);
    diff == 0
}
#[unsafe(link_section = ".pd_fast.burst")]
#[inline(never)]
fn burst(data: &[u16]) -> Result<Option<Event>, Error> {
    pd::burst::decode(data, 20, 5)
}
#[unsafe(link_section = ".pd_fast.filtered")]
#[inline(never)]
fn filtered(data: &[u16]) -> Result<Option<Event>, Error> {
    pd::burst::decode_configured::<true>(data, 20, 5, |_| {})
}
#[inline(never)]
fn flash(data: &[u16]) -> Result<Option<Event>, Error> {
    pd::burst::decode(data, 20, 5)
}
#[unsafe(link_section = ".pd_fast.stream")]
#[inline(never)]
fn streaming(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut d = pd::Decoder::new(20, 5).unwrap();
    for &n in data {
        let in_packet = d.in_packet();
        match d.push_interval(n) {
            Ok(Some(e)) => return Ok(Some(e)),
            Err(e) if in_packet => return Err(e),
            _ => (),
        }
    }
    d.finish()?;
    Ok(None)
}
#[unsafe(link_section = ".pd_fast.symbols")]
#[inline(never)]
fn symbol_rust(data: &[u16]) -> Result<Option<Event>, Error> {
    symbols::decode::<0, true>(data, symbols::scalar_pair)
}
#[unsafe(link_section = ".pd_fast.paired")]
#[inline(never)]
fn paired_rust(data: &[u16]) -> Result<Option<Event>, Error> {
    symbols::decode::<1, true>(data, symbols::scalar_pair)
}
#[unsafe(link_section = ".pd_fast.dsp")]
#[inline(never)]
fn paired_dsp(data: &[u16]) -> Result<Option<Event>, Error> {
    symbols::decode::<1, true>(data, dsp_pair)
}

// Each scanner gets exactly CAPACITY writable bytes and an exclusive input end.
// The word-load variant additionally requires Cortex-M unaligned SRAM accesses.
#[unsafe(link_section = ".pd_fast.asm_scalar")]
#[inline(never)]
fn asm_scalar(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; symbols::CAPACITY];
    let status = unsafe {
        km_pd_symbols_scalar(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_word")]
#[inline(never)]
fn asm_word(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; symbols::CAPACITY];
    let status = unsafe {
        km_pd_symbols_dsp_word(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_half")]
#[inline(never)]
fn asm_half(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; symbols::CAPACITY];
    let status = unsafe {
        km_pd_symbols_dsp_half(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_four_word")]
#[inline(never)]
fn asm_four_word(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; symbols::CAPACITY];
    let status = unsafe {
        km_pd_symbols_dsp4_word(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_four_half")]
#[inline(never)]
fn asm_four_half(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; symbols::CAPACITY];
    let status = unsafe {
        km_pd_symbols_dsp4_half(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_bytes")]
#[inline(never)]
fn asm_bytes(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; 35];
    let status = unsafe {
        km_pd_bytes_scalar(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate_bytes(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_unroll")]
#[inline(never)]
fn asm_unroll(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; 35];
    let status = unsafe {
        km_pd_bytes_unroll(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate_bytes(status, &out)
}
#[unsafe(link_section = ".pd_fast.asm_hoist")]
#[inline(never)]
fn asm_hoist(data: &[u16]) -> Result<Option<Event>, Error> {
    let mut out = [0; 35];
    let status = unsafe {
        km_pd_bytes_hoist(
            data.as_ptr(),
            data.as_ptr().add(data.len()),
            out.as_mut_ptr(),
            core::ptr::null_mut(),
        )
    };
    symbols::validate_bytes(status, &out)
}
macro_rules! byte_candidate {
    ($name:ident, $kernel:ident) => {
        #[unsafe(link_section = concat!(".pd_fast.", stringify!($name)))]
        #[inline(never)]
        fn $name(data: &[u16]) -> Result<Option<Event>, Error> {
            let mut out = [0; 35];
            let status = unsafe {
                $kernel(
                    data.as_ptr(),
                    data.as_ptr().add(data.len()),
                    out.as_mut_ptr(),
                    core::ptr::null_mut(),
                )
            };
            symbols::validate_bytes(status, &out)
        }
    };
}
byte_candidate!(asm_byte_pair, km_pd_bytes_pair);
byte_candidate!(asm_preamble, km_pd_bytes_preamble);
byte_candidate!(asm_hoist_preamble, km_pd_bytes_hoist_preamble);
pub const CANDIDATES: &[(&str, Decode)] = &[
    ("streaming", streaming),
    ("burst", burst),
    ("burst_flash", flash),
    ("burst_filter", filtered),
    ("symbols_rust", symbol_rust),
    ("paired_rust", paired_rust),
    ("paired_dsp", paired_dsp),
    ("asm_scalar", asm_scalar),
    ("asm_dsp_half", asm_half),
    ("asm_dsp_word", asm_word),
    ("asm_dsp4_word", asm_four_word),
    ("asm_dsp4_half", asm_four_half),
    ("asm_fused", asm_bytes),
    ("asm_unroll", asm_unroll),
    ("asm_hoist", asm_hoist),
    ("asm_byte_pair", asm_byte_pair),
    ("asm_preamble", asm_preamble),
    ("asm_hoist_preamble", asm_hoist_preamble),
];
