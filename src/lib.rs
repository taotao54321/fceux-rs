use std::ffi::CString;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use lazy_static::lazy_static;

#[derive(Debug, thiserror::Error)]
#[error("fceux error: {0}")]
pub struct Error(String);

impl Error {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

lazy_static! {
    static ref MUTEX: Mutex<()> = Mutex::new(());
}

/// libfceux の制限のため、同時に存在できるインスタンスは 1 つまで。
#[derive(Debug)]
pub struct Fceux<'a> {
    guard: MutexGuard<'a, ()>,
}

impl<'a> Fceux<'a> {
    pub fn new(path_rom: impl AsRef<Path>) -> Result<Self> {
        let guard = MUTEX
            .try_lock()
            .map_err(|_| Error::new("sorry, Fceux is singleton"))?;

        let path_rom = path_rom.as_ref();
        let path_rom_c = CString::new(
            path_rom
                .as_os_str()
                .to_str()
                .ok_or_else(|| Error::new("OsStr::to_str() failed"))?,
        )
        .map_err(|e| Error::new(format!("CString::new() failed: {}", e)))?;

        let status = unsafe { libfceux_sys::fceux_init(path_rom_c.as_ptr()) };
        if status == 0 {
            return Err(Error::new("fceux_init() failed"));
        }

        Ok(Self { guard })
    }

    pub fn run_frame<F: FnOnce(&[u8], &[i32])>(&self, joy1: u8, joy2: u8, f: F) {
        let mut xbuf: *mut u8 = std::ptr::null_mut();
        let mut soundbuf: *mut i32 = std::ptr::null_mut();
        let mut soundbuf_size: i32 = 0;
        let (xbuf, soundbuf) = unsafe {
            libfceux_sys::fceux_run_frame(joy1, joy2, &mut xbuf, &mut soundbuf, &mut soundbuf_size);
            (
                std::slice::from_raw_parts(xbuf, 256 * 240),
                std::slice::from_raw_parts(soundbuf, soundbuf_size as usize),
            )
        };
        f(xbuf, soundbuf);
    }

    pub fn snapshot_create(&self) -> Snapshot {
        Snapshot::new()
    }

    pub fn snapshot_load(&self, snap: &Snapshot) -> Result<()> {
        todo!();
    }

    pub fn snapshot_save(&self, snap: &Snapshot) -> Result<()> {
        todo!();
    }

    pub fn video_get_palette(&self, idx: u8) -> (u8, u8, u8) {
        let mut r = 0;
        let mut g = 0;
        let mut b = 0;
        unsafe {
            libfceux_sys::fceux_video_get_palette(idx, &mut r, &mut g, &mut b);
        }
        (r, g, b)
    }
}

impl<'a> Drop for Fceux<'a> {
    fn drop(&mut self) {
        unsafe {
            libfceux_sys::fceux_quit();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MemoryDomain {
    Cpu = libfceux_sys::FCEUX_MEMORY_CPU,
}

#[derive(Debug)]
pub struct Snapshot<'a> {
    snap: *mut libfceux_sys::Snapshot,
    phantom: PhantomData<&'a ()>,
}

impl<'a> Snapshot<'a> {
    fn new() -> Self {
        let snap = unsafe { libfceux_sys::fceux_snapshot_create() };
        if snap.is_null() {
            panic!("out of memory");
        }

        Self {
            snap,
            phantom: PhantomData,
        }
    }
}

impl<'a> Drop for Snapshot<'a> {
    fn drop(&mut self) {
        unsafe {
            libfceux_sys::fceux_snapshot_destroy(self.snap);
        }
    }
}
