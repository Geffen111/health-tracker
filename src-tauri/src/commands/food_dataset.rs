//! Tagging food items from a built-in list (food_dataset.tsv), for when the AI tagger is
//! switched off. Instant and local; anything it can't place stays untagged for the person
//! (or a later AI run) to fill in. Rows it tags get `tag_source = 'dataset'`, which — like
//! 'ai' — is only ever written over a NULL, so hand edits are never touched.
//!
//! Matching is by whole words, plurals folded ("Sausages" finds "sausage"):
//! - the longest phrase wins ("peanut butter" over "peanut" and "butter");
//! - the category comes from the main part of the name — the bit before " - ", " with ",
//!   "(" or "," — and within that, the last food mentioned, since that's usually the head
//!   noun ("Banana & Honey Protein Milk" is a milk drink; "Jam - Strawberry" is jam);
//! - flags are the union over every food mentioned, plus modifier words ("pickled",
//!   "wholemeal"), minus negations ("gluten free", "decaf", "no sugar").

use std::collections::HashSet;
use std::sync::OnceLock;

const DATA: &str = include_str!("food_dataset.tsv");

const CATEGORIES: &[(&str, &str)] = &[
    ("fr", "Fruit"), ("veg", "Vegetables"), ("gr", "Grains & bread"), ("da", "Dairy"),
    ("mf", "Meat & fish"), ("eg", "Eggs"), ("le", "Legumes"), ("nu", "Nuts & seeds"),
    ("me", "Meals & dishes"), ("sn", "Snacks & sweets"), ("sa", "Sauces & spreads"),
    ("hd", "Hot drinks"), ("cd", "Cold drinks"), ("al", "Alcohol"),
];

const FLAGS: &[(char, &str)] = &[
    ('G', "Gluten"), ('D', "Dairy"), ('H', "High-histamine"), ('C', "Caffeine"), ('A', "Alcohol"),
    ('S', "Added sugar"), ('F', "High-FODMAP"), ('P', "Protein"), ('B', "High-fiber"), ('K', "High-carb"),
];

struct Entry {
    /// None for a modifier.
    category: Option<&'static str>,
    add: Vec<&'static str>,
    remove: Vec<&'static str>,
    phrases: Vec<Vec<String>>,
}

/// Lower-case words with simple plural folding, applied to the list and the item alike.
fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('&', " and ")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(stem)
        .collect()
}

fn stem(w: &str) -> String {
    let n = w.len();
    if n > 4 && w.ends_with("ies") {
        return format!("{}y", &w[..n - 3]);
    }
    if n > 4 && w.ends_with("es") {
        let base = &w[..n - 2];
        if base.ends_with("sh") || base.ends_with("ch") || base.ends_with('x') || base.ends_with('o') || base.ends_with("ss") {
            return base.to_string();
        }
    }
    if n > 3 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") {
        return w[..n - 1].to_string();
    }
    w.to_string()
}

fn entries() -> &'static [Entry] {
    static LIST: OnceLock<Vec<Entry>> = OnceLock::new();
    LIST.get_or_init(|| {
        DATA.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|line| {
                let mut parts = line.splitn(3, '|');
                let (cat, flags, names) = (parts.next()?.trim(), parts.next()?.trim(), parts.next()?.trim());
                let category = if cat == "-" { None } else { Some(CATEGORIES.iter().find(|(c, _)| *c == cat)?.1) };
                let mut add = Vec::new();
                let mut remove = Vec::new();
                for f in flags.split_whitespace() {
                    let (neg, code) = match f.strip_prefix('-') { Some(c) => (true, c), None => (false, f) };
                    if let Some((_, name)) = code.chars().next().and_then(|c| FLAGS.iter().find(|(k, _)| *k == c)) {
                        if neg { remove.push(*name) } else { add.push(*name) }
                    }
                }
                let phrases = names.split(';').map(words).filter(|p| !p.is_empty()).collect();
                Some(Entry { category, add, remove, phrases })
            })
            .collect()
    })
}

#[derive(Debug, PartialEq)]
pub struct Tags {
    pub category: Option<&'static str>,
    pub flags: Vec<&'static str>,
}

/// Where the main part of a name ends: " - ", " with ", "(", ",". Returns how many of the
/// name's words come before it.
fn head_len(name: &str) -> usize {
    let lower = name.to_lowercase();
    let cut = [" - ", " – ", " with ", " w/ ", "(", ","]
        .iter()
        .filter_map(|sep| lower.find(sep))
        .min()
        .unwrap_or(lower.len());
    words(&lower[..cut]).len()
}

