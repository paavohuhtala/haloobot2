mod hs;

mod dude_carpet;
pub use dude_carpet::handle_dude_carpet;

mod get_excuse;
pub use get_excuse::handle_get_excuse;

mod fingerpori;
pub use fingerpori::handle_fingerpori;
pub use fingerpori::handle_randompori;

mod fokit;
pub use fokit::handle_fokit;
pub use fokit::handle_random_fokit;

mod lasaga;
pub use lasaga::handle_lasaga;
pub use lasaga::handle_random_lasaga;

mod subscription;
pub use subscription::handle_subscribe;

mod autoreply;
pub use autoreply::handle_add_message;
pub use autoreply::handle_add_message_reply;

mod config;
pub use config::handle_set_autoreply_chance;
