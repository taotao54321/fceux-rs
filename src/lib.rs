//! libfceux は元々単一インスタンスしかサポートしていないので、シングルスレッドでしか使えない。
//! よってスレッド安全性は考慮していない。

use std::ffi::CString;
use std::os::raw::{c_int, c_uint, c_void};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
#[error("fceux error: {0}")]
pub struct Error(String);

impl Error {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Hook {
    fn before_exec(&mut self, _addr: u16) {}
}

static mut INITIALIZED: bool = false;
static mut HOOK: Option<Box<dyn Hook>> = None;

unsafe extern "C" fn ffi_hook_before_exec(_: *mut c_void, addr: u16) {
    if let Some(hook) = HOOK.as_mut() {
        hook.before_exec(addr);
    }
}

/// 初期化処理。
/// この関数が成功する前に他の関数を使った場合の結果は未定義。
///
/// 終了処理はサポートしない。
pub fn init(path_rom: impl AsRef<Path>) -> Result<()> {
    unsafe {
        if INITIALIZED {
            return Err(Error::new("already initialized"));
        }
    }

    let path_rom = path_rom.as_ref();
    let path_rom_c = CString::new(
        path_rom
            .as_os_str()
            .to_str()
            .ok_or_else(|| Error::new("OsStr::to_str() failed"))?,
    )
    .map_err(|e| Error::new(format!("CString::new() failed: {}", e)))?;

    unsafe {
        let status = libfceux_sys::fceux_init(path_rom_c.as_ptr());
        if status == 0 {
            return Err(Error::new("fceux_init() failed"));
        }

        libfceux_sys::fceux_hook_before_exec(Some(ffi_hook_before_exec), std::ptr::null_mut());

        INITIALIZED = true;
    }

    Ok(())
}

/// フレーム境界以外から呼び出した場合の結果は未定義。
pub fn run_frame<F: FnOnce(&[u8], &[i32])>(joy1: u8, joy2: u8, f: F) {
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

pub fn mem_read(addr: u16, domain: MemoryDomain) -> u8 {
    unsafe { libfceux_sys::fceux_mem_read(addr, domain as c_uint) }
}

pub fn mem_write(addr: u16, value: u8, domain: MemoryDomain) {
    unsafe {
        libfceux_sys::fceux_mem_write(addr, value, domain as c_uint);
    }
}

pub fn snapshot_create() -> Snapshot {
    Snapshot::new()
}

pub fn snapshot_load(snap: &Snapshot) -> Result<()> {
    let status = unsafe { libfceux_sys::fceux_snapshot_load(snap.snap) };
    if status == 0 {
        return Err(Error::new("fceux_snapshot_load() failed"));
    }
    Ok(())
}

pub fn snapshot_save(snap: &Snapshot) -> Result<()> {
    let status = unsafe { libfceux_sys::fceux_snapshot_save(snap.snap) };
    if status == 0 {
        return Err(Error::new("fceux_snapshot_save() failed"));
    }
    Ok(())
}

pub fn hook_set(hook: Option<Box<dyn Hook>>) {
    unsafe {
        HOOK = hook;
    }
}

pub fn video_get_palette(idx: u8) -> (u8, u8, u8) {
    let mut r = 0;
    let mut g = 0;
    let mut b = 0;
    unsafe {
        libfceux_sys::fceux_video_get_palette(idx, &mut r, &mut g, &mut b);
    }
    (r, g, b)
}

pub fn sound_set_freq(freq: i32) -> Result<()> {
    let status = unsafe { libfceux_sys::fceux_sound_set_freq(freq as c_int) };
    if status == 0 {
        return Err(Error::new("fceux_sound_set_freq() failed"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MemoryDomain {
    Cpu = libfceux_sys::FCEUX_MEMORY_CPU,
}

#[derive(Debug)]
pub struct Snapshot {
    snap: *mut libfceux_sys::Snapshot,
}

impl Snapshot {
    fn new() -> Self {
        let snap = unsafe { libfceux_sys::fceux_snapshot_create() };
        if snap.is_null() {
            panic!("out of memory");
        }

        Self { snap }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe {
            libfceux_sys::fceux_snapshot_destroy(self.snap);
        }
    }
}
