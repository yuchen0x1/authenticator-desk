mod model;
pub(crate) mod service;
mod templates;
pub use model::*;
pub use service::AccountService;
pub use templates::{PlatformTemplate, platform_templates};
