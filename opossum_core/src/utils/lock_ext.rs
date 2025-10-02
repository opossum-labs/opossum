use crate::error::{OpmResult, OpossumError};
use std::sync::{Mutex, MutexGuard};

pub trait LockExt<T: ?Sized> {
    /// Helper function for locking Mutexex handling errors.
    ///
    /// This function converts a lock error into an [`OpossumError`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the failed.
    fn lock_opm(&self) -> OpmResult<MutexGuard<'_, T>>;
}

impl<T: ?Sized> LockExt<T> for Mutex<T> {
    fn lock_opm(&self) -> OpmResult<MutexGuard<'_, T>> {
        self.lock().map_err(|e| OpossumError::Other(e.to_string()))
    }
}
