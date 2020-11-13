use std::time::{Duration, Instant};

use eyre::eyre;

use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::pixels::PixelFormatEnum;

use fceux::Fceux;

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

    let fceux = Fceux::new(path_rom)?;
    fceux.sound_set_freq(AUDIO_FREQ)?;

    audio.resume();
    let mut timer = Timer::new(60);
    'mainloop: loop {
        for ev in event_pump.poll_iter() {
            match ev {
                Event::Quit { .. } => break 'mainloop,
                _ => {}
            }
        }

        tex.with_lock(None, |buf, pitch| {
            fceux.run_frame(0, 0, |xbuf, soundbuf| {
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
