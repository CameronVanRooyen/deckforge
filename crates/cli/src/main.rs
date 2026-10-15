//! Headless diagnostics: device listing and a loopback routing test (no speaker output).

use std::sync::Arc;
use std::time::Duration;

mod fx_demos;

use anyhow::{anyhow, bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use deckforge_common::{TrackAudio, HOT_CUE_SLOTS};
use deckforge_engine::{
    create_engine, list_output_devices, AudioManager, Command, EngineHandle, LoadRequest, MixerParam, OutputRequest,
    SecondaryOutputRequest, SecondarySource, SecondaryState,
};
use parking_lot::Mutex;

fn tone(freq: f64, secs: f64, sr: u32) -> Arc<TrackAudio> {
    let n = (secs * sr as f64) as usize;
    let samples = (0..n)
        .flat_map(|i| {
            let s = ((std::f64::consts::TAU * freq * i as f64 / sr as f64).sin() * 0.5) as f32;
            [s, s]
        })
        .collect();
    Arc::new(TrackAudio::new(sr, samples))
}

fn load(h: &EngineHandle, deck: usize, track: Arc<TrackAudio>) {
    let request = Box::new(LoadRequest { track, token: deck as u64 + 1, grid: None, cue_secs: Some(0.0), hot_cues: [None; HOT_CUE_SLOTS] });
    h.send(Command::Load { deck, request });
}

fn goertzel(x: &[f32], freq: f64, sr: f64) -> f64 {
    let w = std::f64::consts::TAU * freq / sr;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        re += *v as f64 * (w * i as f64).cos();
        im += *v as f64 * (w * i as f64).sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f64
}

fn devices() {
    for d in list_output_devices() {
        println!(
            "{}{}  [{} ch, rates {:?}]\n    id: {}",
            if d.is_default { "* " } else { "  " },
            d.name,
            d.max_channels,
            d.sample_rates,
            d.id
        );
    }
}

/// Plays deck A to master and deck B to headphones only, captures the device loopback, and
/// checks each tone appears on exactly the expected channel pair.
fn loopback(device_match: &str) -> Result<()> {
    const A_HZ: f64 = 330.0;
    const B_HZ: f64 = 1_250.0;
    let out_dev = list_output_devices()
        .into_iter()
        .find(|d| d.name.contains(device_match))
        .ok_or_else(|| anyhow!("no output device matching '{device_match}'"))?;
    if out_dev.max_channels < 4 {
        bail!("{} has only {} channels; need 4", out_dev.name, out_dev.max_channels);
    }
    let host = cpal::default_host();
    let input = host
        .input_devices()?
        .find(|d| d.description().map(|x| x.name().contains(device_match)).unwrap_or(false))
        .context("matching loopback input device")?;

    let (handle, rt) = create_engine(48_000);
    let manager = AudioManager::spawn(rt);
    let active = manager
        .open(OutputRequest { device_id: Some(out_dev.id.clone()), ..Default::default() })
        .map_err(|e| anyhow!(e))?;
    println!("output: {} @ {} Hz, {} ch, routing {:?}", active.device_name, active.sample_rate, active.channels, active.routing);
    let sr = active.sample_rate;

    load(&handle, 0, tone(A_HZ, 30.0, sr));
    load(&handle, 1, tone(B_HZ, 30.0, sr));
    handle.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    handle.send(Command::Mixer { channel: 1, param: MixerParam::Fader(0.0) });
    handle.send(Command::Mixer { channel: 1, param: MixerParam::Pfl(true) });
    handle.send(Command::HeadphoneMix(0.0));
    handle.send(Command::Play { deck: 0 });
    handle.send(Command::Play { deck: 1 });

    let in_cfg = input.default_input_config()?;
    let in_channels = in_cfg.channels() as usize;
    let in_rate = in_cfg.sample_rate() as f64;
    let captured: Arc<Mutex<Vec<f32>>> = Arc::default();
    let sink = captured.clone();
    let stream = input.build_input_stream(
        in_cfg.config(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| sink.lock().extend_from_slice(data),
        |e| eprintln!("input error: {e}"),
        None,
    )?;
    std::thread::sleep(Duration::from_millis(800));
    stream.play()?;
    std::thread::sleep(Duration::from_secs(2));
    drop(stream);

    let snap = handle.snapshot();
    let status = manager.status();
    manager.close();
    let data = captured.lock().clone();
    let frames = data.len() / in_channels;
    println!("captured {frames} frames x {in_channels} ch @ {in_rate} Hz; callbacks {} ({} frames each), load {:.1}%",
        status.callbacks, status.last_callback_frames, snap.callback_load * 100.0);
    let mut ok = true;
    for ch in 0..4 {
        let x: Vec<f32> = data.chunks(in_channels).map(|f| f[ch]).collect();
        let a = goertzel(&x, A_HZ, in_rate);
        let b = goertzel(&x, B_HZ, in_rate);
        let expect_a = ch < 2;
        let pass = if expect_a { a > 0.1 && b < 0.005 } else { b > 0.1 && a < 0.005 };
        ok &= pass;
        println!("  ch{} {}: deck A {:.4}  deck B {:.4}  {}", ch + 1, if ch < 2 { "master" } else { "phones" }, a, b, if pass { "OK" } else { "FAIL" });
    }
    if !ok {
        bail!("routing check failed");
    }
    println!("PASS: deck A only on master (1-2), cued deck B only on headphones (3-4)");
    Ok(())
}

struct Capture {
    data: Vec<f32>,
    channels: usize,
    rate: f64,
}

fn capture_input(device_match: &str, secs: f64) -> Result<Capture> {
    let input = cpal::default_host()
        .input_devices()?
        .find(|d| d.description().map(|x| x.name().contains(device_match)).unwrap_or(false))
        .context("matching loopback input device")?;
    let cfg = input.default_input_config()?;
    let channels = cfg.channels() as usize;
    let rate = cfg.sample_rate() as f64;
    let captured: Arc<Mutex<Vec<f32>>> = Arc::default();
    let sink = captured.clone();
    let stream = input.build_input_stream(
        cfg.config(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| sink.lock().extend_from_slice(data),
        |e| eprintln!("input error: {e}"),
        None,
    )?;
    stream.play()?;
    std::thread::sleep(Duration::from_secs_f64(secs));
    drop(stream);
    let data = captured.lock().clone();
    Ok(Capture { data, channels, rate })
}

fn channel_levels(c: &Capture, ch: usize, freqs: [f64; 2]) -> [f64; 2] {
    let x: Vec<f32> = c.data.chunks(c.channels).map(|f| f[ch]).collect();
    [goertzel(&x, freqs[0], c.rate), goertzel(&x, freqs[1], c.rate)]
}

/// Drives the engine only through official DDJ-400 MIDI bytes (as the hardware sends them)
/// and verifies the resulting audio on a loopback device.
fn ddj400_sim(device_match: &str) -> Result<()> {
    use deckforge_controller::{ControllerService, Ddj400};
    const A_HZ: f64 = 330.0;
    const B_HZ: f64 = 1_250.0;
    let out_dev = list_output_devices()
        .into_iter()
        .find(|d| d.name.contains(device_match))
        .ok_or_else(|| anyhow!("no output device matching '{device_match}'"))?;
    let (handle, rt) = create_engine(48_000);
    let engine = Arc::new(handle);
    let manager = AudioManager::spawn(rt);
    let active = manager
        .open(OutputRequest { device_id: Some(out_dev.id.clone()), ..Default::default() })
        .map_err(|e| anyhow!(e))?;
    let sr = active.sample_rate;
    let app_actions: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = app_actions.clone();
    let ctl = ControllerService::spawn(Box::new(Ddj400::new()), engine.clone(), Box::new(move |a| sink.lock().push(format!("{a:?}"))));
    load(&engine, 0, tone(A_HZ, 30.0, sr));
    load(&engine, 1, tone(B_HZ, 30.0, sr));
    let send = |bytes: &[u8]| {
        ctl.inject(bytes.to_vec());
        std::thread::sleep(Duration::from_millis(15));
    };
    let fader = |deck: u8, v: u16| [[0xB0 | deck, 0x13, (v >> 7) as u8], [0xB0 | deck, 0x33, (v & 0x7F) as u8]];
    for m in fader(0, 16_383).iter().chain(fader(1, 0).iter()) {
        send(m);
    }
    send(&[0xB6, 0x1F, 0x40]);
    send(&[0xB6, 0x3F, 0x00]);
    send(&[0x91, 0x54, 0x7F]);
    send(&[0x91, 0x54, 0x00]);
    send(&[0x90, 0x0B, 0x7F]);
    send(&[0x90, 0x0B, 0x00]);
    send(&[0x91, 0x0B, 0x7F]);
    send(&[0x91, 0x0B, 0x00]);
    send(&[0xB6, 0x40, 0x01]);
    send(&[0x96, 0x47, 0x7F]);
    std::thread::sleep(Duration::from_millis(600));

    let snap = engine.snapshot();
    println!("after MIDI: deck A playing={} deck B playing={} fader A={:.3} fader B={:.3} PFL B={} crossfader={:.3}",
        snap.decks[0].playing, snap.decks[1].playing, snap.channels[0].fader, snap.channels[1].fader, snap.channels[1].pfl, snap.master.crossfader);
    let cap = capture_input(device_match, 1.5)?;
    let master = channel_levels(&cap, 0, [A_HZ, B_HZ]);
    let phones = channel_levels(&cap, 2, [A_HZ, B_HZ]);
    println!("  master: A {:.4} B {:.4}   phones: A {:.4} B {:.4}", master[0], master[1], phones[0], phones[1]);
    let stage1 = master[0] > 0.1 && master[1] < 0.005 && phones[1] > 0.1 && phones[0] < 0.005;

    for m in fader(1, 16_383) {
        send(&m);
    }
    send(&[0x90, 0x0B, 0x7F]);
    send(&[0x90, 0x0B, 0x00]);
    std::thread::sleep(Duration::from_millis(400));
    let cap = capture_input(device_match, 1.0)?;
    let master2 = channel_levels(&cap, 0, [A_HZ, B_HZ]);
    println!("  after raising fader B + pausing A via PLAY: master A {:.4} B {:.4}", master2[0], master2[1]);
    let stage2 = master2[0] < 0.005 && master2[1] > 0.1;

    let actions = app_actions.lock().clone();
    println!("  app actions from browse/load: {actions:?}");
    let log = ctl.log();
    println!("  MIDI log entries: {} (last: {})", log.len(), log.last().map(|e| e.summary.as_str()).unwrap_or("-"));
    manager.close();
    if stage1 && stage2 && actions.len() == 2 {
        println!("PASS: DDJ-400 MIDI drove play/pause, faders, crossfader, headphone cue and browse/load end-to-end");
        Ok(())
    } else {
        bail!("simulation check failed")
    }
}

const SECONDARY_TONE_HZ: f64 = 330.0;
const QUIET_LEVEL: f32 = 0.05;
const QUIET_SECS: f64 = 3.0;

fn find_output(device_match: &str) -> Result<deckforge_engine::OutputDeviceInfo> {
    list_output_devices()
        .into_iter()
        .find(|d| d.name.contains(device_match))
        .ok_or_else(|| anyhow!("no output device matching '{device_match}'"))
}

/// Mirrors the master to a second device for `secs` and reports latency, drift and underruns.
/// Anything that is not a BlackHole loopback is capped at 3 s and level 0.05.
fn secondary_check(primary_match: &str, secondary_match: &str, secs: f64, level: f32, master: f32) -> Result<()> {
    let primary_dev = find_output(primary_match)?;
    let secondary_dev = find_output(secondary_match)?;
    let virtual_device = secondary_dev.name.contains("BlackHole");
    if !virtual_device && (secs > QUIET_SECS || level > QUIET_LEVEL) {
        bail!("{} is a real output: limited to {QUIET_SECS} s at level {QUIET_LEVEL}", secondary_dev.name);
    }
    let (handle, rt) = create_engine(48_000);
    let manager = AudioManager::spawn(rt);
    let active = manager
        .open(OutputRequest { device_id: Some(primary_dev.id.clone()), ..Default::default() })
        .map_err(|e| anyhow!(e))?;
    println!("primary:   {} @ {} Hz, {} ch", active.device_name, active.sample_rate, active.channels);
    load(&handle, 0, tone(SECONDARY_TONE_HZ, secs + 30.0, active.sample_rate));
    handle.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    handle.send(Command::Crossfader(0.0));
    handle.send(Command::MasterLevel(master));
    handle.send(Command::Play { deck: 0 });
    manager
        .set_secondary(Some(SecondaryOutputRequest {
            device_id: secondary_dev.id.clone(),
            source: SecondarySource::Master,
            level,
            channels: [0, 1],
        }))
        .map_err(|e| anyhow!(e))?;
    let started = manager.status();
    let sec = started.secondary.context("secondary status missing after open")?;
    println!("secondary: {} @ {} Hz, level {level}", sec.device_name, sec.sample_rate);

    let capture_secs = secs.min(3.0);
    let captures: Vec<_> = [(primary_match, &primary_dev), (secondary_match, &secondary_dev)]
        .into_iter()
        .filter(|(_, dev)| dev.name.contains("BlackHole"))
        .map(|(m, _)| {
            let name = m.to_string();
            let wait = secs - capture_secs;
            let handle = std::thread::spawn({
                let name = name.clone();
                move || {
                    std::thread::sleep(Duration::from_secs_f64(wait));
                    capture_input(&name, capture_secs)
                }
            });
            (name, handle)
        })
        .collect();

    let warmup = (secs / 3.0).min(10.0);
    let begin = std::time::Instant::now();
    let mut warm_underruns = None;
    let mut latencies: Vec<(f64, f32)> = Vec::new();
    let mut last = Some(sec.clone());
    while begin.elapsed().as_secs_f64() < secs {
        std::thread::sleep(Duration::from_secs(1).min(Duration::from_secs_f64(secs / 3.0)));
        let t = begin.elapsed().as_secs_f64();
        let status = manager.status();
        let Some(sec) = status.secondary else { bail!("secondary output vanished") };
        println!(
            "  t={t:5.1}s  {:?}  latency {:5.1} ms  drift {:+7.1} ppm  underruns {}  primary xruns {}  primary callbacks {}",
            sec.state, sec.latency_ms, sec.drift_ppm, sec.underruns, status.xruns, status.callbacks
        );
        if t >= warmup {
            warm_underruns.get_or_insert(sec.underruns);
            latencies.push((t, sec.latency_ms));
        }
        last = Some(sec);
    }
    let sec = last.context("no status samples")?;
    let shared = manager.secondary_shared().stats();
    manager.close();

    let base = warm_underruns.unwrap_or(0);
    let (min, max) = latencies.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (_, l)| (lo.min(*l), hi.max(*l)));
    println!(
        "summary: state {:?}, latency {min:.1}..{max:.1} ms after warm-up, drift {:+.1} ppm, underruns {} total / {} after warm-up, ring fill {:.1} ms (target {:.1} ms), secondary callbacks {}, ring overruns {}",
        sec.state, sec.drift_ppm, sec.underruns, sec.underruns - base, shared.fill_ms, shared.target_ms, shared.callbacks, shared.overruns
    );
    let mut ok = sec.state == SecondaryState::Running && sec.underruns == base && (max - min) < 5.0;
    let mut readings = Vec::new();
    let both_captured = captures.len() == 2;
    for (name, capture) in captures {
        let cap = capture.join().map_err(|_| anyhow!("capture thread panicked"))??;
        let peak = |ch: usize| cap.data.chunks(cap.channels).map(|f| f[ch].abs()).fold(0.0f32, f32::max) as f64;
        let (level_l, level_r) = (peak(0), peak(1));
        println!("  captured {name}: {SECONDARY_TONE_HZ} Hz tone peak  L {level_l:.4}  R {level_r:.4}  ({} frames @ {} Hz)", cap.data.len() / cap.channels, cap.rate);
        readings.push(level_l);
    }
    if let (true, [primary, secondary]) = (both_captured, &readings[..]) {
        let (primary, secondary) = (*primary, *secondary);
        let ratio = secondary / primary.max(1e-9);
        let expected = deckforge_engine::mixer::master_gain(level) as f64;
        println!("  secondary/primary amplitude {ratio:.3} (expected {expected:.3})");
        ok &= primary > 0.1 && (ratio - expected).abs() < 0.03;
    }
    if ok {
        println!("PASS: secondary output ran with stable latency and no underruns after warm-up");
        Ok(())
    } else {
        bail!("secondary output check failed")
    }
}

/// Switches the primary between devices while the secondary runs, then checks it follows, pauses on a clash and resumes.
fn secondary_swap(first: &str, second: &str, secondary_match: &str) -> Result<()> {
    let (first, second, secondary_dev) = (find_output(first)?, find_output(second)?, find_output(secondary_match)?);
    let (handle, rt) = create_engine(48_000);
    let manager = AudioManager::spawn(rt);
    let open = |dev: &deckforge_engine::OutputDeviceInfo| {
        manager.open(OutputRequest { device_id: Some(dev.id.clone()), ..Default::default() }).map_err(|e| anyhow!(e))
    };
    let request = SecondaryOutputRequest { device_id: secondary_dev.id.clone(), source: SecondarySource::Master, level: 0.8, channels: [0, 1] };
    let report = |label: &str| {
        let status = manager.status();
        let sec = status.secondary.clone();
        println!(
            "  {label}: primary {:?} on {} | secondary {}",
            status.state,
            status.active.as_ref().map_or("-", |a| a.device_name.as_str()),
            sec.map_or("off".to_string(), |s| format!("{:?} latency {:.1} ms underruns {} callbacks {}", s.state, s.latency_ms, s.underruns, manager.secondary_shared().stats().callbacks)),
        );
        status
    };
    open(&first)?;
    load(&handle, 0, tone(SECONDARY_TONE_HZ, 120.0, 48_000));
    handle.send(Command::Mixer { channel: 0, param: MixerParam::Fader(1.0) });
    handle.send(Command::MasterLevel(0.05));
    handle.send(Command::Play { deck: 0 });
    manager.set_secondary(Some(request.clone())).map_err(|e| anyhow!(e))?;
    std::thread::sleep(Duration::from_secs(4));
    report("both running        ");

    let reject = manager.set_secondary(Some(SecondaryOutputRequest { device_id: first.id.clone(), ..request.clone() }));
    println!("  secondary on the primary device -> {reject:?}");
    let mut ok = reject.is_err();

    open(&second)?;
    std::thread::sleep(Duration::from_secs(4));
    let after_switch = report("primary switched     ");
    ok &= after_switch.secondary.as_ref().is_some_and(|s| s.state == SecondaryState::Running && s.underruns <= 1);

    open(&secondary_dev)?;
    std::thread::sleep(Duration::from_millis(1500));
    let clash = report("primary took its device");
    ok &= clash.secondary.as_ref().is_some_and(|s| s.state == SecondaryState::Lost);

    open(&first)?;
    std::thread::sleep(Duration::from_secs(4));
    let resumed = report("primary moved away    ");
    ok &= resumed.secondary.as_ref().is_some_and(|s| s.state == SecondaryState::Running);

    manager.secondary_shared().mark_lost();
    std::thread::sleep(Duration::from_millis(400));
    ok &= report("simulated device loss ").secondary.is_some_and(|s| s.state == SecondaryState::Lost);
    std::thread::sleep(Duration::from_millis(2500));
    let recovered = report("device back          ");
    ok &= recovered.secondary.is_some_and(|s| s.state == SecondaryState::Running) && recovered.state == deckforge_engine::OutputState::Running;

    manager.set_secondary(None).map_err(|e| anyhow!(e))?;
    std::thread::sleep(Duration::from_millis(300));
    ok &= report("secondary turned off  ").secondary.is_none();
    manager.close();
    if ok { println!("PASS: secondary survived primary switches, rejected a clash, resumed and reconnected after a loss"); Ok(()) } else { bail!("swap check failed") }
}

#[derive(Clone, Copy)]
enum FxPass {
    Off,
    Beat(deckforge_common::BeatFxKind),
    Color(deckforge_common::ColorFxKind),
    AllRotating,
}

struct PassResult {
    median_pct: f64,
    p99_pct: f64,
    worst_pct: f64,
    finite: bool,
}

impl PassResult {
    fn line(&self) -> String {
        format!(
            "median block {:>5.2}% of budget, p99 {:>5.2}%, worst {:>6.1}%, finite {}",
            self.median_pct, self.p99_pct, self.worst_pct, self.finite
        )
    }
}

/// Offline stress run: two real tracks, key lock + filters + sync + loops, with load/seek churn.
fn bench(paths: &[String]) -> Result<()> {
    use deckforge_common::{BeatFxKind, ColorFxKind};
    let a = deckforge_analysis::decode_file(std::path::Path::new(&paths[0]))?;
    let b = deckforge_analysis::decode_file(std::path::Path::new(&paths[1]))?;
    let ga = deckforge_analysis::analyze_beatgrid(&a, Default::default()).map(|x| x.grid);
    let gb = deckforge_analysis::analyze_beatgrid(&b, Default::default()).map(|x| x.grid);
    let (a, b) = (Arc::new(a), Arc::new(b));
    let tracks = (a, b, ga, gb);

    let (base, bpm_a, bpm_b, synced) = bench_best(&tracks, FxPass::Off)?;
    println!("60 s of audio (2 decks, key lock, sync, loops, filters, reloads), best of {BENCH_REPEATS}:");
    println!("  no FX        {}", base.line());
    println!("deck B synced={synced} bpm A {bpm_a:.2} B {bpm_b:.2}");

    println!("\nwith FX (beat FX on the master bus at level 1.0, one kind at a time):");
    for kind in BeatFxKind::ALL {
        let r = bench_best(&tracks, FxPass::Beat(kind))?.0;
        println!("  {:<12} {}", format!("{kind:?}"), r.line());
    }
    println!("colour FX (both CFX knobs engaged):");
    for kind in [ColorFxKind::Filter, ColorFxKind::Space, ColorFxKind::DubEcho, ColorFxKind::Crush, ColorFxKind::Noise] {
        let r = bench_best(&tracks, FxPass::Color(kind))?.0;
        println!("  {:<12} {}", format!("{kind:?}"), r.line());
    }
    let all = bench_best(&tracks, FxPass::AllRotating)?.0;
    println!("all FX on (beat FX rotating kinds/targets + colour FX on both channels):");
    println!("  {}", all.line());
    Ok(())
}

const BENCH_REPEATS: usize = 3;

/// Best of several passes, so scheduler noise from other processes does not dominate the numbers.
fn bench_best(tracks: &BenchTracks, fx: FxPass) -> Result<(PassResult, f64, f64, bool)> {
    let mut best = bench_pass(tracks, fx)?;
    for _ in 1..BENCH_REPEATS {
        let next = bench_pass(tracks, fx)?;
        best.0.median_pct = best.0.median_pct.min(next.0.median_pct);
        best.0.p99_pct = best.0.p99_pct.min(next.0.p99_pct);
        best.0.worst_pct = best.0.worst_pct.min(next.0.worst_pct);
        best.0.finite &= next.0.finite;
    }
    Ok(best)
}

type BenchTracks = (Arc<TrackAudio>, Arc<TrackAudio>, Option<deckforge_common::Beatgrid>, Option<deckforge_common::Beatgrid>);

fn bench_pass(tracks: &BenchTracks, fx: FxPass) -> Result<(PassResult, f64, f64, bool)> {
    use deckforge_common::{Band, BeatFxKind, ColorFxKind, FxTarget};
    use deckforge_engine::Routing;
    let (a, b, ga, gb) = tracks;
    let (h, mut rt) = create_engine(48_000);
    rt.reconfigure(48_000, Routing::for_channel_count(4));
    let req = |track: Arc<TrackAudio>, grid, token| Box::new(LoadRequest { track, token, grid, cue_secs: None, hot_cues: [None; HOT_CUE_SLOTS] });
    h.send(Command::Load { deck: 0, request: req(a.clone(), *ga, 1) });
    h.send(Command::Load { deck: 1, request: req(b.clone(), *gb, 2) });
    let knob = |d: usize| if d == 0 { -0.4 } else { 0.4 };
    for d in 0..2 {
        h.send(Command::Mixer { channel: d, param: MixerParam::Fader(1.0) });
        h.send(Command::Mixer { channel: d, param: MixerParam::Filter(knob(d)) });
        h.send(Command::Mixer { channel: d, param: MixerParam::Eq(Band::Low, 0.3) });
        h.send(Command::SetKeyLock { deck: d, enabled: true });
        h.send(Command::SetTempo { deck: d, ratio: 1.04 });
        h.send(Command::Play { deck: d });
    }
    h.send(Command::Mixer { channel: 1, param: MixerParam::Pfl(true) });
    h.send(Command::Sync { deck: 1, enabled: true });
    match fx {
        FxPass::Off => {}
        FxPass::Beat(kind) => {
            h.send(Command::BeatFxKind(kind));
            h.send(Command::BeatFxTarget(FxTarget::Master));
            h.send(Command::BeatFxBeats(0.5));
            h.send(Command::BeatFxLevel(1.0));
            h.send(Command::BeatFxEnabled(true));
        }
        FxPass::Color(kind) => {
            h.send(Command::ColorFx(kind));
            for d in 0..2 {
                h.send(Command::Mixer { channel: d, param: MixerParam::Filter(if d == 0 { -0.8 } else { 0.8 }) });
            }
        }
        FxPass::AllRotating => {
            h.send(Command::BeatFxLevel(1.0));
            h.send(Command::BeatFxEnabled(true));
            h.send(Command::ColorFx(ColorFxKind::Space));
            for d in 0..2 {
                h.send(Command::Mixer { channel: d, param: MixerParam::Filter(if d == 0 { -0.8 } else { 0.8 }) });
            }
        }
    }
    let block = 256;
    let mut buf = vec![0f32; block * 4];
    let blocks = 48_000 * 60 / block;
    let mut times = Vec::with_capacity(blocks);
    for i in 0..blocks {
        if i % 400 == 0 {
            h.send(Command::BeatLoop { deck: 0, beats: 1.0 });
            h.send(Command::BeatJump { deck: 1, beats: 4.0 });
        }
        if i % 400 == 200 {
            h.send(Command::ReloopExit { deck: 0 });
            h.send(Command::HotCue { deck: 1, slot: 0, pressed: true });
            h.send(Command::HotCue { deck: 1, slot: 0, pressed: false });
        }
        if i % 2000 == 1000 {
            h.send(Command::Load { deck: 0, request: req(a.clone(), *ga, 1) });
            h.send(Command::Play { deck: 0 });
        }
        if matches!(fx, FxPass::AllRotating) {
            if i % 600 == 0 {
                h.send(Command::BeatFxKind(BeatFxKind::ALL[(i / 600) % BeatFxKind::ALL.len()]));
                h.send(Command::BeatFxTarget([FxTarget::Master, FxTarget::A, FxTarget::B][(i / 1800) % 3]));
            }
            if i % 900 == 0 {
                h.send(Command::ColorFx([ColorFxKind::Space, ColorFxKind::DubEcho, ColorFxKind::Crush, ColorFxKind::Noise][(i / 900) % 4]));
            }
        }
        let t = std::time::Instant::now();
        rt.process(&mut buf, 4);
        times.push(t.elapsed());
        h.collect_garbage();
    }
    times.sort_unstable();
    let budget = Duration::from_secs_f64(block as f64 / 48_000.0).as_secs_f64();
    let pct = |d: Duration| d.as_secs_f64() / budget * 100.0;
    let s = h.snapshot();
    let result = PassResult {
        median_pct: pct(times[times.len() / 2]),
        p99_pct: pct(times[times.len() * 99 / 100]),
        worst_pct: pct(*times.last().unwrap_or(&Duration::ZERO)),
        finite: buf.iter().all(|v| v.is_finite()),
    };
    Ok((result, s.decks[0].bpm, s.decks[1].bpm, s.decks[1].sync_enabled))
}

/// Creates/extends an isolated library (for `DECKFORGE_DATA_DIR`) and optionally pins its audio output.
fn seed(dir: &str, music: &str, device_match: Option<&str>) -> Result<()> {
    let lib = deckforge_library::Library::open(&std::path::Path::new(dir).join("library.sqlite"))?;
    let r = lib.import_paths(&[std::path::PathBuf::from(music)]);
    println!("imported {} (skipped {}, failed {})", r.added, r.skipped, r.failed);
    if let Some(m) = device_match {
        let dev = list_output_devices().into_iter().find(|d| d.name.contains(m)).context("device not found")?;
        lib.set_setting("audio.output", &OutputRequest { device_id: Some(dev.id.clone()), ..Default::default() })?;
        println!("audio output pinned to {}", dev.name);
    }
    Ok(())
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("devices") => devices(),
        Some("seed") if args.len() >= 3 => seed(&args[1], &args[2], args.get(3).map(String::as_str))?,
        Some("bench") if args.len() >= 3 => bench(&args[1..3])?,
        Some("fx-demos") if args.len() >= 3 => fx_demos::run(&args[1], &args[2], args.get(3).and_then(|v| v.parse().ok()).unwrap_or(60.0), args.get(4).map(String::as_str))?,
        Some("ddj400-sim") => ddj400_sim(args.get(1).map(String::as_str).unwrap_or("BlackHole 16ch"))?,
        Some("secondary") => {
            let arg = |i: usize| args.get(i).map(String::as_str);
            secondary_check(
                arg(1).unwrap_or("BlackHole 16ch"),
                arg(2).unwrap_or("BlackHole 2ch"),
                arg(3).and_then(|v| v.parse().ok()).unwrap_or(60.0),
                arg(4).and_then(|v| v.parse().ok()).unwrap_or(0.8),
                arg(5).and_then(|v| v.parse().ok()).unwrap_or(0.8),
            )?
        }
        Some("secondary-swap") => secondary_swap(
            args.get(1).map(String::as_str).unwrap_or("BlackHole 16ch"),
            args.get(2).map(String::as_str).unwrap_or("DDJ-400"),
            args.get(3).map(String::as_str).unwrap_or("BlackHole 2ch"),
        )?,
        Some("loopback") => loopback(args.get(1).map(String::as_str).unwrap_or("BlackHole 16ch"))?,
        _ => println!("usage: deckforge-cli devices | loopback [device] | ddj400-sim [device] | secondary [primary] [secondary] [secs] [level] [master] | secondary-swap [first] [second] [secondary] | bench <a> <b> | fx-demos <track> <out-dir> [start-secs] [file-label]"),
    }
    Ok(())
}
