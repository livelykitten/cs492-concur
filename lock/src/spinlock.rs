use core::sync::atomic::{AtomicBool, Ordering};

use crossbeam_utils::Backoff;

use crate::lock::*;
/*
    tries to acquire a lock in a spin loop
    spinlock is a boolean, that can be accessed from multiple threads
    true - lock, held by someone
    false - no one is using (no one is locking)
*/
pub struct SpinLock {
    inner: AtomicBool,
}
/*
    Default - false, since not used
*/
impl Default for SpinLock {
    fn default() -> Self {
        Self {
            inner: AtomicBool::new(false),
        }
    }
}

impl RawLock for SpinLock {
    type Token = ();
/*
    lock function returns some kind of  proof
    Token - for other locks have meaningful value
    Backup sonnze is called, and it waits for the lock to turn off.
    every consec call to  snooze increases wait time exponentially.
*/
    fn lock(&self) {
        let backoff = Backoff::new();
/*
    compare_and_swap: trying to replace Atomicbool 
    from false to true, and ret true when did it.
*/
        while self.inner.compare_and_swap(false, true, Ordering::Acquire) {
            backoff.snooze();
        }
    }
/*
    unlock - stores false, make it able for others to use
    unsafe - not 100% safe. should'nt unlock lock acquired by others, not me.
    token acquired by this function needs to be provided to unlock funciton
*/
    unsafe fn unlock(&self, _token: ()) {
        self.inner.store(false, Ordering::Release);
    }
}
/*
    RawTryLock - try to acquire lock
    discussed later
*/
impl RawTryLock for SpinLock {
    fn try_lock(&self) -> Result<(), ()> {
        if !self.inner.compare_and_swap(false, true, Ordering::Acquire) {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::spinlock::SpinLock;

    #[test]
    fn smoke() {
        crate::lock::tests::smoke::<SpinLock>();
    }
}
