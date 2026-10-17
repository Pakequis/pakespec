//! Translatable interface text.
//!
//! Source strings are English and double as catalog keys. Each language is a
//! TOML file in `locales/` mapping the English string to its translation;
//! `locales/en.toml` lists every string of the interface. A missing
//! translation falls back to the English text. To add a language: add a file
//! in `locales/` and one line in `LANGUAGES`.
//!
//! Always translate whole messages with `{name}` placeholders. Never build
//! sentences from fragments, because word order differs between languages.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Pt,
}

pub struct LanguageInfo {
    pub id: Language,
    /// BCP 47 code, used to match the system language.
    pub code: &'static str,
    /// Name of the language in that language.
    pub native_name: &'static str,
    catalog: &'static str,
}

pub const LANGUAGES: [LanguageInfo; 2] = [
    LanguageInfo {
        id: Language::En,
        code: "en",
        native_name: "English",
        catalog: include_str!("../locales/en.toml"),
    },
    LanguageInfo {
        id: Language::Pt,
        code: "pt",
        native_name: "Português",
        catalog: include_str!("../locales/pt.toml"),
    },
];

impl Language {
    /// Picks the language from a system locale such as `pt_BR.UTF-8`; English when unknown.
    pub fn from_system_locale(locale: &str) -> Language {
        let primary = locale
            .split(['_', '-', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        LANGUAGES
            .iter()
            .find(|l| l.code == primary)
            .map_or(Language::En, |l| l.id)
    }

    /// Reads the locale from the usual environment variables.
    pub fn from_environment() -> Language {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(var)
                && !v.is_empty()
                && v != "C"
                && v != "POSIX"
            {
                return Self::from_system_locale(&v);
            }
        }
        Language::En
    }
}

fn parse_catalog(text: &str) -> HashMap<String, String> {
    toml::from_str(text).unwrap_or_default()
}

/// Translator for one language.
#[derive(Debug, Clone)]
pub struct Translator {
    catalog: HashMap<String, String>,
}

impl Translator {
    pub fn new(language: Language) -> Self {
        let info = LANGUAGES
            .iter()
            .find(|l| l.id == language)
            .expect("every Language has an entry in LANGUAGES");
        Self {
            catalog: parse_catalog(info.catalog),
        }
    }

    /// Translates `text`, falling back to the English source.
    pub fn tr<'a>(&'a self, text: &'a str) -> &'a str {
        self.catalog.get(text).map_or(text, String::as_str)
    }

    /// Translates `text` and fills `{name}` placeholders.
    pub fn tr_args(&self, text: &str, args: &[(&str, &str)]) -> String {
        let mut out = self.tr(text).to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(text: &str) -> BTreeSet<String> {
        parse_catalog(text).into_keys().collect()
    }

    #[test]
    fn every_catalog_has_the_same_strings_as_english() {
        let english = keys(LANGUAGES[0].catalog);
        assert!(!english.is_empty());
        for l in &LANGUAGES[1..] {
            let k = keys(l.catalog);
            assert_eq!(
                english.difference(&k).collect::<Vec<_>>(),
                Vec::<&String>::new(),
                "{} lacks strings",
                l.code
            );
            assert_eq!(
                k.difference(&english).collect::<Vec<_>>(),
                Vec::<&String>::new(),
                "{} has strings unknown to en.toml",
                l.code
            );
        }
    }

    #[test]
    fn placeholders_match_between_languages() {
        let placeholders = |s: &str| -> BTreeSet<String> {
            s.split('{')
                .skip(1)
                .filter_map(|p| p.split('}').next())
                .map(String::from)
                .collect()
        };
        let english = parse_catalog(LANGUAGES[0].catalog);
        for l in &LANGUAGES[1..] {
            for (k, v) in parse_catalog(l.catalog) {
                assert_eq!(placeholders(&k), placeholders(&v), "{}: {k}", l.code);
            }
        }
        assert!(!english.is_empty());
    }

    #[test]
    fn translates_and_falls_back() {
        let pt = Translator::new(Language::Pt);
        assert_eq!(pt.tr("Start frequency"), "Frequência inicial");
        assert_eq!(
            pt.tr("A string nobody translated"),
            "A string nobody translated"
        );
        assert_eq!(
            Translator::new(Language::En).tr("Start frequency"),
            "Start frequency"
        );
    }

    #[test]
    fn fills_placeholders() {
        let pt = Translator::new(Language::Pt);
        assert_eq!(
            pt.tr_args("Sweep failed: {reason}", &[("reason", "timeout")]),
            "A varredura falhou: timeout"
        );
    }

    #[test]
    fn system_locale_selection() {
        assert_eq!(Language::from_system_locale("pt_BR.UTF-8"), Language::Pt);
        assert_eq!(Language::from_system_locale("en_US.UTF-8"), Language::En);
        assert_eq!(Language::from_system_locale("de_DE"), Language::En);
        assert_eq!(Language::from_system_locale(""), Language::En);
    }
}
