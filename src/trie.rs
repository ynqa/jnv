use radix_trie::{Trie, TrieCommon};

use promkit::serde_json;

#[derive(Default, Clone)]
pub struct QueryTrie(Trie<String, Vec<serde_json::Value>>);

impl QueryTrie {
    pub fn insert(&mut self, query: &str, json_nodes: Vec<serde_json::Value>) {
        self.0.insert(query.to_string(), json_nodes);
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

    mod prefix_search {
        use super::*;
        use serde_json::json;

        #[test]
        fn test() {
            let mut trie = QueryTrie::default();
            trie.insert("apple", vec![json!({"type": "fruit"})]);
            trie.insert("app", vec![json!({"type": "abbreviation"})]);

            let result = trie.prefix_search("app");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "abbreviation"}));

            let result = trie.prefix_search("application");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "abbreviation"}));

            let result = trie.prefix_search("apple");
            assert!(result.is_some());
            let values = result.unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0], json!({"type": "fruit"}));

            let result = trie.prefix_search("ap");
            assert!(result.is_none());
        }
    }
}
