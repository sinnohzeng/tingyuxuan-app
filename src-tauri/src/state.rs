use tingyuxuan_core::config::AppConfig;
use tingyuxuan_core::history::HistoryManager;

pub struct AppState {
    pub config: AppConfig,
    pub history: HistoryManager,
}

impl AppState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = AppConfig::load().unwrap_or_default();
        let history = HistoryManager::new()?;

        Ok(Self { config, history })
    }
}
