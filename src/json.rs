use jaq_core::{
    load::{Arena, File, Loader},
    Compiler, Ctx, RcIter,
};
use jaq_json::Val;

use promkit_widgets::{
    jsonstream::jsonz,
    serde_json::{self, Deserializer, Value},
};

/// An error from running a jq filter, split by phase so callers can treat an
/// incomplete or syntactically invalid expression — common while the user is
/// still typing — differently from a genuine runtime failure.
#[derive(Debug)]
pub enum JaqError {
    /// The filter could not be parsed or compiled. This is the expected state
    /// for a half-typed expression (e.g. a trailing `|`), so callers typically
    /// stay quiet rather than surfacing it as an error.
    Invalid(String),
    /// The filter parsed and compiled but failed during execution. This is a
    /// real error worth reporting, since the filter itself is well-formed.
    Execution(String),
}

impl std::fmt::Display for JaqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JaqError::Invalid(msg) | JaqError::Execution(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for JaqError {}

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
) -> Result<Vec<serde_json::Value>, JaqError> {
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
        // Parse failures are usually just an incomplete expression mid-typing.
        .map_err(|_| JaqError::Invalid("incomplete or invalid jq filter".to_string()))?;
    let filter = Compiler::default()
        .with_funs(jaq_std::funs().chain(jaq_json::funs()))
        .compile(modules)
        .map_err(|_| JaqError::Invalid("invalid jq filter".to_string()))?;

    let mut ret = Vec::<serde_json::Value>::new();

    for input in json_stream {
        let inputs = RcIter::new(core::iter::empty());
        let out = filter.run((Ctx::new([], &inputs), Val::from(input.clone())));
        for item in out {
            match item {
                Ok(val) => ret.push(val.into()),
                Err(err) => return Err(JaqError::Execution(err.to_string())),
            }
        }
    }

    Ok(ret)
}
