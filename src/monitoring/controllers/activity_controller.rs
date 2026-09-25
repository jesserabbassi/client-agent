use crate::ui::{Activity, ClientView, Page, Reservation, Transaction};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct State {
    cents: i32,
    reservations: Vec<Reservation>,
    transactions: Vec<Transaction>,
    active: Option<i32>,
    selected_game: String,
}
fn money(cents: i32) -> String {
    format!("{}.{:02} €", cents / 100, cents % 100)
}
fn amount(text: &str) -> Option<i32> {
    let text = text.trim();
    let mut parts = text.split('.');
    let whole = parts.next()?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let cents =
        whole
            .parse::<i32>()
            .ok()?
            .checked_mul(100)?
            .checked_add(if fraction.is_empty() {
                0
            } else {
                fraction.parse::<i32>().ok()? * if fraction.len() == 1 { 10 } else { 1 }
            })?;
    (100..=50_000).contains(&cents).then_some(cents)
}
impl State {
    fn publish(&self, ui: &ClientView, message: &str) {
        let view = ui.global::<Activity>();
        view.set_selected_game(self.selected_game.clone().into());
        view.set_balance(money(self.cents).into());
        view.set_reservations(ModelRc::new(VecModel::from(self.reservations.clone())));
        view.set_transactions(ModelRc::new(VecModel::from(self.transactions.clone())));
        view.set_session_active(self.active.is_some());
        view.set_session_label(
            self.active
                .and_then(|id| self.reservations.iter().find(|r| r.id == id))
                .map(|r| format!("{} · {}", r.station, r.duration))
                .unwrap_or_else(|| "No active session".into())
                .into(),
        );
        view.set_message(message.into());
    }
    fn transaction(&mut self, description: String, cents: i32) {
        self.transactions.insert(
            0,
            Transaction {
                reference: format!("TX-{:04}", self.transactions.len() + 1).into(),
                description: description.into(),
                amount: format!(
                    "{}{}",
                    if cents >= 0 { "+" } else { "−" },
                    money(cents.abs())
                )
                .into(),
            },
        );
    }
}
pub(crate) fn bind(ui: &ClientView) {
    let state = Rc::new(RefCell::new(State::default()));
    let weak = ui.as_weak();
    let data = state.clone();
    ui.global::<Activity>().on_top_up(move |text| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() {
            return;
        }
        let mut state = data.borrow_mut();
        let Some(cents) = amount(&text) else {
            state.publish(
                &ui,
                "Enter an amount from 1.00 to 500.00 with up to two decimal places.",
            );
            return;
        };
        let Some(balance) = state.cents.checked_add(cents) else {
            state.publish(&ui, "Balance limit reached.");
            return;
        };
        state.cents = balance;
        state.transaction("Demo wallet top-up".into(), cents);
        state.publish(
            &ui,
            "Demo funds added successfully. No real payment was taken.",
        );
    });
    let weak = ui.as_weak();
    let data = state.clone();
    ui.global::<Activity>().on_reserve(move |station, hours| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() {
            return;
        }
        let mut state = data.borrow_mut();
        if !(0..4).contains(&station) || !(1..=3).contains(&hours) {
            return;
        }
        let station = format!("Station {:02}", station + 1);
        if state
            .reservations
            .iter()
            .any(|r| r.station == station && r.status != "Completed")
        {
            state.publish(&ui, "You already have a reservation for this station.");
            return;
        }
        let id = state.reservations.len() as i32 + 1;
        state.reservations.push(Reservation {
            id,
            station: station.into(),
            duration: format!("{hours}h").into(),
            price: money(hours * 600).into(),
            status: "Reserved".into(),
        });
        state.publish(
            &ui,
            "Station reserved. Start when ready; your wallet will be charged then.",
        );
    });
    let weak = ui.as_weak();
    let data = state.clone();
    ui.global::<Activity>().on_start_session(move |hours| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() || !(1..=3).contains(&hours) {
            return;
        }
        let state = data.borrow();
        if state.active.is_some() {
            state.publish(&ui, "End your current session before starting another.");
            return;
        }
        if state.cents < hours * 600 {
            state.publish(
                &ui,
                "Insufficient funds. Add money in Wallet, then start your session.",
            );
            return;
        }
        let station = (0..4).find(|station| {
            let name = format!("Station {:02}", station + 1);
            !state
                .reservations
                .iter()
                .any(|r| r.station == name && r.status != "Completed")
        });
        let Some(station) = station else {
            state.publish(
                &ui,
                "All demo stations are reserved. Start one of your existing reservations below.",
            );
            return;
        };
        let id = state.reservations.len() as i32 + 1;
        drop(state);
        ui.global::<Activity>().invoke_reserve(station, hours);
        ui.global::<Activity>().invoke_start(id);
    });
    let weak = ui.as_weak();
    let data = state.clone();
    ui.global::<Activity>().on_start(move |id| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() {
            return;
        }
        let mut state = data.borrow_mut();
        if state.active.is_some() {
            state.publish(&ui, "End your current session before starting another.");
            return;
        }
        let Some(index) = state
            .reservations
            .iter()
            .position(|r| r.id == id && r.status == "Reserved")
        else {
            return;
        };
        let row = state.reservations[index].clone();
        let cost = row.duration.trim_end_matches('h').parse::<i32>().unwrap() * 600;
        if state.cents < cost {
            state.publish(
                &ui,
                "Insufficient funds. Add money in Wallet, then start your session.",
            );
            return;
        }
        state.cents -= cost;
        state.active = Some(id);
        state.reservations[index].status = "Playing".into();
        state.transaction(format!("{} · {} session", row.station, row.duration), -cost);
        state.selected_game.clear();
        state.publish(&ui, "Session started. Choose the game you want to play.");
        ui.set_page(Page::Games);
    });
    let weak = ui.as_weak();
    let data = state.clone();
    ui.global::<Activity>().on_choose_game(move |game| {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() {
            return;
        }
        let mut state = data.borrow_mut();
        if state.active.is_none() {
            state.publish(
                &ui,
                "Start a session from your dashboard before choosing a game.",
            );
            return;
        }
        if ![
            "Counter-Strike 2",
            "Valorant",
            "League of Legends",
            "Dota 2",
            "EA FC 24",
            "Rocket League",
        ]
        .contains(&game.as_str())
        {
            return;
        }
        state.selected_game = game.to_string();
        state.publish(
            &ui,
            "Game selected for this session. Demo only: game launching is not connected.",
        );
    });
    let weak = ui.as_weak();
    ui.global::<Activity>().on_end(move || {
        let Some(ui) = weak.upgrade() else { return };
        if !ui.get_authenticated() {
            return;
        }
        let mut state = state.borrow_mut();
        let Some(id) = state.active.take() else {
            return;
        };
        if let Some(row) = state.reservations.iter_mut().find(|r| r.id == id) {
            row.status = "Completed".into();
        }
        state.selected_game.clear();
        state.publish(&ui, "Session ended. Thanks for playing!");
        ui.set_page(Page::Dashboard);
    });
}
#[cfg(test)]
mod tests {
    use super::amount;
    #[test]
    fn validates_top_up_without_rounding_or_overflow() {
        for (input, expected) in [
            ("20", Some(2000)),
            ("1.05", Some(105)),
            ("500.00", Some(50000)),
            ("2.5", Some(250)),
            ("0", None),
            ("500.01", None),
            ("NaN", None),
            ("-20", None),
            ("1.001", None),
            ("2147483647", None),
        ] {
            assert_eq!(amount(input), expected, "{input}");
        }
    }
}
