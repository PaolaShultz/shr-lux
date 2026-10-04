#![cfg(target_os = "linux")]
use shr_lux::{authority::Authority, fixture::*, local_service::*, wire::Service};
use std::{
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    time::Duration,
};
fn request() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"contract":"C-LIGHT","version":1,"show_id":"11111111-1111-4111-8111-111111111111","module":"lighting","epoch":"9","writer":null,"lease":null,"request_id":null,"expected_revision":null,"kind":"snapshot","body":{}})).unwrap()
}
#[test]
fn private_fragmented_oversize_timeout_reconnect_and_cleanup() {
    let path = std::env::temp_dir().join(format!("lux-ipc-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let engine = Authority::new(
        Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![FixtureSpec::synthetic(
                Id::new("f1").unwrap(),
                "Synthetic",
                SyntheticMode::Dimmer,
                1,
            )],
        })
        .unwrap(),
        "11111111-1111-4111-8111-111111111111",
        9,
    )
    .unwrap();
    let mut server = LocalServer::bind(&path, Service::new(engine.clone()).unwrap()).unwrap();
    assert!(LocalServer::bind(&path, Service::new(engine).unwrap()).is_err());
    let mut client = UnixStream::connect(server.socket()).unwrap();
    let bytes = request();
    for byte in (bytes.len() as u32)
        .to_be_bytes()
        .iter()
        .chain(bytes.iter())
    {
        client.write_all(&[*byte]).unwrap();
    }
    let response = read_frame(&mut client).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response).unwrap()["kind"],
        "snapshot"
    );
    drop(client);
    let mut over = UnixStream::connect(server.socket()).unwrap();
    over.write_all(&65537u32.to_be_bytes()).unwrap();
    over.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(over.read(&mut [0; 1]).unwrap(), 0);
    drop(over);
    let mut stalled = UnixStream::connect(server.socket()).unwrap();
    stalled.write_all(&[0]).unwrap();
    stalled
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    assert_eq!(stalled.read(&mut [0; 1]).unwrap(), 0);
    drop(stalled);
    let mut reconnect = UnixStream::connect(server.socket()).unwrap();
    write_frame(&mut reconnect, &bytes).unwrap();
    assert!(!read_frame(&mut reconnect).unwrap().is_empty());
    drop(reconnect);
    server.shutdown().unwrap();
    assert!(!server.socket().exists());
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn stalled_reply_reader_has_absolute_timeout() {
    let (mut writer, _reader) = UnixStream::pair().unwrap();
    let start = std::time::Instant::now();
    let bytes = vec![b'x'; 65536];
    let mut refused = false;
    for _ in 0..32 {
        if write_frame(&mut writer, &bytes).is_err() {
            refused = true;
            break;
        }
    }
    assert!(refused);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn healthy_renew_interval_keeps_connection_open() {
    let path = std::env::temp_dir().join(format!("lux-idle-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let engine = Authority::new(
        Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![],
        })
        .unwrap(),
        "11111111-1111-4111-8111-111111111111",
        9,
    )
    .unwrap();
    let mut server = LocalServer::bind(&path, Service::new(engine).unwrap()).unwrap();
    let mut stream = UnixStream::connect(server.socket()).unwrap();
    write_frame(&mut stream, &request()).unwrap();
    read_frame(&mut stream).unwrap();
    std::thread::sleep(Duration::from_millis(650));
    write_frame(&mut stream, &request()).unwrap();
    read_frame(&mut stream).unwrap();
    drop(stream);
    server.shutdown().unwrap();
    std::fs::remove_dir(path).unwrap();
}
