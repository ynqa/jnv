use radix_trie::{Trie, TrieCommon};

use promkit::serde_json;

#[derive(Default, Clone)]
pub struct FilterTrie(Trie<String, Vec<serde_json::Value>>);

impl FilterTrie {
    pub fn insert(&mut self, query: &str, json_nodes: Vec<serde_json::Value>) {
        self.0.insert(query.to_string(), json_nodes);
    }

    pub fn exact_search(&self, query: &str) -> Option<&Vec<serde_json::Value>> {
        self.0.get(query)
    }

    pub fn prefix_search(&self, query: &str) -> Option<&Vec<serde_json::Value>> {
        self.0
            .get_ancestor(query)
            .and_then(|subtrie| subtrie.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod exact_search {
        use super::*;
        use serde_json::json;

        #[test]
        fn test_exact_match() {
            let mut trie = FilterTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.exact_search("app");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "abbreviation"}));
        }

        #[test]
        fn test_no_match() {
            let mut trie = FilterTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.exact_search("application");
            assert!(result.is_none());
        }
    }

    mod prefix_search {
        use super::*;
        use serde_json::json;

        #[test]
        fn test_with_exact_prefix() {
            let mut trie = FilterTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.prefix_search("app");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "abbreviation"}));
        }

        #[test]
        fn test_with_longer_query_than_keys() {
            let mut trie = FilterTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.prefix_search("application");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "abbreviation"}));
        }

        #[test]
        fn test_with_full_key_match() {
            let mut trie = FilterTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.prefix_search("apple");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "fruit"}));
        }

        #[test]
        fn test_with_shorter_query_than_any_key() {
            let mut trie = FilterTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.prefix_search("ap");
            assert!(result.is_none());
        }
    }
}
