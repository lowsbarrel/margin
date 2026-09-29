use super::*;

fn tree_from(names: &[&str]) -> VaultTree {
    let mut tree = VaultTree {
        root: "/vault".into(),
        entries: Vec::new(),
        trie: vec![TrieNode::default()],
    };
    for name in names {
        let name_lc = name.to_lowercase();
        let id = tree.entries.len() as u32;
        tree.insert(&name_lc, id);
        for token in interior_tokens(&name_lc) {
            tree.insert(token, id);
        }
        tree.entries.push(Entry {
            name: (*name).into(),
            name_lc,
            rel_lc: name.to_lowercase(),
            path: format!("/vault/{name}"),
            is_dir: false,
            modified: 0,
        });
    }
    tree
}

#[test]
fn interior_tokens_skips_the_leading_run() {
    assert_eq!(interior_tokens("my-project.md"), vec!["project", "md"]);
    assert_eq!(interior_tokens("readme"), Vec::<&str>::new());
}

#[test]
fn trie_finds_whole_name_and_interior_tokens() {
    let tree = tree_from(&["my-project.md", "readme.md"]);
    assert_eq!(tree.candidates("my"), Some(&[0u32][..]));
    assert_eq!(tree.candidates("project"), Some(&[0u32][..]));
    assert_eq!(tree.candidates("md"), Some(&[0u32, 1][..]));
    assert_eq!(tree.candidates("zzz"), None);
}

#[test]
fn shared_token_prefixes_are_not_duplicated() {
    let tree = tree_from(&["read-readme.md"]);
    assert_eq!(tree.candidates("read"), Some(&[0u32][..]));
}

#[test]
fn ranks_prefer_literal_prefixes_over_fuzzy() {
    let tree = tree_from(&["project.md"]);
    let e = &tree.entries[0];
    assert_eq!(rank_of(e, "pro", true), RANK_NAME_PREFIX);
    assert_eq!(rank_of(e, "ject", false), RANK_NAME_SUBSTRING);
    assert_eq!(rank_of(e, "pjt", false), RANK_FUZZY);
}
