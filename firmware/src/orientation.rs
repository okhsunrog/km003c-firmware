//! SC7A20-family orientation on shared I2C3. AW9523 and sensor operations share
//! an async bus mutex, including CC termination changes.
use core::cell::Cell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embedded_hal_async::i2c::I2c;
use km003c_drivers::sc7a20::{Filter, Sc7a20};
#[derive(Clone, Copy, Debug)]
struct State {
    id: u8,
    source: u8,
    xyz_mg: [i16; 3],
    rotation: u8,
    error: bool,
    count: u32,
}
static STATE: Mutex<CriticalSectionRawMutex, Cell<State>> = Mutex::new(Cell::new(State {
    id: 0,
    source: 0,
    xyz_mg: [0; 3],
    rotation: 3,
    error: false,
    count: 0,
}));
pub struct Controller {
    sensor: Sc7a20,
    filter: Filter,
    last: embassy_time::Instant,
}
pub async fn probe(bus: &mut impl I2c) -> Option<Controller> {
    match Sc7a20::configure(bus).await {
        Ok(sensor) => {
            STATE.lock(|c| {
                let mut s = c.get();
                s.id = 0x11;
                c.set(s)
            });
            log::info!("SC7A20-family ID=11; 10Hz HR +/-2g BDU; config readback passed");
            Some(Controller {
                sensor,
                filter: Filter::default(),
                last: embassy_time::Instant::now(),
            })
        }
        Err(e) => {
            log::warn!("Accelerometer disabled: {e:?}");
            STATE.lock(|c| {
                let mut s = c.get();
                s.error = true;
                c.set(s)
            });
            None
        }
    }
}
impl Controller {
    pub async fn poll(&mut self, bus: &mut impl I2c) -> u8 {
        let mut s = STATE.lock(Cell::get);
        if s.error || self.last.elapsed() < embassy_time::Duration::from_millis(100) {
            return s.rotation;
        }
        self.last = embassy_time::Instant::now();
        match self.sensor.sample(bus).await {
            Ok(sample) => {
                s.xyz_mg = sample.xyz_mg;
                s.source = sample.source;
                // Board mounting offset verified visually by the owner:
                // rotate every native direction 90 degrees counterclockwise.
                s.rotation = (self.filter.update(sample) + 3) % 4;
                s.count = s.count.wrapping_add(1);
            }
            Err(e) => {
                log::warn!("Accelerometer sample failed: {e:?}");
                s.error = true;
            }
        }
        STATE.lock(|c| c.set(s));
        s.rotation
    }
}
