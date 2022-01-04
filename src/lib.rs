//! libfceux は元々単一インスタンスしかサポートしていないので、シングルスレッドでしか使えない。
//! よってスレッド安全性は考慮していない。

use std::cell::UnsafeCell;
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

/// P レジスタ。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RegP(u8);

impl RegP {
    /// キャリーフラグを返す。
    pub fn carry(self) -> bool {
        (self.0 & (1 << 0)) != 0
    }

    /// ゼロフラグを返す。
    pub fn zero(self) -> bool {
        (self.0 & (1 << 1)) != 0
    }

    /// オーバーフローフラグを返す。
    pub fn overflow(self) -> bool {
        (self.0 & (1 << 6)) != 0
    }

    /// ネガティブフラグを返す。
    pub fn negative(self) -> bool {
        (self.0 & (1 << 7)) != 0
    }
}

fn hook_dummy(_addr: u16) {}

struct Hook {
    f: UnsafeCell<*const dyn FnMut(u16)>,
}

impl Hook {
    fn replace(&self, f: &dyn FnMut(u16)) {
        unsafe {
            let f: &'static dyn FnMut(u16) = std::mem::transmute(f);
            *self.f.get() = f;
        }
    }

    fn call(&self, addr: u16) {
        unsafe {
            let f: &mut dyn FnMut(u16) = &mut *(*self.f.get() as *mut dyn FnMut(u16));
            f(addr);
        }
    }
}

unsafe impl Sync for Hook {}

static mut INITIALIZED: bool = false;
static HOOK: Hook = Hook {
    f: UnsafeCell::new(&hook_dummy),
};

unsafe extern "C" fn ffi_hook_before_exec(_: *mut c_void, addr: u16) {
    HOOK.call(addr);
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

pub fn was_init() -> bool {
    unsafe { INITIALIZED }
}

pub fn power() {
    unsafe {
        libfceux_sys::fceux_power();
    }
}

pub fn reset() {
    unsafe {
        libfceux_sys::fceux_reset();
    }
}

/// フレーム境界以外から呼び出した場合の結果は未定義。
pub fn run_frame<VideoSoundF>(
    joy1: u8,
    joy2: u8,
    f_video_sound: VideoSoundF,
    f_hook: &dyn FnMut(u16),
) where
    VideoSoundF: FnOnce(&[u8], &[i32]),
{
    HOOK.replace(f_hook);

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
    f_video_sound(xbuf, soundbuf);

    HOOK.replace(&hook_dummy);
}

/// P レジスタを読み取る。
pub fn reg_p() -> RegP {
    let inner = unsafe { libfceux_sys::fceux_reg_p() };
    RegP(inner)
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
