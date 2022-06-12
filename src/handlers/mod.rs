mod autoreply;
mod config;
mod dude_carpet;
mod fingerpori;
mod get_excuse;
mod lasaga;
mod subscription;

pub use dude_carpet::handle_dude_carpet;
pub use get_excuse::handle_get_excuse;

pub use fingerpori::handle_fingerpori;
pub use fingerpori::handle_randompori;

pub use lasaga::handle_lasaga;
pub use lasaga::handle_random_lasaga;

pub use subscription::handle_subscribe;

pub use autoreply::handle_add_message;
pub use autoreply::handle_add_message_reply;

pub use config::handle_set_autoreply_chance;
