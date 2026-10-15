//! Offline listening renders: a real track through the real engine with one effect, written as WAV files.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use deckforge_common::{Beatgrid, BeatFxKind, ColorFxKind, FxBands, FxTarget, FxTrigger, TrackAudio, HOT_CUE_SLOTS};
use deckforge_engine::anchors::BarMap;
use deckforge_engine::{create_engine, Command, LoadRequest, MixerParam, Routing};

const SR: u32 = 48_000;
const BLOCK: usize = 256;
const CHANNELS: usize = 4;
const BARS: f64 = 4.0;
const SWEEP_STEP_SECS: f64 = 0.02;
const TARGET_PEAK_DB: f32 = -1.0;

struct Demo {
    name: &'static str,
    tail_secs: f64,
    fx_secs: f64,
    events: Vec<(f64, Command)>,
}

struct BeatSpec {
    name: &'static str,
    kind: BeatFxKind,
    beats: f64,
    percent: f64,
    level: f32,
    tail_secs: f64,
    bands: Option<FxBands>,
    division_sweep: &'static [f64],
}

fn beat_demo(spec: BeatSpec, section: f64, bar: f64) -> Demo {
    let mut events = vec![
        (0.0, Command::BeatFxKind(spec.kind)),
        (0.0, Command::BeatFxTarget(FxTarget::A)),
        (0.0, Command::BeatFxBeats(spec.beats)),
        (0.0, Command::BeatFxPercent(spec.percent)),
        (0.0, Command::BeatFxLevel(spec.level)),
        (section, Command::BeatFxEnabled(true)),
        (2.0 * section, Command::Pause { deck: 0 }),
        (2.0 * section, Command::BeatFxEnabled(false)),
    ];
    if let Some(bands) = spec.bands {
        events.push((0.0, Command::BeatFxBands(bands)));
    }
    for (i, beats) in spec.division_sweep.iter().enumerate() {
        events.push((section + i as f64 * bar, Command::BeatFxBeats(*beats)));
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    Demo { name: spec.name, tail_secs: spec.tail_secs, fx_secs: section, events }
}

fn color_demo(name: &'static str, kind: ColorFxKind, side: f32, tail_secs: f64, section: f64) -> Demo {
    let mut events = vec![(0.0, Command::ColorFx(kind))];
    let steps = (section / SWEEP_STEP_SECS) as usize;
    for i in 0..=steps {
        let t = i as f64 * SWEEP_STEP_SECS;
        events.push((section + t, Command::Mixer { channel: 0, param: MixerParam::Filter(side * (t / section) as f32) }));
    }
    events.push((2.0 * section, Command::Mixer { channel: 0, param: MixerParam::Filter(0.0) }));
    events.push((2.0 * section, Command::Pause { deck: 0 }));
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    Demo { name, tail_secs, fx_secs: section, events }
}

const RIDE_STEP_SECS: f64 = 0.05;

fn reverb_depth_ride(section: f64, bar: f64) -> Demo {
    let ride = 8.0 * bar;
    let mut events = vec![
        (0.0, Command::BeatFxKind(BeatFxKind::Reverb)),
        (0.0, Command::BeatFxTarget(FxTarget::A)),
        (0.0, Command::BeatFxPercent(70.0)),
        (0.0, Command::BeatFxLevel(0.0)),
        (section, Command::BeatFxEnabled(true)),
    ];
    let steps = (ride / RIDE_STEP_SECS) as usize;
    for i in 0..=steps {
        let t = i as f64 * RIDE_STEP_SECS;
        events.push((section + t, Command::BeatFxLevel((t / ride) as f32)));
    }
    let cut = section + ride;
    events.push((cut, Command::BeatFxLevel(0.0)));
    events.push((cut, Command::BeatFxEnabled(false)));
    let end = ride + 2.0 * bar;
    events.push((section + end, Command::Pause { deck: 0 }));
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    Demo { name: "reverb-depth-ride", tail_secs: 4.0, fx_secs: end, events }
}

fn reverb_percent_sweep(section: f64, bar: f64) -> Demo {
    let jumps: [f64; 6] = [60.0, 50.0, 70.0, 40.0, 100.0, 1.0];
    let mut events = vec![
        (0.0, Command::BeatFxKind(BeatFxKind::Reverb)),
        (0.0, Command::BeatFxTarget(FxTarget::A)),
        (0.0, Command::BeatFxPercent(50.0)),
        (0.0, Command::BeatFxLevel(0.75)),
        (section, Command::BeatFxEnabled(true)),
    ];
    for (i, percent) in jumps.iter().enumerate() {
        events.push((section + (i as f64 + 1.0) * bar, Command::BeatFxPercent(*percent)));
    }
    let ride_start = (jumps.len() as f64 + 1.0) * bar;
    let ride = 4.0 * bar;
    let steps = (ride / RIDE_STEP_SECS) as usize;
    for i in 0..=steps {
        let t = i as f64 * RIDE_STEP_SECS;
        events.push((section + ride_start + t, Command::BeatFxPercent(1.0 + 99.0 * t / ride)));
    }
    let end = ride_start + ride;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    Demo { name: "reverb-percent-sweep", tail_secs: 10.0, fx_secs: end, events }
}

fn demo(name: &'static str, tail_secs: f64, fx_secs: f64, mut events: Vec<(f64, Command)>) -> Demo {
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    Demo { name, tail_secs, fx_secs, events }
}

fn setup(kind: BeatFxKind, beats: f64, level: f32, params: &[(usize, f32)]) -> Vec<(f64, Command)> {
    let mut events = vec![
        (0.0, Command::BeatFxKind(kind)),
        (0.0, Command::BeatFxTarget(FxTarget::A)),
        (0.0, Command::BeatFxBeats(beats)),
        (0.0, Command::BeatFxLevel(level)),
    ];
    events.extend(params.iter().map(|(index, value)| (0.0, Command::BeatFxParam { index: *index, value: *value })));
    events
}

fn spiral_division_change(section: f64, bar: f64) -> Demo {
    let mut events = setup(BeatFxKind::Spiral, 0.5, 0.85, &[]);
    events.push((section, Command::BeatFxEnabled(true)));
    for (i, beats) in [1.0, 0.5, 1.0, 0.5].iter().enumerate() {
        events.push((section + (i as f64 + 1.0) * 2.0 * bar, Command::BeatFxBeats(*beats)));
    }
    let end = 10.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("spiral-division-change", 8.0, end, events)
}

fn echo_feedback_freeze(section: f64, bar: f64) -> Demo {
    let mut events = setup(BeatFxKind::Echo, 0.75, 0.8, &[(0, 42.0)]);
    events.push((section, Command::BeatFxEnabled(true)));
    let ride = 4.0 * bar;
    let steps = (ride / RIDE_STEP_SECS) as usize;
    for i in 0..=steps {
        let t = i as f64 * RIDE_STEP_SECS;
        events.push((section + bar + t, Command::BeatFxParam { index: 0, value: 42.0 + 58.0 * (t / ride) as f32 }));
    }
    events.push((section + 6.0 * bar, Command::BeatFxTrigger(FxTrigger::FreezeOn)));
    events.push((section + 8.0 * bar, Command::BeatFxTrigger(FxTrigger::FreezeOff)));
    let end = 10.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("echo-feedback-freeze", 14.0, end, events)
}

fn reverb_voicing(name: &'static str, voicing: f32) -> impl FnOnce(f64, f64) -> Demo {
    move |section, bar| {
        let mut events = setup(BeatFxKind::Reverb, 1.0, 1.0, &[(3, voicing)]);
        events.push((0.0, Command::BeatFxPercent(60.0)));
        events.push((section, Command::BeatFxEnabled(true)));
        let end = 4.0 * bar;
        events.push((section + end, Command::BeatFxEnabled(false)));
        events.push((section + end, Command::Pause { deck: 0 }));
        demo(name, 12.0, end, events)
    }
}

fn pulse_verb_accelerate(section: f64, bar: f64) -> Demo {
    let mut events = setup(BeatFxKind::PulseVerb, 1.0, 0.85, &[(0, 55.0), (1, 85.0), (2, 35.0), (3, 1.0), (4, 1.0)]);
    events.push((section, Command::BeatFxEnabled(true)));
    let end = 8.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("pulse-verb-accelerate", 6.0, end, events)
}

fn dub_tape_throw(section: f64, bar: f64) -> Demo {
    let beat = bar / 4.0;
    let mut events = setup(BeatFxKind::DubTape, 0.75, 0.9, &[(0, 72.0), (5, 1.0)]);
    events.push((section, Command::BeatFxEnabled(true)));
    for (at, hold) in [(0.0, beat), (2.0 * bar, 0.5 * beat), (3.0 * bar + beat, 2.0 * beat)] {
        events.push((section + at, Command::BeatFxTrigger(FxTrigger::ThrowOn)));
        events.push((section + at + hold, Command::BeatFxTrigger(FxTrigger::ThrowOff)));
    }
    let end = 7.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("dub-tape-throw", 14.0, end, events)
}

fn dub_tape_oscillation_ride(section: f64, bar: f64) -> Demo {
    let mut events = setup(BeatFxKind::DubTape, 0.75, 0.8, &[(0, 45.0), (3, 25.0)]);
    events.push((section, Command::BeatFxEnabled(true)));
    let ride = 6.0 * bar;
    let steps = (ride / RIDE_STEP_SECS) as usize;
    for i in 0..=steps {
        let t = i as f64 * RIDE_STEP_SECS;
        events.push((section + t, Command::BeatFxParam { index: 0, value: 45.0 + 60.0 * (t / ride) as f32 }));
    }
    let mute = section + ride + bar;
    events.push((mute, Command::Pause { deck: 0 }));
    let end = ride + 5.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    demo("dub-tape-oscillation-ride", 12.0, end, events)
}

fn dub_tape_freeze(section: f64, bar: f64) -> Demo {
    let mut events = setup(BeatFxKind::DubTape, 0.5, 0.8, &[(0, 50.0)]);
    events.push((section, Command::BeatFxEnabled(true)));
    events.push((section + bar, Command::BeatFxTrigger(FxTrigger::FreezeOn)));
    events.push((section + 5.0 * bar, Command::BeatFxTrigger(FxTrigger::FreezeOff)));
    let end = 7.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("dub-tape-freeze", 10.0, end, events)
}

fn halo_freeze(section: f64, bar: f64) -> Demo {
    let mut events = setup(BeatFxKind::Halo, 1.0, 0.8, &[(0, 65.0), (2, 30.0)]);
    events.push((0.0, Command::BeatFxPercent(65.0)));
    events.push((section, Command::BeatFxEnabled(true)));
    events.push((section + bar, Command::BeatFxTrigger(FxTrigger::FreezeOn)));
    events.push((section + 5.0 * bar, Command::BeatFxTrigger(FxTrigger::FreezeOff)));
    let end = 7.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("halo-freeze", 14.0, end, events)
}

fn ascend_build(phrases: &[f64], grid: Beatgrid, start: f64, section: f64, bar: f64) -> Demo {
    let map = BarMap::new(grid, phrases);
    let enable_track = start + section;
    let landing = map.ascend_landing(enable_track, 16.0, true);
    println!("ascend: enabled at track {enable_track:.2} s, 16-bar build lands at {:.2} s ({:.1} bars, bar {})", landing.secs, landing.bars, landing.bar_number);
    let landing_demo = landing.secs - start;
    let mut events = setup(BeatFxKind::Ascend, 1.0, 0.5, &[(0, 2.0), (1, 0.0), (2, 0.0)]);
    events.push((section, Command::BeatFxEnabled(true)));
    events.push((landing_demo - 0.5 * bar, Command::BeatFxTrigger(FxTrigger::Release)));
    let end = landing_demo - section + 4.0 * bar;
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("ascend-16-bar-build-drop", 4.0, end, events)
}

fn backwash_into_downbeat(phrases: &[f64], grid: Beatgrid, start: f64, section: f64, bar: f64) -> Demo {
    let map = BarMap::new(grid, phrases);
    let trigger_track = start + section + 0.6 * bar;
    let landing = map.next_bar_line(trigger_track);
    println!("backwash: armed at track {trigger_track:.2} s, lands on the downbeat at {landing:.2} s");
    let mut events = setup(BeatFxKind::Backwash, 4.0, 1.0, &[(0, 70.0)]);
    events.push((section + 0.6 * bar, Command::BeatFxEnabled(true)));
    let second = map.next_bar_line(landing + 3.0 * bar);
    events.push((second - start - 1.7 * bar, Command::BeatFxTrigger(FxTrigger::ThrowOn)));
    let end = second - start - section + 2.0 * bar;
    events.push((section + end, Command::BeatFxEnabled(false)));
    events.push((section + end, Command::Pause { deck: 0 }));
    demo("backwash-into-downbeat", 3.0, end, events)
}

fn render(track: &Arc<TrackAudio>, grid: Beatgrid, phrases: &[f64], start_secs: f64, demo: Demo, section: f64) -> Vec<f32> {
    let (handle, mut rt) = create_engine(SR);
    rt.reconfigure(SR, Routing::for_channel_count(CHANNELS));
    let request = Box::new(LoadRequest { track: track.clone(), token: 1, grid: Some(grid), cue_secs: Some(0.0), hot_cues: [None; HOT_CUE_SLOTS] });
    handle.send(Command::Load { deck: 0, request });
    handle.send(Command::SetPhraseStarts { deck: 0, starts: phrases.into() });
    handle.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    handle.send(Command::MasterLevel(0.8));
    handle.send(Command::SeekSecs { deck: 0, secs: start_secs });
    handle.send(Command::Play { deck: 0 });

    let total = ((section + demo.fx_secs + demo.tail_secs) * SR as f64) as usize;
    let mut events = demo.events.into_iter().peekable();
    let mut buf = vec![0f32; BLOCK * CHANNELS];
    let mut out = Vec::with_capacity(total * 2);
    let mut done = 0usize;
    while done < total {
        let now = done as f64 / SR as f64;
        while events.peek().is_some_and(|(t, _)| *t <= now) {
            if let Some((_, command)) = events.next() {
                handle.send(command);
            }
        }
        rt.process(&mut buf, CHANNELS);
        for frame in buf.chunks(CHANNELS) {
            out.push(frame[0]);
            out.push(frame[1]);
        }
        handle.collect_garbage();
        done += BLOCK;
    }
    out
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let spec = hound::WavSpec { channels: 2, sample_rate: SR, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).with_context(|| format!("create {}", path.display()))?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32_767.0).round() as i16)?;
    }
    w.finalize()?;
    Ok(())
}

