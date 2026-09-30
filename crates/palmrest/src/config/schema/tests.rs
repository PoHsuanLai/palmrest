use super::{Kind, Schema};
use crate::config::parse;
use std::collections::BTreeSet;

/// Every dotted path of the config's leaf settings; an action's `{ kind = ".." }` is one leaf.
fn leaves(prefix: &str, value: &toml::Value, into: &mut BTreeSet<String>) {
    match value {
        toml::Value::Table(table) if !table.contains_key("kind") => {
            for (key, inner) in table {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                leaves(&path, inner, into);
            }
        }
        _ => {
            into.insert(prefix.to_owned());
        }
    }
}

/// The text of a config holding just `path = value`.
fn config_with(path: &str, value: &toml::Value) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    let (last, tables) = parts.split_last().expect("a path has a part");
    let mut table = toml::Table::new();
    table.insert((*last).to_owned(), value.clone());
    for part in tables.iter().rev() {
        let mut outer = toml::Table::new();
        outer.insert((*part).to_owned(), toml::Value::Table(table));
        table = outer;
    }
    table.to_string()
}

/// `value` read back through the lenient reader: the warnings it earns.
fn warnings(path: &str, value: &toml::Value) -> Vec<String> {
    let (_, warn) = parse(&config_with(path, value)).expect("valid toml");
    warn.iter().map(ToString::to_string).collect()
}

#[test]
fn every_setting_has_one_row_and_every_row_a_setting() {
    let defaults = toml::Value::try_from(crate::config::Config::default()).unwrap();
    let mut want = BTreeSet::new();
    leaves("", &defaults, &mut want);
    let paths: Vec<String> = Schema::palmrest().key.into_iter().map(|k| k.path).collect();
    let got: BTreeSet<String> = paths.iter().cloned().collect();
    assert_eq!(got.len(), paths.len(), "a path has two rows");
    assert_eq!(got, want);
}

#[test]
fn every_default_is_inside_its_own_kind() {
    for key in Schema::palmrest().key {
        let ok = match (&key.kind, &key.default) {
            (Kind::Bounded { min, max, .. }, toml::Value::Integer(n)) => (min..=max).contains(&n),
            (Kind::Toggle { variants }, toml::Value::String(word)) => variants.contains(word),
            (Kind::Segmented { variants } | Kind::Menu { variants }, toml::Value::String(word)) => {
                variants.contains(word)
            }
            _ => false,
        };
        assert!(
            ok,
            "{}: {:?} is not in {:?}",
            key.path, key.default, key.kind
        );
    }
}

#[test]
fn the_reader_accepts_every_bound_and_every_word() {
    for key in Schema::palmrest().key {
        let values: Vec<toml::Value> = match &key.kind {
            Kind::Bounded { min, max, .. } => vec![(*min).into(), (*max).into()],
            Kind::Toggle { variants } => variants.iter().map(|w| w.as_str().into()).collect(),
            Kind::Segmented { variants } | Kind::Menu { variants } => {
                variants.iter().map(|w| w.as_str().into()).collect()
            }
        };
        for value in values {
            assert_eq!(
                warnings(&key.path, &value),
                Vec::<String>::new(),
                "{} = {value}",
                key.path
            );
        }
    }
}

#[test]
fn a_bound_just_outside_a_ranged_key_is_refused() {
    // The rows whose reader has a range: the schema's bounds are that range exactly.
    for path in [
        "scroll.speed",
        "momentum.b",
        "palm_rejection.rear_band_percent",
    ] {
        let key = Schema::palmrest()
            .key
            .into_iter()
            .find(|k| k.path == path)
            .unwrap();
        let Kind::Bounded { min, max, .. } = key.kind else {
            panic!("{path} is bounded")
        };
        assert_eq!(warnings(path, &(max + 1).into()).len(), 1, "{path} above");
        if min > 0 {
            assert_eq!(warnings(path, &(min - 1).into()).len(), 1, "{path} below");
        }
    }
}

#[test]
fn the_kind_follows_the_count_of_words() {
    let words =
        |n: usize| -> Vec<&'static str> { ["a", "b", "c", "d", "e", "f", "g"][..n].to_vec() };
    assert!(matches!(Kind::choice(&words(2)), Kind::Toggle { .. }));
    assert!(matches!(Kind::choice(&words(3)), Kind::Segmented { .. }));
    assert!(matches!(Kind::choice(&words(5)), Kind::Segmented { .. }));
    assert!(matches!(Kind::choice(&words(6)), Kind::Menu { .. }));
}

#[test]
fn the_file_is_toml_with_one_key_table_per_row() {
    let schema = Schema::palmrest();
    let text = schema.to_toml();
    let table: toml::Table = text.parse().unwrap();
    assert_eq!(table["app"].as_str(), Some("palmrest"));
    assert_eq!(table["file"].as_str(), Some("palmrest/gestures.toml"));
    assert_eq!(
        table["key"].as_array().map(Vec::len),
        Some(schema.key.len())
    );
    let first = &table["key"][0];
    assert_eq!(first["page"]["kind"].as_str(), Some("mouse_and_gestures"));
    assert_eq!(first["kind"]["kind"].as_str(), Some("toggle"));
}

#[test]
fn every_key_is_labelled_and_explained() {
    for key in Schema::palmrest().key {
        assert!(
            !key.label.is_empty() && !key.section.is_empty(),
            "{}",
            key.path
        );
        assert!(key.help.ends_with('.'), "{}: help is a sentence", key.path);
    }
}

#[test]
fn every_word_of_a_choice_has_a_human_label() {
    for key in Schema::palmrest().key {
        let words: Vec<&String> = match &key.kind {
            Kind::Toggle { variants } => variants.iter().collect(),
            Kind::Segmented { variants } | Kind::Menu { variants } => variants.iter().collect(),
            Kind::Bounded { .. } => {
                assert!(key.labels.is_empty(), "{}", key.path);
                continue;
            }
        };
        for word in words {
            let label = key
                .labels
                .get(word)
                .unwrap_or_else(|| panic!("{}: {word}", key.path));
            assert!(
                !label.contains('_'),
                "{}: {label} reads like code",
                key.path
            );
        }
    }
}
