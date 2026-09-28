//! Descriptor-only diagnostics. Never claim interfaces or send vendor requests.
use std::time::Duration;

pub fn report() -> Result<Vec<String>, rusb::Error> {
    let mut lines = vec!["USB diagnostics (read-only; no DMX output)".into()];
    let mut matches = 0;
    for device in rusb::devices()?.iter() {
        let descriptor = device.device_descriptor()?;
        if descriptor.vendor_id() != 0x16c0 || descriptor.product_id() != 0x05dc {
            continue;
        }
        matches += 1;
        lines.push(format!(
            "Candidate 16c0:05dc at bus {} address {}",
            device.bus_number(),
            device.address()
        ));
        match device.open() {
            Ok(handle) => {
                let timeout = Duration::from_secs(1);
                let strings = (|| {
                    let language = *handle
                        .read_languages(timeout)?
                        .first()
                        .ok_or(rusb::Error::Other)?;
                    let manufacturer =
                        handle.read_manufacturer_string(language, &descriptor, timeout)?;
                    let product = handle.read_product_string(language, &descriptor, timeout)?;
                    let serial =
                        handle.read_serial_number_string(language, &descriptor, timeout)?;
                    Ok::<_, rusb::Error>((manufacturer, product, serial))
                })();
                match strings {
                    Ok((manufacturer, product, serial)) => {
                        lines.push(format!("Identity: {manufacturer} / {product} / {serial}"));
                        if manufacturer == "www.anyma.ch" && product == "uDMX" {
                            lines.push(
                                "uDMX identity verified; USB descriptor reads succeeded.".into(),
                            );
                        } else {
                            lines.push(
                                "Shared USB ID: identity does not match expected uDMX.".into(),
                            );
                        }
                    }
                    Err(error) => {
                        lines.push(format!("Opened device; identity read failed: {error}"))
                    }
                }
            }
            Err(rusb::Error::Access) => {
                lines.push("Access denied: see docs/notes/0004-linux-usb-access.md".into())
            }
            Err(error) => lines.push(format!("Cannot open device: {error}")),
        }
    }
    if matches == 0 {
        lines.push("No 16c0:05dc candidate detected.".into());
    }
    lines.push("DMX electrical output and fixture response remain unverified.".into());
    Ok(lines)
}
