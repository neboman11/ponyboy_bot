use std::fs::File;
use std::io::prelude::*;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) keyword_actions: Option<Vec<KeywordAction>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KeywordAction {
    pub(crate) keywords: Option<Vec<String>>,
    pub(crate) actions: Option<Vec<String>>,
    pub(crate) emotes: Option<Vec<String>>,
    pub(crate) name: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) file: Option<String>,
}

pub(crate) fn load_keyword_actions() -> Vec<KeywordAction> {
    let mut input = String::new();
    File::open("keyword_actions.toml")
        .and_then(|mut f| f.read_to_string(&mut input))
        .unwrap();
    let decoded: Config = toml::from_str(&input).unwrap();
    println!("{:#?}", decoded);
    return decoded.keyword_actions.unwrap();
}