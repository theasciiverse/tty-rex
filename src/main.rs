mod audio;
mod capability;
mod highscore;
mod input;
mod octants;
mod palette;
mod render;
mod settings;
mod sim;
mod sprites;

use input::{Cmd, Input};
use ratatui::crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::layout::Rect;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use render::{Look, Renderer};
use sim::Runner;
use std::io;
use std::time::{Duration, Instant, SystemTime};

const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);
const FRAME: Duration = Duration::from_micros(16_667);
const LOOK_NOTE: Duration = Duration::from_millis(1500);

fn main() -> io::Result<()> {
    // Session logging: RUST_LOG=debug tty-rex 2>tty-rex.log
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    if has("--help") || has("-h") {
        println!(
            "tty-rex [--look NAME] [--wide] [--octants | --no-octants] [--mute] [--force] [--scale N] [--seed N]\n\n\
             Chrome's offline T-Rex runner, in Braille.\n\
             Space / Up jump (hold for higher), Down ducks, Enter restarts,\n\
             P pauses (so does switching away from the terminal), V changes the look,\n\
             M mutes, Q quits.\n\
             Fits anything from a 20x7 split pane up to Chrome's own pixels (38 rows).\n\n\
             --look NAME how it's drawn (V cycles them in game and remembers the choice):\n\
                           mixed    colour half-block scenery, colour Braille sprites (default)\n\
                           blocks   everything in colour half-blocks\n\
                           color    colour Braille sprites on a plain page\n\
                           braille  Braille in the terminal's own colours (default with NO_COLOR)\n\
                           classic  Chrome's grey on white (needs true colour)\n\
             --mono, --classic   same as --look braille, --look classic\n\
             --wide      the road runs the full terminal width, past Chrome's 600 px\n\
                         (you see further ahead, so it's a little easier)\n\
             --octants   draw the blocks look's T-Rex in block octants, as sharp as Braille.\n\
                         On by default in VTE 0.78+ (GNOME Terminal, Ptyxis, ...) and Ghostty 1.1.1+;\n\
                         kitty 0.40+ and foot 1.20+ draw them too but can't be detected\n\
             --no-octants  quadrants instead\n\
             --mute      no sound\n\
             --force     start even if Braille looks unusable here\n\
             --scale N   canvas pixels per Braille dot, 1 (finest) to 6; default fits the terminal\n\
             --seed N    fixed random seed, for replaying a run"
        );
        return Ok(());
    }
    if !has("--force") {
        if let Err(e) = capability::check() {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    let arg_after = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let forced_scale = match arg_after("--scale").map(|v| v.parse::<i32>()) {
        None => None,
        Some(Ok(s)) if (render::MIN_SCALE..=render::MAX_SCALE).contains(&s) => Some(s),
        Some(_) => {
            eprintln!("--scale takes {} to {}", render::MIN_SCALE, render::MAX_SCALE);
            std::process::exit(2);
        }
    };
    let seed = arg_after("--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64));
    // NO_COLOR (no-color.org) and dumb terminals get the monochrome look
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) || std::env::var("TERM").is_ok_and(|t| t == "dumb");
    let look = if let Some(name) = arg_after("--look") {
        Look::parse(&name).unwrap_or_else(|| {
            let names: Vec<_> = Look::ALL.iter().map(|l| l.name()).collect();
            eprintln!("--look takes one of: {}", names.join(", "));
            std::process::exit(2);
        })
    } else if has("--classic") {
        Look::Classic
    } else if has("--mono") {
        Look::Braille
    } else if let Some(saved) = settings::load_look().and_then(|n| Look::parse(&n)) {
        // the look V last picked
        saved
    } else if no_color {
        Look::Braille
    } else {
        Look::Mixed
    };
    let depth = palette::Depth::detect();

    // open audio before the alternate screen, so any ALSA chatter lands above it
    let mut audio = audio::Audio::new(has("--mute"));
    log::info!("audio available {} muted {} seed {seed} look {look:?} depth {depth:?}", audio.available(), audio.muted);

    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    // focus reports drive auto-pause; terminals without them just never send any
    execute!(io::stdout(), EnableFocusChange)?;
    let size = terminal.size()?;
    log::info!("start kitty_keyboard={kitty} terminal {}x{}", size.width, size.height);
    let detected = octants_detected();
    let octants = if has("--no-octants") { false } else { has("--octants") || detected.is_some() };
    let mut renderer = Renderer::new(look, depth, forced_scale, has("--wide"), octants);
    log::info!("octants {octants} (detected: {})", detected.unwrap_or("no"));
    let layout = renderer.layout(Rect::new(0, 0, size.width, size.height));
    log::info!("layout {layout:?}");
    let mut runner = Runner::new(layout.map_or(sim::MAX_WIDTH, |l| l.width), highscore::load(), seed);
    let mut input = Input::new(kitty);
    let result = run(&mut terminal, &mut runner, &mut renderer, &mut input, &mut audio);
    let _ = execute!(io::stdout(), DisableFocusChange);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// Whether the terminal is known to draw block octants (U+1CD00..1CDE5)
/// itself, so the blocks look's T-Rex can be drawn at Braille resolution.
/// Only terminals that say which version they are can be detected; checked
/// in each one's source (2026-10-05): VTE since 0.78, Ghostty since 1.1.1.
/// kitty (0.40.0+) and foot (1.20.0+) draw them too but don't put their
/// version in the environment, so they need --octants; WezTerm has them only
/// in nightlies, Alacritty not at all.
fn octants_detected() -> Option<&'static str> {
    let env = |k: &str| std::env::var(k).ok();
    if env("VTE_VERSION").and_then(|v| v.parse::<u32>().ok()).is_some_and(|v| v >= 7800) {
        return Some("VTE 0.78+");
    }
    if env("TERM_PROGRAM").is_some_and(|p| p == "ghostty") {
        let version = env("TERM_PROGRAM_VERSION").unwrap_or_default();
        let mut parts = version.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse::<u32>().unwrap_or(0));
        let v = (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        if v >= (1, 1, 1) {
            return Some("Ghostty 1.1.1+");
        }
    }
    None
}

fn run(terminal: &mut ratatui::DefaultTerminal, r: &mut Runner, renderer: &mut Renderer, input: &mut Input, audio: &mut audio::Audio) -> io::Result<()> {
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;
    let mut snapshot = Instant::now();
    // when the look last changed, to name it on screen for a moment
    let mut look_shown: Option<Instant> = None;
    loop {
        let frame_start = Instant::now();
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) => {
                    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    match input.on_key(k, Instant::now(), r) {
                        Some(Cmd::Quit) => return Ok(()),
                        Some(Cmd::Pause) => {
                            r.toggle_pause();
                            input.clear();
                            log::info!("pause {}", r.paused);
                        }
                        Some(Cmd::Mute) => {
                            audio.muted = !audio.muted;
                            log::info!("muted {}", audio.muted);
                        }
                        Some(Cmd::Restart) => r.restart_key(),
                        Some(Cmd::Look) => {
                            renderer.look = renderer.look.next();
                            settings::save_look(renderer.look.name());
                            look_shown = Some(Instant::now());
                            log::info!("look {:?}", renderer.look);
                        }
                        None => {}
                    }
                }
                Event::Resize(w, h) => {
                    let layout = renderer.layout(Rect::new(0, 0, w, h));
                    log::info!("resize {w}x{h} layout {layout:?}");
                    if let Some(l) = layout {
                        r.resize(l.width);
                    }
                    // offline.ts adjustDimensions: a resize mid-run stops
                    // the game (Space resumes); the speed stays as it was
                    r.pause();
                    input.clear();
                }
                Event::FocusLost => {
                    let was = r.paused;
                    r.pause();
                    input.clear();
                    log::info!("focus lost, paused {} (was {was})", r.paused);
                }
                Event::FocusGained => log::info!("focus gained"),
                _ => {}
            }
        }
        for key in input.poll(Instant::now(), r) {
            log::debug!("inferred release {key}");
        }

        let now = Instant::now();
        acc += now.duration_since(last).min(Duration::from_millis(100));
        last = now;
        while acc >= FRAME {
            acc -= FRAME;
            r.tick();
        }
        for e in r.take_events() {
            match e {
                sim::Event::Sound(s) => audio.play(s),
                sim::Event::Crashed { score, high } => {
                    log::info!("crash score {score} high {high} speed {:.3}", r.speed);
                    if score == high && score > 0 {
                        highscore::save(high);
                    }
                }
                other => log::info!("{other:?}"),
            }
        }

        let help: &[&str] = if r.crashed {
            &["ENTER or SPACE to restart  ·  Q quit", "ENTER restart · Q quit", "ENTER restart"]
        } else if r.paused {
            &["SPACE or P to resume  ·  Q quit", "SPACE resume · Q quit", "SPACE resume"]
        } else if !r.playing {
            &["SPACE or ↑ to start  ·  Q quit", "SPACE start · Q quit", "SPACE start"]
        } else {
            &["SPACE/↑ jump  ·  ↓ duck  ·  P pause  ·  V look  ·  M mute  ·  Q quit", "↑ jump · ↓ duck · P pause · V look", "↑ jump · ↓ duck"]
        };
        let note = look_shown.filter(|t| t.elapsed() < LOOK_NOTE).map(|_| format!("look: {}", renderer.look.name()));
        terminal.draw(|f| renderer.draw(r, f.area(), f.buffer_mut(), help, note.as_deref()))?;

        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            let t = &r.trex;
            log::debug!(
                "snapshot playing {} activated {} crashed {} paused {} night {} score {} speed {:.3} trex ({},{}) {:?} jumping {} ducking {} obstacles {:?}",
                r.playing,
                r.activated,
                r.crashed,
                r.paused,
                r.inverted,
                r.score(),
                r.speed,
                t.x,
                t.y,
                t.status,
                t.jumping,
                t.ducking,
                r.horizon.obstacles.iter().map(|o| (o.kind, o.size, o.x, o.y)).collect::<Vec<_>>()
            );
        }
        let spent = frame_start.elapsed();
        if spent < FRAME {
            std::thread::sleep(FRAME - spent);
        }
    }
}