/// Category and flags for a food or drink name, or None if nothing in the list matches.
pub fn tag(name: &str) -> Option<Tags> {
    let ws = words(name);
    if ws.is_empty() {
        return None;
    }
    // Every phrase occurrence: (entry, start, length).
    let mut hits: Vec<(&Entry, usize, usize)> = Vec::new();
    for e in entries() {
        for p in &e.phrases {
            if p.len() > ws.len() {
                continue;
            }
            for start in 0..=ws.len() - p.len() {
                if ws[start..start + p.len()] == p[..] {
                    hits.push((e, start, p.len()));
                }
            }
        }
    }
    // A food inside a longer food phrase doesn't count on its own ("butter" in "peanut
    // butter"), nor does one inside a negation ("sugar" in "no sugar").
    let foods: Vec<(&Entry, usize, usize)> = hits
        .iter()
        .copied()
        .filter(|(e, s, l)| {
            e.category.is_some()
                && !hits.iter().any(|(o, os, ol)| {
                    (o.category.is_some() || !o.remove.is_empty()) && *ol > *l && *os <= *s && s + l <= os + ol
                })
        })
        .collect();
    if foods.is_empty() {
        return None;
    }
    let head = head_len(name).max(1);
    let in_head: Vec<_> = foods.iter().filter(|(_, s, _)| *s < head).collect();
    let main = match in_head.iter().max_by_key(|(_, s, l)| (*s, *l)) {
        Some(h) => **h,
        None => *foods.iter().min_by_key(|(_, s, l)| (*s, std::cmp::Reverse(*l))).unwrap(),
    };

    let mut flags: Vec<&'static str> = Vec::new();
    let mut removed: HashSet<&'static str> = HashSet::new();
    for (e, _, _) in foods.iter().chain(hits.iter().filter(|(e, _, _)| e.category.is_none())) {
        for f in &e.add {
            if !flags.contains(f) {
                flags.push(f);
            }
        }
        removed.extend(e.remove.iter().copied());
    }
    flags.retain(|f| !removed.contains(f));
    // Keep the list's own flag order, for tidy display and stable tests.
    flags.sort_by_key(|f| FLAGS.iter().position(|(_, n)| n == f));
    Some(Tags { category: main.0.category, flags })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(name: &str) -> (Option<&'static str>, Vec<&'static str>) {
        let r = tag(name).unwrap_or(Tags { category: None, flags: vec![] });
        (r.category, r.flags)
    }

    #[test]
    fn every_line_parses() {
        let bad: Vec<&str> = DATA
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter(|l| {
                let p: Vec<&str> = l.splitn(3, '|').collect();
                p.len() != 3
                    || (p[0].trim() != "-" && !CATEGORIES.iter().any(|(c, _)| *c == p[0].trim()))
                    || p[1].split_whitespace().any(|f| {
                        let c = f.trim_start_matches('-').chars().next().unwrap();
                        f.trim_start_matches('-').len() != 1 || !FLAGS.iter().any(|(k, _)| *k == c)
                    })
            })
            .collect();
        assert!(bad.is_empty(), "bad lines: {:?}", bad);
        assert!(entries().len() > 150);
    }

    #[test]
    fn plurals_and_longest_phrase() {
        assert_eq!(t("Sausages"), (Some("Meat & fish"), vec!["High-histamine", "Protein"]));
        assert_eq!(t("Peanut butter").0, Some("Nuts & seeds"));
        assert_eq!(t("Sweet potatoes").0, Some("Vegetables"));
        assert_eq!(t("Chicken nuggets"), (Some("Meat & fish"), vec!["Gluten", "Protein", "High-carb"]));
        assert_eq!(t("Hot chips - frozen").0, Some("Meals & dishes"));
        assert_eq!(t("Cous cous").0, Some("Grains & bread"));
        assert_eq!(t("Coffee - Iced - bottle").0, Some("Cold drinks"));
        assert_eq!(t("Tuna Mornay - ALDI - Frozen").0, Some("Meals & dishes"));
        assert_eq!(t("Biscuit - choc chip"), (Some("Snacks & sweets"), vec!["Gluten", "Dairy", "High-histamine", "Added sugar", "High-carb"]));
    }

    #[test]
    fn category_comes_from_the_main_part_of_the_name() {
        assert_eq!(t("Coffee with milk - French press"), (Some("Hot drinks"), vec!["Dairy", "Caffeine", "High-FODMAP", "Protein"]));
        assert_eq!(t("Jam - Strawberry"), (Some("Sauces & spreads"), vec!["High-histamine", "Added sugar"]));
        assert_eq!(t("Banana & Honey Protein Milk").0, Some("Cold drinks"));
        assert_eq!(t("Milk - Full Cream").0, Some("Dairy"));
        assert_eq!(t("Tea - English Breakfast"), (Some("Hot drinks"), vec!["High-histamine", "Caffeine"]));
    }

    #[test]
    fn modifiers_add_and_negations_remove() {
        assert!(t("Cabbage - Pickled").1.contains(&"High-histamine"));
        assert_eq!(t("Cordial - Fruit - No sugar"), (Some("Cold drinks"), vec![]));
        assert!(!t("Gluten free bread").1.contains(&"Gluten"));
        assert!(!t("Decaf latte").1.contains(&"Caffeine"));
        assert!(t("Wholemeal pasta").1.contains(&"High-fiber"));
    }

    #[test]
    fn unknown_names_stay_untagged() {
        assert_eq!(tag("Grandma's special"), None);
        assert_eq!(tag(""), None);
    }
}
