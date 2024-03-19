use radix_trie::{Trie, TrieCommon};

use promkit::json::JsonNode;

#[derive(Default)]
pub struct QueryTrie(Trie<String, Vec<JsonNode>>);

impl QueryTrie {
    pub fn insert(&mut self, query: &str, json_nodes: Vec<JsonNode>) {
        self.0.insert(query.to_string(), json_nodes);
    }

    pub fn prefix_search(&self, query: &str) -> Option<(&String, &Vec<JsonNode>)> {
        self.0
            .get_ancestor(query)
            .and_then(|subtrie| Some((subtrie.key()?, subtrie.value()?)))
    }

    pub fn prefix_search_value(&self, query: &str) -> Option<&Vec<JsonNode>> {
        self.prefix_search(query).map(|tup| tup.1)
    }
}
