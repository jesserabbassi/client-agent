mod agent_controller;
mod booking_controller;
mod login_controller;
mod stations_controller;

pub(crate) use agent_controller::bind;

#[cfg(test)]
mod tests;
