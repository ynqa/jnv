use radix_trie::{Trie, TrieCommon};

use promkit::serde_json;

#[derive(Default)]
pub struct QueryTrie(Trie<String, Vec<serde_json::Value>>);

impl QueryTrie {
    pub fn insert(&mut self, query: &str, json_nodes: Vec<serde_json::Value>) {
        self.0.insert(query.to_string(), json_nodes);
    }

    pub fn prefix_search(&self, query: &str) -> Option<(&String, &Vec<serde_json::Value>)> {
        self.0
            .get_ancestor(query)
            .and_then(|subtrie| Some((subtrie.key()?, subtrie.value()?)))
    }

    pub fn prefix_search_value(&self, query: &str) -> Option<&Vec<serde_json::Value>> {
        self.prefix_search(query).map(|tup| tup.1)
    }
}
