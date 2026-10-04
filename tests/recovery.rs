use shr_lux::{fixture::*, lighting_contract::StoredKind, recovery::*};
use std::{os::unix::fs::PermissionsExt, path::PathBuf};
const SHOW: &str = "11111111-1111-4111-8111-111111111111";
fn directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("lux-recovery-{name}-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn state(epoch: u64, value: i32) -> DurableState {
    let id = Id::new("fixture-11").unwrap();
    let v = Value {
        fixture: id.clone(),
        attribute: Attribute::Intensity,
        value,
    };
    DurableState {
        format: "shr-lux-checkpoint".into(),
        version: 1,
        show_id: SHOW.into(),
        epoch,
        revision: 3,
        patch: PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![FixtureSpec::synthetic(
                id,
                "Synthetic",
                SyntheticMode::Dimmer,
                1,
            )],
        },
        stores: vec![SavedLook {
            kind: StoredKind::Cue,
            id: Id::new("cue").unwrap(),
            values: vec![v.clone()],
        }],
        manual_hold: vec![v.clone()],
        intended_look: vec![v],
        master: 500,
        fixture_masters: vec![],
        blackout: true,
    }
}
#[test]
fn consecutive_crash_epochs_without_new_checkpoint_and_exclusive_owner() {
    let path = directory("epochs");
    let store = CheckpointStore::open(&path, SHOW).unwrap();
    assert_eq!(store.epoch(), 1);
    assert!(store.load().unwrap().is_none());
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    store.checkpoint(&state(1, 333)).unwrap();
    drop(store);
    let store = CheckpointStore::open(&path, SHOW).unwrap();
    assert_eq!(store.epoch(), 2);
    assert_eq!(store.load().unwrap().unwrap().intended_look[0].value, 333);
    drop(store);
    let store = CheckpointStore::open(&path, SHOW).unwrap();
    assert_eq!(store.epoch(), 3);
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.epoch, 1);
    assert!(loaded.blackout);
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn before_after_rename_failure_honest_and_owned_scratch_cleanup() {
    let path = directory("faults");
    let store = CheckpointStore::open(&path, SHOW).unwrap();
    store.checkpoint(&state(1, 333)).unwrap();
    let file = path.join("lighting-checkpoint.json");
    let old = std::fs::read(&file).unwrap();
    assert!(
        store
            .checkpoint_with_fault(&state(1, 444), WriteFault::BeforeRename)
            .is_err()
    );
    assert_eq!(std::fs::read(&file).unwrap(), old);
    assert!(
        store
            .checkpoint_with_fault(&state(1, 555), WriteFault::AfterRename)
            .is_err()
    );
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(json["intended_look"][0]["value"], 555);
    assert!(
        !std::fs::read_dir(&path).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn mismatch_future_duplicate_symlink_and_missing_registry_preserve_files() {
    let path = directory("reject");
    let store = CheckpointStore::open(&path, SHOW).unwrap();
    store.checkpoint(&state(1, 333)).unwrap();
    drop(store);
    let file = path.join("lighting-checkpoint.json");
    let old = std::fs::read(&file).unwrap();
    let epoch = std::fs::read(path.join("lighting-epoch.json")).unwrap();
    assert!(CheckpointStore::open(&path, "22222222-2222-4222-8222-222222222222").is_err());
    assert_eq!(std::fs::read(&file).unwrap(), old);
    assert_eq!(
        std::fs::read(path.join("lighting-epoch.json")).unwrap(),
        epoch
    );
    let mut future: serde_json::Value = serde_json::from_slice(&old).unwrap();
    future["version"] = serde_json::json!(2);
    std::fs::write(&file, serde_json::to_vec(&future).unwrap()).unwrap();
    let bad = std::fs::read(&file).unwrap();
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), bad);
    std::fs::write(&file, &old).unwrap();
    std::fs::remove_file(path.join("lighting-epoch.json")).unwrap();
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), old);
    std::fs::write(path.join("lighting-epoch.json"), &epoch).unwrap();
    std::fs::set_permissions(
        path.join("lighting-epoch.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink("lighting-epoch.json", &file).unwrap();
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    std::fs::remove_file(&file).unwrap();
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn real_authority_checkpoint_freezes_programmer_and_mid_release_without_transient_replay() {
    use shr_lux::{
        authority::{Authority, Command},
        wire::{SCOPE, Service},
    };
    let path = directory("owner");
    let store = std::sync::Arc::new(CheckpointStore::open(&path, SHOW).unwrap());
    let data = state(store.epoch(), 700);
    let mut e = Authority::new(data.validate(SHOW).unwrap(), SHOW, store.epoch())
        .unwrap()
        .with_stores(
            data.stores
                .iter()
                .map(|s| shr_lux::authority::StoredLook {
                    kind: s.kind,
                    id: s.id.clone(),
                    values: s.values.clone(),
                })
                .collect(),
        )
        .unwrap();
    let f = Id::new("fixture-11").unwrap();
    let value = |v| Value {
        fixture: f.clone(),
        attribute: Attribute::Intensity,
        value: v,
    };
    e.execute(Command::Go {
        cue: Id::new("cue").unwrap(),
        playback: Id::new("playback").unwrap(),
    })
    .unwrap();
    e.execute(Command::Touch(vec![value(0)])).unwrap();
    e.execute(Command::ClearToHold).unwrap();
    e.execute(Command::Master(500)).unwrap();
    e.execute(Command::Blackout(true)).unwrap();
    let token = e.preview_release(&[value(0)]).unwrap();
    e.release_commit(&token).unwrap();
    e.advance(25).unwrap();
    assert_eq!(e.snapshot().fixtures[0].attributes[0].resolved, 350);
    let saved = e.durable_state();
    assert!(!e.transition_status().is_null());
    store.checkpoint(&saved).unwrap();
    drop(store);
    let store = std::sync::Arc::new(CheckpointStore::open(&path, SHOW).unwrap());
    let mut s = Service::recover(store.clone(), data.validate(SHOW).unwrap()).unwrap();
    let inventory = s.inventory().unwrap();
    assert_eq!(inventory["snapshot"]["epoch"], "2");
    assert_eq!(
        inventory["snapshot"]["fixtures"][0]["attributes"][0]["hold"],
        350
    );
    assert_eq!(
        inventory["snapshot"]["fixtures"][0]["attributes"][0]["programmer"],
        serde_json::Value::Null
    );
    assert_eq!(
        inventory["authority_inventory"]["playbacks"],
        serde_json::json!([])
    );
    assert!(inventory["release"]["transition"].is_null());
    assert_eq!(inventory["blackout"], true);
    assert_eq!(inventory["output"], "null_disarmed");
    assert_eq!(inventory["physical"], "unknown");
    let request = serde_json::json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":"2","writer":"old-writer","lease":"1","request_id":"2","expected_revision":"0","kind":"command","body":{"scope":SCOPE,"command":{"action":"master","level":0}}});
    let replies = s.handle(&serde_json::to_vec(&request).unwrap());
    assert_eq!(replies[0]["reason"], "lease");
    let corpus = serde_json::json!({"contract":"C-LIGHT:1","wire_schema":"lx04-durable-v1","checkpoint":saved,"recovered":inventory,"input":request,"replies":replies,"after":s.inventory().unwrap()});
    let corpus_path = "tests/fixtures/lx04/v1/recovery.json";
    if std::env::var_os("LUX_GENERATE_LX04").is_some() {
        std::fs::write(
            corpus_path,
            serde_json::to_string_pretty(&corpus).unwrap() + "\n",
        )
        .unwrap();
    } else {
        assert_eq!(
            corpus,
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(corpus_path).unwrap())
                .unwrap()
        );
    }
    s.advance(500).unwrap();
    assert_eq!(
        s.inventory().unwrap()["snapshot"]["fixtures"][0]["attributes"][0]["hold"],
        350
    );
    drop(s);
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn real_cli_private_service_crash_restart_and_explicit_checkpoint() {
    use serde_json::{Value as Json, json};
    use shr_lux::local_service::{read_frame, write_frame};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command, Stdio};
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let path = directory("cli");
    let start = || {
        let child = OwnedChild(
            Command::new(env!("CARGO_BIN_EXE_lux-service"))
                .args([
                    "--synthetic-private-dir",
                    path.to_str().unwrap(),
                    "--durable",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        for _ in 0..100 {
            if let Ok(stream) = UnixStream::connect(path.join("lux.sock")) {
                return (child, stream);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("own CLI did not bind");
    };
    let request = |epoch: u64,
                   writer: Option<&str>,
                   lease: Option<&str>,
                   id: Option<u64>,
                   revision: Option<u64>,
                   kind: &str,
                   body: Json| json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":epoch.to_string(),"writer":writer,"lease":lease,"request_id":id.map(|v|v.to_string()),"expected_revision":revision.map(|v|v.to_string()),"kind":kind,"body":body});
    let send = |stream: &mut UnixStream, input: Json| {
        write_frame(stream, &serde_json::to_vec(&input).unwrap()).unwrap();
        serde_json::from_slice::<Json>(&read_frame(stream).unwrap()).unwrap()
    };
    let read = |stream: &mut UnixStream, epoch| {
        let page = send(
            stream,
            request(epoch, None, None, None, None, "snapshot", json!({})),
        );
        assert_eq!(page["kind"], "snapshot");
        assert_eq!(page["body"]["page_count"], 1);
        serde_json::from_str::<Json>(page["body"]["chunk"].as_str().unwrap()).unwrap()
    };
    let (mut child, mut stream) = start();
    assert_eq!(read(&mut stream, 1)["wire_schema"], "lx04-durable-v1");
    assert_eq!(
        send(
            &mut stream,
            request(
                1,
                Some("writer"),
                None,
                Some(1),
                Some(0),
                "grant",
                json!({"scope":"lighting-control"})
            )
        )["kind"],
        "applied"
    );
    let actions = vec![
        json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":333}]}),
        json!({"action":"record","kind":"cue","id":"cue"}),
        json!({"action":"clear_to_hold"}),
        json!({"action":"master","level":500}),
        json!({"action":"blackout","enabled":true}),
        json!({"action":"checkpoint"}),
    ];
    let mut revision = 0;
    for (i, action) in actions.into_iter().enumerate() {
        let reply = send(
            &mut stream,
            request(
                1,
                Some("writer"),
                Some("1"),
                Some(i as u64 + 2),
                Some(revision),
                "command",
                json!({"scope":"lighting-control","command":action}),
            ),
        );
        assert_eq!(reply["kind"], "applied");
        revision = reply["revision"].as_str().unwrap().parse().unwrap();
    }
    assert_eq!(
        read(&mut stream, 1)["snapshot"]["durability"],
        "checkpointed"
    );
    drop(stream);
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    drop(child);
    // Explicitly remove only this test's own stale socket after its child has exited.
    std::fs::remove_file(path.join("lux.sock")).unwrap();
    for epoch in [2, 3] {
        let (mut child, mut stream) = start();
        let inventory = read(&mut stream, epoch);
        assert_eq!(
            inventory["snapshot"]["fixtures"][0]["attributes"][0]["hold"],
            333
        );
        assert_eq!(inventory["master"], 500);
        assert_eq!(inventory["blackout"], true);
        assert_eq!(inventory["authority_inventory"]["playbacks"], json!([]));
        assert_eq!(inventory["grants"], json!([]));
        assert_eq!(inventory["physical"], "unknown");
        let old = request(
            1,
            Some("writer"),
            Some("1"),
            Some(2),
            Some(0),
            "command",
            json!({"scope":"lighting-control","command":{"action":"master","level":0}}),
        );
        assert_eq!(send(&mut stream, old)["reason"], "epoch");
        drop(stream);
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        drop(child);
        std::fs::remove_file(path.join("lux.sock")).unwrap();
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn malformed_duplicate_and_counter_overflow_checkpoint_never_adopted() {
    let path = directory("malformed");
    let store = CheckpointStore::open(&path, SHOW).unwrap();
    store.checkpoint(&state(1, 333)).unwrap();
    drop(store);
    let file = path.join("lighting-checkpoint.json");
    let old = std::fs::read(&file).unwrap();
    let mut duplicate = b"{\"version\":1,".to_vec();
    duplicate.extend_from_slice(&old[1..]);
    std::fs::write(&file, &duplicate).unwrap();
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), duplicate);
    let mut invalid: serde_json::Value = serde_json::from_slice(&old).unwrap();
    invalid["epoch"] = serde_json::json!("18446744073709551616");
    std::fs::write(&file, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn every_cli_mode_reserves_epochs_across_crashes_without_checkpoint() {
    use serde_json::{Value as Json, json};
    use shr_lux::local_service::{read_frame, write_frame};
    use std::{
        os::unix::net::UnixStream,
        process::{Child, Command, Stdio},
    };
    struct Owned(Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for mode in [None, Some("--timed")] {
        let path = directory(if mode.is_some() {
            "epoch-timed"
        } else {
            "epoch-static"
        });
        for epoch in [1, 2, 3] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_lux-service"));
            cmd.args(["--synthetic-private-dir", path.to_str().unwrap()]);
            if let Some(mode) = mode {
                cmd.arg(mode);
            }
            let mut child = Owned(
                cmd.stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let mut stream = (0..100)
                .find_map(|_| {
                    let s = UnixStream::connect(path.join("lux.sock")).ok();
                    if s.is_none() {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    s
                })
                .expect("own CLI bind");
            let input = json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":epoch.to_string(),"writer":null,"lease":null,"request_id":null,"expected_revision":null,"kind":"snapshot","body":{}});
            write_frame(&mut stream, &serde_json::to_vec(&input).unwrap()).unwrap();
            let reply: Json = serde_json::from_slice(&read_frame(&mut stream).unwrap()).unwrap();
            assert_eq!(reply["kind"], "snapshot");
            assert_eq!(reply["epoch"], epoch.to_string());
            drop(stream);
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            drop(child);
            std::fs::remove_file(path.join("lux.sock")).unwrap();
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn exhausted_durable_epoch_refuses_without_rewriting_registry() {
    let path = directory("epoch-overflow");
    drop(CheckpointStore::open(&path, SHOW).unwrap());
    let file = path.join("lighting-epoch.json");
    let mut epoch: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    epoch["epoch"] = serde_json::json!(u64::MAX.to_string());
    let bytes = serde_json::to_vec(&epoch).unwrap();
    std::fs::write(&file, &bytes).unwrap();
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn durable_actual_service_commands_pages_error_restart_and_cache_corpus() {
    use serde_json::{Value as Json, json};
    use shr_lux::wire::{SCOPE, Service};
    let path = directory("service-corpus");
    let store = std::sync::Arc::new(CheckpointStore::open(&path, SHOW).unwrap());
    let patch = state(1, 700).validate(SHOW).unwrap();
    let mut service = Service::recover(store.clone(), patch.clone()).unwrap();
    fn request(
        epoch: u64,
        kind: &str,
        id: Option<u64>,
        revision: Option<u64>,
        lease: Option<&str>,
        body: Json,
    ) -> Json {
        json!({"contract":"C-LIGHT","version":1,"show_id":SHOW,"module":"lighting","epoch":epoch.to_string(),"writer":id.map(|_|"writer"),"lease":lease,"request_id":id.map(|v|v.to_string()),"expected_revision":revision.map(|v|v.to_string()),"kind":kind,"body":body})
    }
    fn observe(service: &mut Service) -> Json {
        let input = request(
            service.engine().snapshot().epoch,
            "snapshot",
            None,
            None,
            None,
            json!({}),
        );
        let pages = service.handle(&serde_json::to_vec(&input).unwrap());
        assert!(pages.iter().all(|p| p["kind"] == "snapshot"));
        let text = pages
            .iter()
            .map(|p| p["body"]["chunk"].as_str().unwrap())
            .collect::<String>();
        let decoded: Json = serde_json::from_str(&text).unwrap();
        assert_eq!(decoded, service.inventory().unwrap());
        json!({"input":input,"pages":pages,"inventory":decoded})
    }
    fn transact(service: &mut Service, input: Json, tick: u64, cases: &mut Vec<Json>) -> Json {
        service.advance(tick).unwrap();
        let prior = observe(service);
        let replies = service.handle(&serde_json::to_vec(&input).unwrap());
        let after = observe(service);
        cases.push(json!({"tick":tick.to_string(),"input":input,"prior":prior,"replies":replies,"after":after}));
        replies[0].clone()
    }
    let mut cases = Vec::new();
    transact(
        &mut service,
        request(1, "grant", Some(1), Some(0), None, json!({"scope":SCOPE})),
        0,
        &mut cases,
    );
    let actions = [
        json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":700}]}),
        json!({"action":"record","kind":"cue","id":"cue"}),
        json!({"action":"go","cue":"cue","playback":"playback"}),
        json!({"action":"touch","values":[{"fixture":"fixture-11","attribute":"intensity","value":0}]}),
        json!({"action":"clear_to_hold"}),
        json!({"action":"master","level":500}),
        json!({"action":"blackout","enabled":true}),
        json!({"action":"release_preview","values":[{"fixture":"fixture-11","attribute":"intensity","value":0}]}),
    ];
    let mut preview = Json::Null;
    for (i, action) in actions.into_iter().enumerate() {
        let input = request(
            1,
            "command",
            Some(i as u64 + 2),
            Some(service.engine().revision()),
            Some("1"),
            json!({"scope":SCOPE,"command":action}),
        );
        let reply = transact(&mut service, input, 0, &mut cases);
        assert_eq!(reply["kind"], "applied");
        if i == 7 {
            preview = reply["body"]["token"].clone();
        }
    }
    let mut commit = request(
        1,
        "command",
        Some(10),
        Some(service.engine().revision()),
        Some("1"),
        json!({"scope":SCOPE,"command":{"action":"release_commit","token":preview}}),
    );
    let original = transact(&mut service, commit.clone(), 0, &mut cases);
    assert_eq!(original["kind"], "applied");
    service.advance(25).unwrap();
    assert_eq!(
        service.engine().snapshot().fixtures[0].attributes[0].resolved,
        350
    );
    assert_eq!(
        transact(&mut service, commit.clone(), 25, &mut cases),
        original
    );
    let checkpoint = request(
        1,
        "command",
        Some(11),
        Some(service.engine().revision()),
        Some("1"),
        json!({"scope":SCOPE,"command":{"action":"checkpoint"}}),
    );
    assert_eq!(
        transact(&mut service, checkpoint.clone(), 25, &mut cases)["kind"],
        "applied"
    );
    let checkpoint_file = path.join("lighting-checkpoint.json");
    let bytes = std::fs::read(&checkpoint_file).unwrap();
    let saved: Json = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved["intended_look"][0]["value"], 350);
    std::fs::write(&checkpoint_file, b"{\"version\":99}").unwrap();
    let failed = request(
        1,
        "command",
        Some(12),
        Some(service.engine().revision()),
        Some("1"),
        json!({"scope":SCOPE,"command":{"action":"checkpoint"}}),
    );
    assert_eq!(
        transact(&mut service, failed, 25, &mut cases)["reason"],
        "durability"
    );
    assert_eq!(
        service.inventory().unwrap()["snapshot"]["durability"],
        "error"
    );
    // Exact cached checkpoint is immutable, and does not falsely clear a newer error.
    assert_eq!(
        transact(&mut service, checkpoint, 25, &mut cases)["kind"],
        "applied"
    );
    assert_eq!(
        service.inventory().unwrap()["snapshot"]["durability"],
        "error"
    );
    std::fs::write(&checkpoint_file, &bytes).unwrap();
    drop(service);
    drop(store);
    let store = std::sync::Arc::new(CheckpointStore::open(&path, SHOW).unwrap());
    let mut service = Service::recover(store.clone(), patch).unwrap();
    let recovered = observe(&mut service);
    assert_eq!(recovered["inventory"]["snapshot"]["epoch"], "2");
    assert_eq!(
        recovered["inventory"]["snapshot"]["fixtures"][0]["attributes"][0]["hold"],
        350
    );
    assert_eq!(
        recovered["inventory"]["authority_inventory"]["playbacks"],
        json!([])
    );
    assert_eq!(
        transact(&mut service, commit.clone(), 0, &mut cases)["reason"],
        "epoch"
    );
    commit["epoch"] = json!("2");
    assert_eq!(
        transact(&mut service, commit, 0, &mut cases)["reason"],
        "lease"
    );
    transact(
        &mut service,
        request(2, "grant", Some(1), Some(0), None, json!({"scope":SCOPE})),
        0,
        &mut cases,
    );
    let expired_preview = request(
        2,
        "command",
        Some(2),
        Some(0),
        Some("1"),
        json!({"scope":SCOPE,"command":{"action":"release_commit","token":preview}}),
    );
    assert_eq!(
        transact(&mut service, expired_preview, 0, &mut cases)["reason"],
        "target"
    );
    service.advance(500).unwrap();
    assert_eq!(
        service.engine().snapshot().fixtures[0].attributes[0].resolved,
        350
    );
    let corpus = json!({"contract":"C-LIGHT:1","wire_schema":"lx04-durable-v1","checkpoint":saved,"recovered":recovered,"cases":cases,"after_late_tick":observe(&mut service)});
    let corpus_path = "tests/fixtures/lx04/v1/durable-commands.json";
    fn normalise(v: &mut Json) {
        match v {
            Json::Object(m) => {
                for (k, v) in m {
                    if k == "nonce" {
                        *v = json!("engine-issued-nonce")
                    } else {
                        normalise(v)
                    }
                }
            }
            Json::Array(a) => {
                for v in a {
                    normalise(v)
                }
            }
            _ => {}
        }
    }
    if std::env::var_os("LUX_GENERATE_LX04").is_some() {
        std::fs::write(
            corpus_path,
            serde_json::to_string_pretty(&corpus).unwrap() + "\n",
        )
        .unwrap();
    } else {
        let mut expected: Json =
            serde_json::from_slice(&std::fs::read(corpus_path).unwrap()).unwrap();
        let mut actual = corpus;
        normalise(&mut expected);
        normalise(&mut actual);
        assert_eq!(actual, expected);
    }
    drop(service);
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn non_regular_checkpoint_refuses_without_blocking_or_removing_it() {
    use std::os::unix::ffi::OsStrExt;
    let path = directory("fifo");
    drop(CheckpointStore::open(&path, SHOW).unwrap());
    let checkpoint = path.join("lighting-checkpoint.json");
    let name = std::ffi::CString::new(checkpoint.as_os_str().as_bytes()).unwrap();
    // SAFETY: NUL-terminated owned pathname; create a private synthetic FIFO only.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(CheckpointStore::open(&path, SHOW).is_err());
    use std::os::unix::fs::FileTypeExt;
    assert!(
        std::fs::symlink_metadata(&checkpoint)
            .unwrap()
            .file_type()
            .is_fifo()
    );
    std::fs::remove_file(checkpoint).unwrap();
    std::fs::remove_dir_all(path).unwrap();
}
