//! Board CC2 Rd control through AW9523. Transactions belong to this worker,
//! so cancelling a caller cannot interrupt an expander update halfway through.
//! Synchronous TX/fault cleanup requests high-Z; normal stop also awaits its
//! result. An I2C failure is reported, never treated as a successful detach.
use core::{cell::RefCell, future::poll_fn, task::Poll};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
    waitqueue::MultiWakerRegistration,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bus,
    Identity,
    Readback,
    Timeout,
    Superseded,
}
struct State {
    version: u32,
    applied: u32,
    enabled: bool,
    result: Result<(), Error>,
    actual: Option<bool>,
    // Capture, RPC loader entry and the physical Exit path may all wait for
    // release. A single AtomicWaker would lose one of those notifications.
    waiters: MultiWakerRegistration<3>,
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    version: 0,
    applied: u32::MAX,
    enabled: false,
    result: Ok(()),
    actual: None,
    waiters: MultiWakerRegistration::new(),
}));
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
pub fn request(enabled: bool) -> u32 {
    let version = STATE.lock(|s| {
        let mut s = s.borrow_mut();
        s.version = s.version.wrapping_add(1);
        s.enabled = enabled;
        s.waiters.wake();
        s.version
    });
    CHANGED.signal(());
    version
}
pub async fn set(enabled: bool) -> Result<(), Error> {
    let version = request(enabled);
    poll_fn(|cx| {
        STATE.lock(|s| {
            let mut s = s.borrow_mut();
            if s.version != version {
                Poll::Ready(Err(Error::Superseded))
            } else if s.applied == version {
                Poll::Ready(s.result)
            } else {
                s.waiters.register(cx.waker());
                Poll::Pending
            }
        })
    })
    .await
}
#[embassy_executor::task]
pub async fn task(aw: crate::aw9523::Aw9523) {
    loop {
        CHANGED.wait().await;
        loop {
            let (version, enabled, actual) = STATE.lock(|s| {
                let s = s.borrow();
                (s.version, s.enabled, s.actual)
            });
            let result = if actual == Some(enabled) {
                Ok(())
            } else {
                aw.cc2_rd(enabled).await
            };
            if let Err(e) = result {
                log::error!("CC2 termination update failed: {e:?}");
            }
            let current = STATE.lock(|s| {
                let mut s = s.borrow_mut();
                s.applied = version;
                s.result = result;
                s.actual = result.ok().map(|()| enabled);
                s.waiters.wake();
                s.version == version
            });
            if current {
                break;
            }
        }
    }
}
