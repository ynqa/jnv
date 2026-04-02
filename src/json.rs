use jaq_core::{
    data,
    load::{Arena, File, Loader},
    unwrap_valr, Compiler, Ctx, Vars,
};
use jaq_json::{read, Val};

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
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let loader = Loader::new(defs);
    let modules = loader
        .load(
            &arena,
            File {
                code: query,
                path: (),
            },
        )
        .map_err(|errs| anyhow::anyhow!("jq filter parsing failed: {errs:?}"))?;
    let funs = jaq_core::funs::<data::JustLut<Val>>()
        .chain(jaq_std::funs::<data::JustLut<Val>>())
        .chain(jaq_json::funs::<data::JustLut<Val>>());
    let filter = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errs| anyhow::anyhow!("jq filter compilation failed: {errs:?}"))?;

    let mut ret = Vec::<serde_json::Value>::new();

    for input in json_stream {
        let input = serde_json::to_vec(input)
            .map_err(|e| anyhow::anyhow!("failed to serialize input JSON: {e}"))?;
        let input = read::parse_single(&input)
            .map_err(|e| anyhow::anyhow!("failed to parse input into jaq value: {e}"))?;

        let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
        let out = filter.id.run((ctx, input)).map(unwrap_valr);

        for item in out {
            let val = item.map_err(|err| anyhow::anyhow!("jq filter execution failed: {err}"))?;
            let text = val.to_string();
            let json = serde_json::from_str(&text).map_err(|e| {
                anyhow::anyhow!(
                    "failed to convert jaq output to JSON value (possibly non-JSON output): {e}"
                )
            })?;
            ret.push(json);
        }
    }

    Ok(ret)
}
