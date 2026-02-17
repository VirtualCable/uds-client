pub mod v4;
pub mod v5;

pub mod registry;

// Re-export TunnelMaterial from crypt crate
pub use crypt::TunnelMaterial;

pub use registry::is_any_tunnel_active;
