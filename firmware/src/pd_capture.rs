//! PD receiver using comparator events, TimerA and DMA. PA0/PA1 select CC1/CC2.
//! The receiver never drives CC; the transmitter borrows the stopped timer and
//! restores analog input mode before capture resumes.
use embassy_hc32::{
    dma::{self, Channel, Unit},
    intc::{Line, Route},
    pac::interrupt,
};

const TMRA: usize = 0x40015400;
const CMP: usize = 0x4004a000;
const DCU: usize = 0x40052000;
pub(crate) fn read8(address: usize) -> u8 {
    unsafe { (address as *const u8).read_volatile() }
}
pub(crate) fn write8(address: usize, value: u8) {
    unsafe { (address as *mut u8).write_volatile(value) }
}
pub(crate) fn read16(address: usize) -> u16 {
    unsafe { (address as *const u16).read_volatile() }
}
pub(crate) fn write16(address: usize, value: u16) {
    unsafe { (address as *mut u16).write_volatile(value) }
}
fn read32(address: usize) -> u32 {
    unsafe { (address as *const u32).read_volatile() }
}
fn write32(address: usize, value: u32) {
    unsafe { (address as *mut u32).write_volatile(value) }
}

/// CMP1_INP1/2 reach CC1/CC2 on the hardware-tested HW2.1 board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Cmp1Inp1,
    Cmp1Inp2,
}
impl Input {
    pub fn number(self) -> u8 {
        match self {
            Self::Cmp1Inp1 => 1,
            Self::Cmp1Inp2 => 2,
        }
    }
}
#[derive(Debug)]
pub enum Error {
    Dma(dma::Error),
    Routing(embassy_hc32::intc::IrqError),
}
pub(crate) fn stop_events() {
    // Disable edge events and the physical VCOUT port, retain internal comparator
    // output only. Then stop the timer and disable/clear overflow IRQ state.
    write16(CMP, read16(CMP) & !0x1060);
    write16(TMRA + 0x80, read16(TMRA + 0x80) & !1);
    write8(TMRA + 0x81, read8(TMRA + 0x81) & !0x50);
}
#[interrupt]
fn INT008() {
    if !crate::pd_tx::on_interrupt(0) {
        crate::pd_buffered::on_interrupt(0);
    }
}
#[interrupt]
fn INT016() {
    crate::pd_tx::on_interrupt(1);
}
#[interrupt]
fn INT017() {
    crate::pd_buffered::on_interrupt(1);
}
#[interrupt]
fn INT018() {
    if !crate::pd_tx::on_interrupt(2) {
        crate::pd_buffered::on_interrupt(2);
    }
}

pub struct Receiver {
    _channel: Channel,
    _routes: [Route; 4],
}
impl Receiver {
    /// Switch only the internal comparator mux between captures. Event/output
    /// drivers stay disabled; this does not change termination or any GPIO.
    pub fn select_input(&mut self, input: Input) {
        stop_events();
        let positive = match input {
            Input::Cmp1Inp1 => 0x100,
            Input::Cmp1Inp2 => 0x200,
        };
        write16(CMP + 2, 0x4004 | positive);
    }

    /// # Safety
    /// Own TMRA2, CMP1/DAC1, DCU1, **all DMA1 channels**, corresponding AOS
    /// trigger selectors and IRQ8/16/17/18. Configure the
    /// analog input pad before calling. No task/IRQ may alter those resources.
    /// Keep all CC transmit/termination GPIO outputs disabled separately, except
    /// when the exclusive board TX owner pauses RX and performs its handoff.
    pub unsafe fn new(input: Input, dac_code: u8) -> Result<Self, Error> {
        let [_, _, channel, _] = unsafe { dma::split(Unit::Dma1) }.map_err(Error::Dma)?;
        // First stop inherited event producers before routing CPU interrupts.
        write32(0x40048008, read32(0x40048008) & !8); // FCG2.TMRA2
        write32(0x4004800c, read32(0x4004800c) & !0x110); // FCG3.CMP/DAC
        stop_events();
        write16(CMP, 0); // Comparator off while configuring its reference/input.
        write32(0x40048010, 0xa5a50001);
        write32(0x40048000, read32(0x40048000) & !(1 << 24)); // DCU1
        write32(0x40048010, 0xa5a50000);
        let routes = unsafe {
            [
                Route::new(0x103, Line::new(8).unwrap(), 5).map_err(Error::Routing)?,
                Route::new(Unit::Dma1.complete_event(1).0, Line::new(16).unwrap(), 5)
                    .map_err(Error::Routing)?,
                Route::new(Unit::Dma1.complete_event(2).0, Line::new(17).unwrap(), 5)
                    .map_err(Error::Routing)?,
                Route::new(Unit::Dma1.error_event().0, Line::new(18).unwrap(), 5)
                    .map_err(Error::Routing)?,
            ]
        };
        // Timer counts up, PCLK1/8. Native event starts AND clears the timer,
        // and channel 3 captures each CMP1 event, so samples are edge intervals.
        write16(TMRA + 0x80, 0x32);
        write8(TMRA + 0x81, 0);
        write16(TMRA + 0x84, 0x404); // HSTA2 + HCLE2, common event
        write16(TMRA + 0x88, 0); // Disable alternate count sources.
        write16(TMRA + 0x8c, 0);
        write16(TMRA + 0x94, 4); // Capture IRQ source enable used by native path.
        write16(TMRA + 0x108, 0x4041); // CCONR3: capture mode, common event
        // Never enable the timer's external waveform outputs (PCONR1..8).
        for channel in 0..8 {
            write16(TMRA + 0x140 + 4 * channel, 0);
        }
        write32(0x4001084c, 0x1a0);
        write32(0x40010850, 0x1a0);
        write32(DCU, 0x0b); // 16-bit hardware addition, DATA0 += DATA1
        write32(DCU + 12, 1);
        write32(0x40010804, 0x105); // Capture event -> DCU1
        // DAC1 is shared by comparators: caller explicitly chooses its code.
        unsafe {
            (0x4004a100 as *mut u8).write_volatile(dac_code);
        }
        write16(0x4004a108, read16(0x4004a108) | 1);
        let positive = match input {
            Input::Cmp1Inp1 => 0x100,
            Input::Cmp1Inp2 => 0x200,
        };
        write16(CMP + 2, 0x4004 | positive); // Native DAC negative-input selection
        write16(CMP + 6, 5);
        write16(CMP + 8, 0x0f);
        // Native 0xd065, with OUTEN removed and edges disabled until capture.
        // CMPOE is internal comparison output; OUTEN would drive VCOUT externally.
        write16(CMP, 0xc005);
        Ok(Self {
            _channel: channel,
            _routes: routes,
        })
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        stop_events();
        write16(CMP, 0);
        // Preserve DAC1 state: caller owns it, other comparator users may share
        // its reference after explicitly transferring ownership elsewhere.
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Dma(e) => write!(f, "DMA: {e:?}"),
            Self::Routing(e) => write!(f, "IRQ routing: {e:?}"),
        }
    }
}
