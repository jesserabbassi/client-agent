use crate::ui::{ClientView, StationItem};
use slint::{ComponentHandle, ModelRc, VecModel};

/// Small, local preview catalog; no network or background work.
pub(crate) fn bind(ui: &ClientView) {
    let stations: Vec<StationItem> = crate::repositories::station_repository::list()
        .into_iter()
        .map(|station| StationItem {
            name: station.name.into(),
            status: station.status.into(),
            gpu: station.gpu.into(),
            cpu: station.cpu.into(),
            memory: station.memory.into(),
            monitor: station.monitor.into(),
            storage: station.storage.into(),
            rate: station.rate.into(),
            available: station.available,
            rate_cents: station.rate_cents,
        })
        .collect();
    ui.set_stations(ModelRc::new(VecModel::from(stations.clone())));
    let weak = ui.as_weak();
    ui.on_station_selected(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() || (!ui.get_show_stations() || ui.get_show_booking()) {
            return;
        }
        let Some(station) = usize::try_from(index).ok().and_then(|i| stations.get(i)) else {
            return;
        };
        if station.available {
            ui.set_selected_station(index);
            ui.set_station_detail(station.clone());
        }
    });
}
