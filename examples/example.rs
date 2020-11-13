use std::time::{Duration, Instant};

use eyre::eyre;

use sdl2::event::Event;
use sdl2::pixels::PixelFormatEnum;

use fceux::Fceux;

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
    let mut event_pump = sdl.event_pump().map_err(|s| eyre!(s))?;

    let win = sdl_video.window("fceux-rs demo", 512, 480).build()?;
    let mut canvas = win.into_canvas().build()?;
    let tex_creator = canvas.texture_creator();
    let mut tex = tex_creator.create_texture_streaming(PixelFormatEnum::RGBX8888, 256, 240)?;

    let fceux = Fceux::new(path_rom)?;

    let mut timer = Timer::new(60);
    'mainloop: loop {
        for ev in event_pump.poll_iter() {
            match ev {
                Event::Quit { .. } => break 'mainloop,
                _ => {}
            }
        }

        tex.with_lock(None, |buf, pitch| {
            fceux.run_frame(0, 0, |xbuf, _| {
                for y in 0..240 {
                    for x in 0..256 {
                        let (r, g, b) = fceux.video_get_palette(xbuf[256 * y + x]);
                        buf[pitch * y + 4 * x] = 0x00;
                        buf[pitch * y + 4 * x + 1] = b;
                        buf[pitch * y + 4 * x + 2] = g;
                        buf[pitch * y + 4 * x + 3] = r;
                    }
                }
            });
        })
        .map_err(|s| eyre!(s))?;

        canvas.copy(&tex, None, None).map_err(|s| eyre!(s))?;
        canvas.present();

        timer.delay();
    }

    Ok(())
}
