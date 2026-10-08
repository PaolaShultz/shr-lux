//! Explicit headless synthetic null-output local authority.
use shr_lux::{
    authority::Authority,
    fixture::{FixtureSpec, Id, Patch, PatchSpec, SyntheticMode},
    local_service::LocalServer,
    wire::Service,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 || args[1] != "--synthetic-private-dir" {
        return Err("usage: lux-service --synthetic-private-dir ABSOLUTE_OWNED_0700_DIRECTORY [--timed|--durable] [--analysis ABSOLUTE_PRIVATE_PROVIDER_SOCKET] [--config ABSOLUTE_JSON]".into());
    }
    let mut timed = false;
    let mut durable = false;
    let mut analysis_socket = None;
    let mut config_path = None;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--timed" if !timed && !durable => timed = true,
            "--durable" if !timed && !durable => {
                timed = true;
                durable = true;
            }
            "--config" if config_path.is_none() && i + 1 < args.len() => {
                i += 1;
                config_path = Some(PathBuf::from(&args[i]));
            }
            "--analysis" if analysis_socket.is_none() && i + 1 < args.len() => {
                i += 1;
                analysis_socket = Some(PathBuf::from(&args[i]));
            }
            _ => return Err("invalid lux-service option".into()),
        }
        i += 1;
    }
    let config = config_path
        .as_deref()
        .map(shr_lux::service_config::ServiceConfig::read)
        .transpose()?;
    if config.as_ref().is_some_and(|c| c.analysis.is_some()) && analysis_socket.is_none() {
        return Err("configured analysis requires explicit --analysis socket".into());
    }
    // Validate all startup inputs before persistence or local socket writes.
    if let Some(socket) = &analysis_socket {
        if !socket.is_absolute() {
            return Err("absolute analysis socket required".into());
        }
        shr_lux::local_service::private_directory(socket.parent().ok_or("socket parent")?)?;
    }
    let patch = if let Some(config) = &config {
        config.validate()?
    } else {
        Patch::validate(PatchSpec {
            version: 1,
            patch_revision: 1,
            fixtures: vec![FixtureSpec::synthetic(
                Id::new("fixture-11").map_err(|_| "id")?,
                "Synthetic",
                SyntheticMode::RgbPosition,
                1,
            )],
        })
        .map_err(|_| "patch")?
    };
    let show = config
        .as_ref()
        .map_or("11111111-1111-4111-8111-111111111111", |c| {
            c.show_id.as_str()
        });
    let engine = Authority::new(patch, show, 1).map_err(|_| "authority")?;
    // Every listener reserves and retains a unique epoch, including volatile modes.
    let directory = PathBuf::from(&args[2]);
    let store = Arc::new(if let Some(config) = &config {
        shr_lux::recovery::CheckpointStore::open_configured(&directory, config)?
    } else {
        shr_lux::recovery::CheckpointStore::open(&directory, show)?
    });
    let mut service = if durable {
        Service::recover(store, engine.patch().clone())
    } else {
        Service::reserved(store, engine.patch().clone(), timed)
    }
    .map_err(std::io::Error::other)?;
    let analysis = if let Some(socket) = analysis_socket {
        if let Some(binding) = config.as_ref().and_then(|c| c.analysis.as_ref()) {
            service
                .enable_configured_analysis(binding.clone())
                .map_err(std::io::Error::other)?;
            Some(shr_lux::analysis_subscription::Client::start_configured(
                &socket,
                binding.clone(),
            )?)
        } else {
            service.enable_analysis().map_err(std::io::Error::other)?;
            Some(shr_lux::analysis_subscription::Client::start(&socket)?)
        }
    } else {
        None
    };
    let mut server = LocalServer::bind_with_analysis(&PathBuf::from(&args[2]), service, analysis)?;
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, stop.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, stop.clone())?;
    println!(
        "synthetic null_disarmed physical unknown: {}",
        server.socket().display()
    );
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(20));
    }
    server.shutdown()?;
    Ok(())
}
