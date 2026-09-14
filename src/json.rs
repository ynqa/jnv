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

/// Collect a stream of JSON values into a single JSON array, mirroring `jq --slurp`.
///
/// This lets a filter run against JSON Lines (or any whitespace-separated stream)
/// as one array rather than value by value. `max_streams` bounds how many values
/// are read, the same as [`deserialize`].
pub fn slurp(json_str: &str, max_streams: Option<usize>) -> anyhow::Result<String> {
    let values = deserialize(json_str, max_streams)?;
    serde_json::to_string(&Value::Array(values)).map_err(anyhow::Error::from)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slurp_wraps_whitespace_separated_values_into_one_array() {
        assert_eq!(slurp("1 2 3", None).unwrap(), "[1,2,3]");
    }

    #[test]
    fn slurp_wraps_json_lines_into_one_array() {
        let out = slurp("{\"a\":1}\n{\"a\":2}\n", None).unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        let expected: Value = serde_json::from_str("[{\"a\":1},{\"a\":2}]").unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn slurp_wraps_a_single_value_into_a_one_element_array() {
        let out = slurp("{\"a\":1}", None).unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        let expected: Value = serde_json::from_str("[{\"a\":1}]").unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn slurp_of_empty_input_is_an_empty_array() {
        assert_eq!(slurp("", None).unwrap(), "[]");
        assert_eq!(slurp("  \n  ", None).unwrap(), "[]");
    }

    #[test]
    fn slurp_honors_max_streams() {
        assert_eq!(slurp("1 2 3 4", Some(2)).unwrap(), "[1,2]");
    }

    #[test]
    fn slurp_reports_invalid_json() {
        assert!(slurp("{ not json", None).is_err());
    }
}
