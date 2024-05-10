#[cfg(feature = "lv2")]
pub use lv2hm::Lv2Host as lv2h;
#[cfg(feature = "lv2")]
pub use lv2hm::AddPluginError;

#[cfg(feature = "lv2")]
pub type Lv2Host = lv2h;
#[cfg(not(feature = "lv2"))]
pub type Lv2Host = ();

