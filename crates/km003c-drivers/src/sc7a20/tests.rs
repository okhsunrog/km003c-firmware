use super::*;
extern crate std;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
    vec::Vec,
};
struct Bus {
    id: u8,
    regs: [u8; 64],
    writes: Vec<(u8, u8)>,
}
impl embedded_hal::i2c::ErrorType for Bus {
    type Error = core::convert::Infallible;
}
impl I2c for Bus {
    async fn transaction(
        &mut self,
        address: u8,
        ops: &mut [embedded_hal_async::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        assert_eq!(address, 0x19);
        match ops {
            [embedded_hal_async::i2c::Operation::Write(b)] => {
                self.writes.push((b[0], b[1]));
                self.regs[b[0] as usize] = b[1];
            }
            [
                embedded_hal_async::i2c::Operation::Write(b),
                embedded_hal_async::i2c::Operation::Read(out),
            ] => {
                for (i, v) in out.iter_mut().enumerate() {
                    *v = if b[0] == 0x0f {
                        self.id
                    } else {
                        self.regs[(b[0] & 0x7f) as usize + i]
                    };
                }
            }
            _ => panic!("unexpected transaction"),
        }
        Ok(())
    }
}
fn run<F: Future>(f: F) -> F::Output {
    match pin!(f)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("fake I2C must be immediate"),
    }
}
#[test]
fn gated_configuration_and_signed_burst() {
    let mut bus = Bus {
        id: 0x33,
        regs: [0; 64],
        writes: Vec::new(),
    };
    assert!(matches!(
        run(Sc7a20::configure(&mut bus)),
        Err(Error::Identity(0x33))
    ));
    assert!(bus.writes.is_empty());
    bus.id = 0x11;
    let mut sensor = run(Sc7a20::configure(&mut bus)).unwrap();
    assert_eq!(bus.writes.last(), Some(&(0x20, 0x27)));
    assert!(bus.writes.iter().all(|(r, _)| *r >= 0x20));
    bus.regs[0x27..0x2e].copy_from_slice(&[8, 0x80, 0x3e, 0x80, 0xc1, 0xf0, 0xff]);
    bus.regs[0x31] = 0x41;
    assert_eq!(
        run(sensor.sample(&mut bus)).unwrap(),
        Sample {
            xyz_mg: [1000, -1000, -1],
            source: 0x41,
            status: 8
        }
    );
}
#[test]
fn native_directions_and_debounce() {
    assert_eq!(
        [1, 2, 4, 8].map(|s| direction(s | 0x40)),
        [Some(0), Some(2), Some(1), Some(3)]
    );
    assert_eq!(direction(0x50), None);
    assert_eq!(direction(0x43), None);
    let mut f = Filter::default();
    let sample = |s| Sample {
        xyz_mg: [0; 3],
        source: s,
        status: 8,
    };
    assert_eq!(f.update(sample(0x48)), 0);
    assert_eq!(f.update(sample(0x48)), 0);
    assert_eq!(f.update(sample(0x50)), 0);
    for _ in 0..3 {
        f.update(sample(0x48));
    }
    assert_eq!(f.update(sample(0x50)), 3);
}
