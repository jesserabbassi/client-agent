use crate::models::station::Station;

/// Local sample catalog. No persistence or network requests.
pub(crate) fn list() -> Vec<Station> {
    (1..=8)
        .map(|number| {
            let status = match number {
                4 => "Offline",
                6 => "Occupied",
                8 => "Maintenance",
                _ => "Available",
            };
            Station {
                name: format!("PC-{number:02}").into(),
                status: status.into(),
                available: status == "Available",
                gpu: if number == 3 { "RTX 4090" } else { "RTX 4070" }.into(),
                cpu: if number == 3 {
                    "Intel Core i9"
                } else {
                    "Intel Core i7"
                }
                .into(),
                memory: "32 GB".into(),
                monitor: "240 Hz".into(),
                storage: "1 TB SSD".into(),
                rate: "3.00 € / hour".into(),
                rate_cents: 300,
            }
        })
        .collect()
}