pub fn run(track_path: &str, out_dir: &str, start_near_secs: f64, label: Option<&str>) -> Result<()> {
    let track = deckforge_analysis::decode_file(Path::new(track_path))?;
    let grid = deckforge_analysis::analyze_beatgrid(&track, Default::default()).context("no beat grid for this track")?.grid;
    let phrases: Vec<f64> = deckforge_analysis::analyze_phrases(&track, grid).iter().map(|p| p.start_secs).collect();
    let track = Arc::new(track);
    let bar = 240.0 / grid.bpm;
    let section = BARS * bar;
    let bars_in = ((start_near_secs - grid.first_beat_secs) / bar).ceil().max(0.0);
    let start = grid.first_beat_secs + bars_in * bar;
    std::fs::create_dir_all(out_dir)?;
    println!("track {track_path}\nbpm {:.2}, bar {:.2} s, start {:.2} s, sections: {BARS} bars dry, {BARS} bars effect, then tail; {} phrase starts", grid.bpm, bar, start, phrases.len());

    let echo = |name, kind, beats, level, tail_secs| BeatSpec {
        name,
        kind,
        beats,
        percent: 50.0,
        level,
        tail_secs,
        bands: None,
        division_sweep: &[],
    };
    let reverb = |name, percent, tail_secs| BeatSpec {
        name,
        kind: BeatFxKind::Reverb,
        beats: 1.0,
        percent,
        level: 0.75,
        tail_secs,
        bands: None,
        division_sweep: &[],
    };
    let sweep = BeatSpec { division_sweep: &[0.25, 0.5, 0.75, 1.0], ..echo("echo-division-sweep", BeatFxKind::Echo, 0.25, 0.8, 12.0) };
    let low_on = BeatSpec { bands: Some(FxBands::ALL), ..echo("echo-low-on", BeatFxKind::Echo, 0.75, 0.8, 12.0) };
    let low_off = echo("echo-low-off", BeatFxKind::Echo, 0.75, 0.8, 12.0);
    let demos = vec![
        beat_demo(sweep, section, bar),
        beat_demo(echo("echo-half-depth", BeatFxKind::Echo, 1.0, 0.5, 10.0), section, bar),
        beat_demo(echo("echo-full-depth", BeatFxKind::Echo, 1.0, 1.0, 20.0), section, bar),
        beat_demo(low_off, section, bar),
        beat_demo(low_on, section, bar),
        beat_demo(echo("pingpong-full-depth", BeatFxKind::PingPong, 0.5, 1.0, 20.0), section, bar),
        beat_demo(echo("upecho-half-depth", BeatFxKind::UpEcho, 0.75, 0.6, 12.0), section, bar),
        beat_demo(reverb("reverb-30pct", 30.0, 8.0), section, bar),
        beat_demo(reverb("reverb-100pct", 100.0, 18.0), section, bar),
        beat_demo(BeatSpec { bands: Some(FxBands { low: false, mid: false, high: true }), level: 0.85, ..reverb("reverb-high-only", 60.0, 10.0) }, section, bar),
        reverb_voicing("reverb-classic-full-depth", 0.0)(section, bar),
        reverb_voicing("reverb-tension-full-depth", 1.0)(section, bar),
        reverb_depth_ride(section, bar),
        reverb_percent_sweep(section, bar),
        color_demo("space-sweep-right", ColorFxKind::Space, 1.0, 16.0, section),
        color_demo("space-sweep-left", ColorFxKind::Space, -1.0, 16.0, section),
        color_demo("noise-riser-right", ColorFxKind::Noise, 1.0, 6.0, section),
        color_demo("noise-riser-left", ColorFxKind::Noise, -1.0, 6.0, section),
        spiral_division_change(section, bar),
        echo_feedback_freeze(section, bar),
        pulse_verb_accelerate(section, bar),
        dub_tape_throw(section, bar),
        dub_tape_oscillation_ride(section, bar),
        dub_tape_freeze(section, bar),
        halo_freeze(section, bar),
        ascend_build(&phrases, grid, start, section, bar),
        backwash_into_downbeat(&phrases, grid, start, section, bar),
    ];

    let reference = {
        let spec = BeatSpec { name: "reference", kind: BeatFxKind::Echo, beats: 1.0, percent: 50.0, level: 0.0, tail_secs: 0.0, bands: None, division_sweep: &[] };
        let samples = render(&track, grid, &phrases, start, beat_demo(spec, section, bar), section);
        let edge = (section * SR as f64) as usize * 2;
        db(rms(&samples[edge..2 * edge])) - db(rms(&samples[..edge]))
    };
    println!("\nechoes at level 0 (the music alone): effect section is {reference:+.1} dB against the dry section");
    println!("echo family loudness, effect section vs dry section (1 beat, no wav):");
    let kinds = [BeatFxKind::Echo, BeatFxKind::Delay, BeatFxKind::PingPong, BeatFxKind::UpEcho, BeatFxKind::DownEcho, BeatFxKind::Spiral];
    for kind in kinds {
        let mut cells = Vec::new();
        for level in [0.5f32, 1.0] {
            let spec = BeatSpec { name: "loudness", kind, beats: 1.0, percent: 50.0, level, tail_secs: 0.0, bands: None, division_sweep: &[] };
            let samples = render(&track, grid, &phrases, start, beat_demo(spec, section, bar), section);
            let edge = (section * SR as f64) as usize * 2;
            let (dry, fx) = (db(rms(&samples[..edge])), db(rms(&samples[edge..2 * edge])));
            cells.push(format!("level {level}: dry {dry:.1} -> fx {fx:.1} dBFS ({:+.1} dB, {:+.1} dB from the effect)", fx - dry, fx - dry - reference));
        }
        println!("  {:<9} {}", format!("{kind:?}"), cells.join("   "));
    }
    println!();

    for demo in demos {
        let (name, fx_secs) = (demo.name, demo.fx_secs);
        let mut samples = render(&track, grid, &phrases, start, demo, section);
        let raw_peak = peak(&samples);
        let gain = 10f32.powf(TARGET_PEAK_DB / 20.0) / raw_peak.max(1e-6);
        samples.iter_mut().for_each(|s| *s *= gain);
        let dry_end = (section * SR as f64) as usize * 2;
        let fx_end = ((section + fx_secs) * SR as f64) as usize * 2;
        let file = label.map_or_else(|| format!("{name}.wav"), |l| format!("{l}--{name}.wav"));
        let path = Path::new(out_dir).join(file);
        write_wav(&path, &samples)?;
        println!(
            "{}  {:.1} s  raw peak {:.1} dBFS, gain {:+.1} dB -> peak {:.1} dBFS, rms {:.1} dBFS (dry section {:.1}, effect section {:.1}, tail {:.1})",
            path.display(),
            samples.len() as f64 / 2.0 / SR as f64,
            db(raw_peak),
            db(gain),
            db(peak(&samples)),
            db(rms(&samples)),
            db(rms(&samples[..dry_end])),
            db(rms(&samples[dry_end..fx_end])),
            db(rms(&samples[fx_end..])),
        );
    }
    Ok(())
}
