use serde_json::{Value, json};
use shr_lux::analysis_subscription::{AnalysisState, Descriptor, decode_window};
fn corpus() -> Value {
    serde_json::from_str(include_str!("fixtures/lx05/v1/e09.json")).unwrap()
}
fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn descriptor() -> Descriptor {
    Descriptor::decode(&serde_json::to_vec(&corpus()["descriptor"]).unwrap()).unwrap()
}
fn window(first: u64, seq: u32, now: u64) -> Vec<u8> {
    let c = corpus();
    let mut b = b"GAW1".to_vec();
    b.extend(0u64.to_be_bytes());
    b.extend(1u64.to_be_bytes());
    b.extend(2u64.to_be_bytes());
    b.extend(now.to_be_bytes());
    b.extend([0; 4]);
    for n in 0..10 {
        let mut p = bytes(c["packets_hex"][n].as_str().unwrap());
        p[24..32].copy_from_slice(&(first + n as u64 * 48).to_be_bytes());
        p[32..36].copy_from_slice(&(seq + n as u32).to_be_bytes());
        b.extend(p);
    }
    b
}
#[test]
fn exact_provider_data_pcm24_and_strict_identity() {
    let d = descriptor();
    let b = window(48000, 0, 1000);
    let w = decode_window(&d, &b, 1100).unwrap();
    assert_eq!(w.first, 48000);
    assert_eq!(
        w.pcm[0],
        [28672., 57344., 86016., 114688.].map(|x| x / 8388608.)
    );
    for (key, value) in [
        ("sample_rate", json!(44100)),
        ("stream", json!(2)),
        ("tap", json!("post-fader")),
        ("sources", json!(["bass", "kick", "guitar-1", "guitar-2"])),
        ("source_epoch", json!("09")),
        ("map_revision", json!("0")),
        ("calibration_revision", json!("0")),
        ("first_frame", json!("48001")),
        ("extra", json!(true)),
    ] {
        let mut v = corpus()["descriptor"].clone();
        v[key] = value;
        assert!(Descriptor::decode(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    assert!(decode_window(&d, &b, 999).is_err());
    assert!(decode_window(&d, &b, 1101).is_err());
    for offset in [0, 36, 40, 44, 48, 56, 76, 80] {
        let mut changed = b.clone();
        changed[offset] ^= 1;
        assert!(
            decode_window(&d, &changed, 1000).is_err(),
            "offset {offset}"
        );
    }
}

#[test]
fn first_delivered_window_is_bound_to_descriptor_origin_and_sequence() {
    let d = descriptor();
    // Late subscribers may skip whole windows, preserving source sequence.
    assert!(decode_window(&d, &window(48960, 20, 1000), 1000).is_ok());
    for (frame, sequence) in [(48001, 0), (48048, 1), (48480, 0), (48000, 10)] {
        assert!(
            decode_window(&d, &window(frame, sequence, 1000), 1000).is_err(),
            "frame {frame}, sequence {sequence}"
        );
    }
}
#[test]
fn fresh_explicit_calibration_known_energy_and_loss_refusals() {
    let mut s = AnalysisState::default();
    s.attach(descriptor());
    s.ingest(&window(48000, 0, 1000), 1000).unwrap();
    assert!(!s.ready());
    assert!(s.calibrate("finish", 1000).is_err());
    s.calibrate("start", 1000).unwrap();
    for n in 1..=50 {
        s.ingest(
            &window(48000 + n * 480, n as u32 * 10, 1000 + n * 10),
            1000 + n * 10,
        )
        .unwrap();
    }
    s.calibrate("finish", 1500).unwrap();
    for n in 51..=110 {
        s.ingest(
            &window(48000 + n * 480, n as u32 * 10, 1000 + n * 10),
            1000 + n * 10,
        )
        .unwrap();
    }
    assert!(s.ready());
    let status = s.status(2100);
    assert!(status["energy_millionths"].as_u64().unwrap() > 100000);
    assert_eq!(status["confidence"], 1000);
    assert!(status["beat"].is_null());
    assert!(status["downbeat"].is_null());
    let mut b = window(48000 + 111 * 480, 1110, 2110);
    b[4..12].copy_from_slice(&1u64.to_be_bytes());
    assert!(s.ingest(&b, 2110).is_err());
    assert!(!s.ready());
    assert_eq!(s.reason, Some("lost_windows"));
    assert!(s.calibrate("finish", 2110).is_err());
}
#[test]
fn gaps_overlap_mapping_epoch_clock_and_staleness_invalidate() {
    for fault in ["gap", "overlap", "map", "epoch", "calibration", "clock"] {
        let mut s = AnalysisState::default();
        s.attach(descriptor());
        s.ingest(&window(48000, 0, 1000), 1000).unwrap();
        s.calibrate("start", 1000).unwrap();
        let mut b = window(48480, 10, 1010);
        match fault {
            "gap" => b = window(48960, 20, 1010),
            "overlap" => b = window(48000, 0, 1010),
            "map" => b[19] ^= 1,
            "epoch" => b[63] ^= 1,
            "calibration" => b[27] ^= 1,
            "clock" => b[28..36].copy_from_slice(&999u64.to_be_bytes()),
            _ => unreachable!(),
        }
        assert!(s.ingest(&b, 1010).is_err(), "{fault}");
        assert!(!s.ready());
        assert!(s.calibrate("finish", 1010).is_err());
    }
    let mut s = AnalysisState::default();
    s.attach(descriptor());
    s.ingest(&window(48000, 0, 1000), 1000).unwrap();
    s.expire(1101);
    assert_eq!(s.state, "stale");
    assert_eq!(s.status(1101)["source_age_ms"], 101);
}
#[test]
fn silent_source_never_calibrates_as_ready() {
    let mut s = AnalysisState::default();
    s.attach(descriptor());
    s.ingest(&window(48000, 0, 1000), 1000).unwrap();
    s.calibrate("start", 1000).unwrap();
    for n in 1..=50 {
        let mut b = window(48000 + n * 480, n as u32 * 10, 1000 + n * 10);
        for p in b[40..].chunks_exact_mut(624) {
            p[48..].fill(0);
        }
        s.ingest(&b, 1000 + n * 10).unwrap();
    }
    assert!(s.calibrate("finish", 1500).is_err());
    assert!(!s.ready());
}

#[test]
fn disconnect_and_reattach_never_relabel_old_source_observations() {
    let mut s = AnalysisState::default();
    s.attach(descriptor());
    s.ingest(&window(48000, 0, 1000), 1000).unwrap();
    assert!(!s.status(1000)["source_window_range"].is_null());
    s.absent();
    // This is the LX05 consumer contract: a missing descriptor cannot carry a
    // window. Expiry must also not turn detached state back into stale state.
    s.expire(1200);
    let absent = s.status(1200);
    assert_eq!(absent["state"], "absent");
    for key in ["source_identity", "source_window_range", "source_age_ms"] {
        assert!(absent[key].is_null(), "{key}");
    }

    s.attach(descriptor());
    s.ingest(&window(48000, 0, 1300), 1300).unwrap();
    let mut next = descriptor();
    next.source_epoch = "10".into();
    next.first_frame = "96000".into();
    s.attach(next);
    let attached = s.status(1300);
    assert_eq!(attached["source_epoch"], "10");
    assert!(attached["source_window_range"].is_null());
    assert!(attached["source_age_ms"].is_null());
    assert_eq!(attached["calibration_windows"], 0);
    assert!(!s.ready());
    assert!(s.calibrate("start", 1300).is_err());
}
#[test]
fn full_private_listener_backlog_cannot_block_client_drop() {
    use shr_lux::analysis_subscription::Client;
    use std::{
        os::{
            fd::AsRawFd,
            unix::net::{UnixListener, UnixStream},
        },
        time::{Duration, Instant},
    };
    let dir = std::env::temp_dir().join(format!("lux-analysis-backlog-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("analysis.sock");
    let listener = UnixListener::bind(&path).unwrap();
    // SAFETY: owned listening socket, finite backlog; only this test endpoint changes.
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
    let one = UnixStream::connect(&path).unwrap();
    let two = UnixStream::connect(&path).unwrap();
    let client = Client::start(&path).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let start = Instant::now();
    drop(client);
    assert!(start.elapsed() < Duration::from_millis(300));
    drop(one);
    drop(two);
    drop(listener);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
