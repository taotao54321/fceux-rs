use std::ffi::CString;
use std::marker::PhantomData;
use std::os::raw::{c_int, c_void};
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

unsafe extern "C" fn hook_before_exec(userdata: *mut c_void, addr: u16) {
    let before_exec = userdata as *mut Box<dyn FnMut(u16)>;
    (*before_exec)(addr);
}

/// libfceux の制限のため、同時に存在できるインスタンスは 1 つまで。
pub struct Fceux<'a> {
    guard: MutexGuard<'a, ()>,

    before_exec: Box<Box<dyn FnMut(u16)>>,
}

impl<'a> Fceux<'a> {
    pub fn new(path_rom: impl AsRef<Path>, before_exec: Box<dyn FnMut(u16)>) -> Result<Self> {
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

        // トレイトオブジェクトは fat pointer なので、直接 raw pointer に変換することはできない。
        // そこでもう 1 段 Box で包む。
        // ref: https://users.rust-lang.org/t/sending-a-boxed-trait-over-ffi/21708
        let before_exec = Box::new(before_exec);
        let before_exec_raw = Box::into_raw(before_exec); // before_exec は自動解放されなくなる

        unsafe {
            libfceux_sys::fceux_hook_before_exec(
                Some(hook_before_exec),
                before_exec_raw as *mut c_void,
            );
        }

        // 改めて self 内に before_exec を Box として保持する。
        // これにより before_exec が自動解放されるようになる。
        Ok(Self {
            guard,
            before_exec: unsafe { Box::from_raw(before_exec_raw) },
        })
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
        let status = unsafe { libfceux_sys::fceux_snapshot_load(snap.snap) };
        if status == 0 {
            return Err(Error::new("fceux_snapshot_load() failed"));
        }
        Ok(())
    }

    pub fn snapshot_save(&self, snap: &Snapshot) -> Result<()> {
        let status = unsafe { libfceux_sys::fceux_snapshot_save(snap.snap) };
        if status == 0 {
            return Err(Error::new("fceux_snapshot_save() failed"));
        }
        Ok(())
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

    pub fn sound_set_freq(&self, freq: i32) -> Result<()> {
        let status = unsafe { libfceux_sys::fceux_sound_set_freq(freq as c_int) };
        if status == 0 {
            return Err(Error::new("fceux_sound_set_freq() failed"));
        }
        Ok(())
    }
}

impl<'a> Drop for Fceux<'a> {
    fn drop(&mut self) {
        unsafe {
            libfceux_sys::fceux_hook_before_exec(None, std::ptr::null_mut());
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
