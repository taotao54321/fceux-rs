use std::time::{Duration, Instant};

use eyre::eyre;

use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::{KeyboardState, Keycode, Scancode};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;
use sdl2::EventPump;

use fceux::{MemoryDomain, Snapshot};

const AUDIO_FREQ: i32 = 44100;

#[derive(Debug)]
struct Timer {
    frame_dur: Duration,
    nxt: Instant, // 次フレームのタイムスタンプ
}

impl Timer {
    fn new(fps: u32) -> Self {
        let frame_dur = Duration::new(0, 1_000_000_000 / fps);
        let nxt = Instant::now() + frame_dur;
        Self { frame_dur, nxt }
    }

    fn delay(&mut self) {
        let now = Instant::now();
        if now < self.nxt {
            std::thread::sleep(self.nxt - now);
            self.nxt += self.frame_dur;
        } else {
            // 処理が追いつかない場合、諦めて次から 60 FPS を目指す。
            // ここを self.nxt += self.frame_dur とすると遅れを(可能なら)挽回できるが、
            // 挽回している間 FPS が 60 を超えてしまうのは望ましくないと考えた。
            self.nxt = now + self.frame_dur;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cmd {
    Quit,
    Load,
    Save,
    Power,
    Reset,
    Emulate(u8),
}

fn event(event_pump: &mut EventPump) -> Cmd {
    for ev in event_pump.poll_iter() {
        match ev {
            Event::Quit { .. } => return Cmd::Quit,
            Event::KeyDown {
                keycode: Some(key), ..
            } => match key {
                Keycode::Q => return Cmd::Quit,
                Keycode::L => return Cmd::Load,
                Keycode::S => return Cmd::Save,
                Keycode::P => return Cmd::Power,
                Keycode::R => return Cmd::Reset,
                _ => {}
            },
            _ => {}
        }
    }

    event_pump.pump_events();
    let keys = KeyboardState::new(event_pump);

    let mut joy = 0;
    let mut joykey = |scancode: Scancode, bit: i32| {
        if keys.is_scancode_pressed(scancode) {
            joy |= 1 << bit;
        }
    };

    joykey(Scancode::Z, 0);
    joykey(Scancode::X, 1);
    joykey(Scancode::V, 2);
    joykey(Scancode::C, 3);
    joykey(Scancode::Up, 4);
    joykey(Scancode::Down, 5);
    joykey(Scancode::Left, 6);
    joykey(Scancode::Right, 7);

    Cmd::Emulate(joy)
}

fn cmd_load(snap: &Snapshot) {
    match fceux::snapshot_load(snap) {
        Ok(_) => eprintln!("loaded snapshot"),
        Err(_) => eprintln!("cannot load snapshot"),
    }
}

fn cmd_save(snap: &Snapshot) {
    match fceux::snapshot_save(snap) {
        Ok(_) => eprintln!("saved snapshot"),
        Err(_) => eprintln!("cannot save snapshot"),
    }
}

fn cmd_power() {
    fceux::power();
    eprintln!("power");
}

fn cmd_reset() {
    fceux::reset();
    eprintln!("reset");
}

fn cmd_emulate(
    canvas: &mut Canvas<Window>,
    tex: &mut Texture,
    audio: &AudioQueue<i16>,
    joy: u8,
) -> eyre::Result<()> {
    let mut nmi_called = false;
    let f_hook = |addr: u16| {
        let addr_nmi = fceux::mem_read(0xFFFA, MemoryDomain::Cpu) as u16
            | ((fceux::mem_read(0xFFFB, MemoryDomain::Cpu) as u16) << 8);
        if addr == addr_nmi {
            nmi_called = true;
        }
    };

    tex.with_lock(None, |buf, pitch| {
        fceux::run_frame(
            joy,
            0,
            |xbuf, soundbuf| {
                // FCEUX はサウンドバッファが 32bit 単位なので変換が必要。
                // サンプル単位で処理しているので若干遅そうだが、手元では問題なく鳴っている。
                // ちゃんとやるなら [i16; 1024] 程度のバッファを用意して変換すべきか。
                //
                // なお、AudioQueue::queue() は内部で SDL_QueueAudio() を呼んでいる。
                // この関数は実装当初は音がおかしかったが、現在は問題ない模様。
                for sample in soundbuf {
                    audio.queue(&[*sample as i16]);
                }

                for y in 0..240 {
                    for x in 0..256 {
                        let (r, g, b) = fceux::video_get_palette(xbuf[256 * y + x]);
                        buf[pitch * y + 4 * x] = 0x00;
                        buf[pitch * y + 4 * x + 1] = b;
                        buf[pitch * y + 4 * x + 2] = g;
                        buf[pitch * y + 4 * x + 3] = r;
                    }
                }
            },
            &f_hook,
        );
    })
    .map_err(|s| eyre!(s))?;

    canvas.copy(&tex, None, None).map_err(|s| eyre!(s))?;
    canvas.present();

    /*
    if nmi_called {
        eprintln!("NMI");
    }
    */

    Ok(())
}

fn mainloop(
    event_pump: &mut EventPump,
    canvas: &mut Canvas<Window>,
    tex: &mut Texture,
    audio: &AudioQueue<i16>,
) -> eyre::Result<()> {
    let snap = fceux::snapshot_create();

    audio.resume();
    let mut timer = Timer::new(60);
    loop {
        let cmd = event(event_pump);
        match cmd {
            Cmd::Quit => break,
            Cmd::Load => cmd_load(&snap),
            Cmd::Save => cmd_save(&snap),
            Cmd::Power => cmd_power(),
            Cmd::Reset => cmd_reset(),
            Cmd::Emulate(joy) => cmd_emulate(canvas, tex, audio, joy)?,
        }

        timer.delay();
    }

    Ok(())
}

fn print_instruction() {
    eprintln!(
        "\
Instruction
-----------
Arrow-keys      D-pad
z               A
x               B
c               Start
v               Select
l               Load state
s               Save state
p               Power
r               Reset
q               Quit
"
    );
}

fn usage() -> ! {
    eprintln!("example <rom.nes>");
    std::process::exit(1);
}

fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        usage();
    }
    let path_rom = &args[1];

    let sdl = sdl2::init().map_err(|s| eyre!(s))?;
    let sdl_video = sdl.video().map_err(|s| eyre!(s))?;
    let sdl_audio = sdl.audio().map_err(|s| eyre!(s))?;
    let mut event_pump = sdl.event_pump().map_err(|s| eyre!(s))?;

    let win = sdl_video.window("fceux-rs demo", 512, 480).build()?;
    let mut canvas = win.into_canvas().build()?;
    let tex_creator = canvas.texture_creator();
    let mut tex = tex_creator.create_texture_streaming(PixelFormatEnum::RGBX8888, 256, 240)?;

    let audio = {
        let want = AudioSpecDesired {
            freq: Some(AUDIO_FREQ),
            channels: Some(1),
            samples: Some(4096),
        };
        sdl_audio
            .open_queue::<i16, _>(None, &want)
            .map_err(|s| eyre!(s))
    }?;

    assert!(!fceux::was_init());
    fceux::init(path_rom)?;
    assert!(fceux::was_init());

    fceux::sound_set_freq(AUDIO_FREQ)?;

    print_instruction();

    mainloop(&mut event_pump, &mut canvas, &mut tex, &audio)
}
