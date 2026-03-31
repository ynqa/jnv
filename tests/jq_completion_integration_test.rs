#[path = "../src/json.rs"]
mod json;
#[path = "../src/jq_completion.rs"]
mod jq_completion;

struct Case<'a> {
    name: &'a str,
    input_json: &'static str,
    request: jq_completion::CompletionRequest<'a>,
    expected_candidates: Vec<jq_completion::CompletionCandidate>,
}

async fn initialize_and_wait(input: &'static str) -> jq_completion::CompletionEngine {
    let (engine, loader_task) = jq_completion::spawn_initialize(input, None, 8);
    loader_task
        .await
        .expect("completion path loader task should finish");
    engine
}

#[tokio::test]
async fn jq_completion_cases_table() {
    let cases: Vec<Case> = vec![
       
    ];

    for case in cases {

    }
}
