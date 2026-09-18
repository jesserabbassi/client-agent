/// Station data independent of the Slint view model.
pub(crate) struct Station {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) gpu: String,
    pub(crate) cpu: String,
    pub(crate) memory: String,
    pub(crate) monitor: String,
    pub(crate) storage: String,
    pub(crate) rate: String,
    pub(crate) available: bool,
    pub(crate) rate_cents: i32,
}
