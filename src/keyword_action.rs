use std::env;
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) keyword_actions: Option<Vec<KeywordAction>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KeywordAction {
    pub(crate) keywords: Option<Vec<String>>,
    pub(crate) name: Option<String>,
    pub(crate) triggers: Option<Vec<String>>,
    pub(crate) mentioned_user: Option<u64>,
    pub(crate) actions: Option<Vec<Action>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Action {
    pub(crate) emotes: Option<Vec<String>>,
    pub(crate) file: Option<String>,
    pub(crate) mention: Option<String>,
    pub(crate) message: Option<String>,
}

pub(crate) fn load_keyword_actions() -> Vec<KeywordAction> {
    let file_base_dir =
        env::var("FILE_BASE_DIR").expect("Expected file base dir to be set in the environment");
    let path = Path::new(&file_base_dir).join("keyword_actions.toml");

    let input = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("Warning: {} not found, no keyword actions loaded", path.display());
            return Vec::new();
        }
        Err(e) => panic!("Failed to read {}: {}", path.display(), e),
    };

    let decoded: Config =
        toml::from_str(&input).expect("Failed to parse keyword_actions.toml");
    decoded.keyword_actions.unwrap_or_default()
}
