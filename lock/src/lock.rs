use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

// high level spinlock no need to care unsafe

/*
    train RawLock - needs to have Token, lock, unlock
    (Token from lock and to unlock should match)

    other things user need to guarentee is that the data that is protected
    should be  related to the lock. 

    those are taken care by high level api
*/

pub trait RawLock: Default {
    type Token: Clone;

    fn lock(&self) -> Self::Token;

    /// # Safety
    ///
    /// `unlock()` should be called with the token given by the corresponding `lock()`.
    unsafe fn unlock(&self, token: Self::Token);
}

pub trait RawTryLock: RawLock {
    fn try_lock(&self) -> Result<Self::Token, ()>;
}

/*
    now lock and data comes in a pair
    unsafeCell - internal mutability for initial access
*/

#[repr(C)]
pub struct Lock<L: RawLock, T> {
    lock: L,
    data: UnsafeCell<T>,
}

/*
    lines for making lock type as send or sync
    T should be sendable since it is accessible by multiple threads
    no need for T to be Sync. no such case that multiple threads accessing T at the same time
    (guarenteed by implementation of spinlock)
    
    marked unsafe because they are guarenteed by
    implementation block, not by rust implementation sys
    manually inspect that locks are send and sync
*/

unsafe impl<L: RawLock, T: Send> Send for Lock<L, T> {}
unsafe impl<L: RawLock, T: Send> Sync for Lock<L, T> {}

/*
    creating Rawlock using Spinlock for type T
    initial value needs  to  be protected.
    
*/

impl<L: RawLock, T> Lock<L, T> {
    // data: data to protect
    pub fn new(data: T) -> Self {
        Self {
            // for spinlock, init to false, as seen in rawlock
            lock: L::default(),
            data: UnsafeCell::new(data),
        }
    }

    // destroy lock and get inner data T
    // when you are given  the total ownership of the lock
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
    /*
        calling  lock acquires inner lock and ret lockguard
        LockGuard is a proof that you acquired the lock
        
    */
    pub fn lock(&self) -> LockGuard<L, T> {
        /*
            token partly proves that you acquired the lock,
            and shouldbe given to the lock function
        */
        let token = self.lock.lock();
        LockGuard {
            lock: self,
            token,
            _marker: PhantomData,
        }
    }
}

impl<L: RawTryLock, T> Lock<L, T> {
    pub fn try_lock(&self) -> Result<LockGuard<L, T>, ()> {
        self.lock.try_lock().map(|token| LockGuard {
            lock: self,
            token,
            _marker: PhantomData,
        })
    }
}

impl<L: RawLock, T> Lock<L, T> {
    pub unsafe fn unlock_unchecked(&self, token: L::Token) {
        self.lock.unlock(token);
    }

    pub unsafe fn get_unchecked(&self) -> &T {
        &*self.data.get()
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut_unchecked(&self) -> &mut T {
        &mut *self.data.get()
    }
}
/*
    scope o statically guarenteed to enclose the lifetime of the lock type
    when the scope is  used in the first  line,
    Lockguard should be dropped before the lifetime  for s ends.
    that's the meaning  of 's
    
    pub fn lock(&self) -> LockGuard<L, T>
    scope(lifetime of lock) is given as &self in here
    so it follows this
    
    LockGuard shouldn't outlife the lock - simple!!
    
    
*/
pub struct LockGuard<'s, L: RawLock, T> {
    lock: &'s Lock<L, T>,
    token: L::Token, // token given to the lock function
    _marker: PhantomData<*const ()>, // !Send + !Sync
}

unsafe impl<'s, L: RawLock, T> Send for LockGuard<'s, L, T> {}
unsafe impl<'s, L: RawLock, T: Send + Sync> Sync for LockGuard<'s, L, T> {}

impl<'s, L: RawLock, T> LockGuard<'s, L, T> {
    pub fn raw(&mut self) -> usize {
        self.lock as *const _ as usize
    }
}
/*
    when lockguard is dropped(destroyed) underlying lock should be automatically released
    so this calls unlock function
*/
impl<'s, L: RawLock, T> Drop for LockGuard<'s, L, T> {
    fn drop(&mut self) {
        // unsafe cuz unlock func is unsafe
        unsafe { self.lock.lock.unlock(self.token.clone()) };
    }
    // internally guarenteed to be safe by programmer, so drop function is safe
}

/*
    dereference data
*/
impl<'s, L: RawLock, T> Deref for LockGuard<'s, L, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}
/*
    mutably dereference data
*/
impl<'s, L: RawLock, T> DerefMut for LockGuard<'s, L, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'s, L: RawLock, T> LockGuard<'s, L, T> {
    pub fn into_raw(self) -> usize {
        let ret = self.lock as *const _ as usize;
        mem::forget(self);
        ret
    }

    pub unsafe fn from_raw(data: usize, token: L::Token) -> Self {
        Self {
            lock: &*(data as *const _),
            token,
            _marker: PhantomData,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use core::ops::Deref;

    use crossbeam_utils::thread::scope;

    use super::{Lock, RawLock};

    pub fn smoke<L: RawLock>() {
        const LENGTH: usize = 1024;
        let d = Lock::<L, Vec<usize>>::new(vec![]);

        scope(|s| {
            for i in 1..LENGTH {
                let d = &d;
                s.spawn(move |_| {
                    let mut d = d.lock();
                    d.push(i);
                });
            }
        })
        .unwrap();

        let mut d = d.lock();
        d.sort();
        assert_eq!(d.deref(), &(1..LENGTH).collect::<Vec<usize>>());
    }
}
