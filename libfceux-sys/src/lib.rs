use std::os::raw::{c_char, c_int, c_uint, c_void};

pub const FCEUX_MEMORY_CPU: FceuxMemoryDomain = 0;
pub type FceuxMemoryDomain = c_uint;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Snapshot {
    _unused: [u8; 0],
}

pub type FceuxHookBeforeExec = Option<unsafe extern "C" fn(userdata: *mut c_void, addr: u16)>;

extern "C" {
    pub fn fceux_init(path_rom: *const c_char) -> c_int;
    pub fn fceux_quit();

    pub fn fceux_run_frame(
        joy1: u8,
        joy2: u8,
        xbuf: *mut *mut u8,
        soundbuf: *mut *mut i32,
        soundbuf_size: *mut i32,
    );

    pub fn fceux_mem_read(addr: u16, domain: FceuxMemoryDomain) -> u8;
    pub fn fceux_mem_write(addr: u16, value: u8, domain: FceuxMemoryDomain);

    pub fn fceux_snapshot_create() -> *mut Snapshot;
    pub fn fceux_snapshot_destroy(snap: *mut Snapshot);
    pub fn fceux_snapshot_load(snap: *mut Snapshot) -> c_int;
    pub fn fceux_snapshot_save(snap: *mut Snapshot) -> c_int;

    pub fn fceux_hook_before_exec(hook: FceuxHookBeforeExec, userdata: *mut c_void);

    pub fn fceux_video_get_palette(idx: u8, r: *mut u8, g: *mut u8, b: *mut u8);

    pub fn fceux_sound_set_freq(freq: c_int) -> c_int;
}
