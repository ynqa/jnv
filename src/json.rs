use jaq_core::{
    load::{Arena, File, Loader},
    Compiler, Ctx, RcIter,
};
use jaq_json::Val;

use promkit_widgets::{
    jsonstream::jsonz,
    serde_json::{self, Deserializer, Value},
};

/// Get all JSON paths from the input JSON string,
/// respecting the max_streams limit if provided.
pub async fn get_all_paths(
    json_str: &str,
    max_streams: Option<usize>,
) -> anyhow::Result<impl Iterator<Item = String>> {
    let stream = deserialize(json_str, max_streams)?;
    let paths = jsonz::get_all_paths(stream.iter()).collect::<Vec<_>>();
    Ok(paths.into_iter())
}

/// Deserialize JSON string into a vector of serde_json::Value.
/// If max_streams is given, only deserialize up to that many JSON values.
pub fn deserialize(
    json_str: &str,
    max_streams: Option<usize>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let deserializer: serde_json::StreamDeserializer<'_, serde_json::de::StrRead<'_>, Value> =
        Deserializer::from_str(json_str).into_iter::<serde_json::Value>();
    let results = match max_streams {
        Some(l) => deserializer.take(l).collect::<Result<Vec<_>, _>>(),
        None => deserializer.collect::<Result<Vec<_>, _>>(),
    };
    results.map_err(anyhow::Error::from)
}

pub fn run_jaq(
    query: &str,
    json_stream: &[serde_json::Value],
) -> anyhow::Result<Vec<serde_json::Value>> {
    let arena = Arena::default();
    let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
    let modules = loader
        .load(
            &arena,
            File {
                code: query,
                path: (),
            },
        )
        .map_err(|errs| anyhow::anyhow!("jq filter parsing failed: {errs:?}"))?;
    let filter = Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .map_err(|errs| anyhow::anyhow!("jq filter compilation failed: {errs:?}"))?;

    let mut ret = Vec::<serde_json::Value>::new();

    for input in json_stream {
        let inputs = RcIter::new(core::iter::empty());
        let out = filter.run((Ctx::new([], &inputs), Val::from(input.clone())));
        for item in out {
            match item {
                Ok(val) => ret.push(val.into()),
                Err(err) => return Err(anyhow::anyhow!("jq filter execution failed: {err}")),
            }
        }
    }

    Ok(ret)
}

/// Summarize a jq result stream as a short `type · length` string, mirroring
/// what `| type` and `| length` would report. Used for a status-line hint so
/// users don't have to append those filters manually.
///
/// A single value is described by its type and (where meaningful) its length;
/// a multi-value stream is reported as a count.
pub fn summarize(values: &[serde_json::Value]) -> String {
    match values {
        [] => "empty (0 results)".to_string(),
        [value] => summarize_value(value),
        many => format!("stream · {} values", many.len()),
    }
}

fn summarize_value(value: &serde_json::Value) -> String {
    match value {
        Value::Object(map) => format!("object · {} {}", map.len(), pluralize(map.len(), "key")),
        Value::Array(items) => {
            format!("array · {} {}", items.len(), pluralize(items.len(), "item"))
        }
        Value::String(s) => {
            let count = s.chars().count();
            format!("string · {} {}", count, pluralize(count, "char"))
        }
        Value::Number(_) => "number".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Null => "null".to_string(),
    }
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use promkit_widgets::serde_json::json;

    #[test]
    fn summarize_object_array_string() {
        assert_eq!(summarize(&[json!({"a": 1, "b": 2})]), "object · 2 keys");
        assert_eq!(summarize(&[json!({"a": 1})]), "object · 1 key");
        assert_eq!(summarize(&[json!([1, 2, 3])]), "array · 3 items");
        assert_eq!(summarize(&[json!([1])]), "array · 1 item");
        assert_eq!(summarize(&[json!("hello")]), "string · 5 chars");
    }

    #[test]
    fn summarize_scalars() {
        assert_eq!(summarize(&[json!(42)]), "number");
        assert_eq!(summarize(&[json!(true)]), "boolean");
        assert_eq!(summarize(&[json!(null)]), "null");
    }

    #[test]
    fn summarize_stream_and_empty() {
        assert_eq!(summarize(&[json!(1), json!(2)]), "stream · 2 values");
        assert_eq!(summarize(&[]), "empty (0 results)");
    }

    #[test]
    fn summarize_string_counts_unicode_scalar_values() {
        // "café" is 4 chars even though 'é' is multi-byte.
        assert_eq!(summarize(&[json!("café")]), "string · 4 chars");
    }
}
