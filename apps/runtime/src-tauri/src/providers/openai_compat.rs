use super::traits::ProviderPlugin;

pub struct OpenAiCompatProvider {
    key: String,
    name: String,
    capabilities: Vec<&'static str>,
}

impl OpenAiCompatProvider {
    pub fn new(key: &str, name: &str, capabilities: Vec<&'static str>) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            capabilities,
        }
    }
}

impl ProviderPlugin for OpenAiCompatProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .map(|cap| (*cap).to_string())
            .collect()
    }
}
