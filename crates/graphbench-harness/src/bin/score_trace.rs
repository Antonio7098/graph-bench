use graphbench_core::load_evidence_spec;
use graphbench_harness::score_turn_ledger_deterministically;
use graphbench_harness::turn_ledger::TurnLedger;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), graphbench_core::AppError> {
    let mut args = env::args().skip(1);
    let Some(ledger_path) = args.next() else {
        return Err(usage_error());
    };
    let Some(evidence_path) = args.next() else {
        return Err(usage_error());
    };

    let ledger = TurnLedger::load(&ledger_path)?;
    let evidence = load_evidence_spec(&evidence_path)?;
    let scored = score_turn_ledger_deterministically(&ledger, &evidence)?;
    let output = serde_json::to_string_pretty(&scored).map_err(|source| {
        graphbench_core::AppError::with_source(
            graphbench_core::ErrorCode::PersistenceWriteFailed,
            "failed to serialize deterministic score breakdown",
            graphbench_core::ErrorContext {
                component: "score_trace",
                operation: "serialize",
            },
            source,
        )
    })?;
    println!("{output}");
    Ok(())
}

fn usage_error() -> graphbench_core::AppError {
    graphbench_core::AppError::new(
        graphbench_core::ErrorCode::ConfigurationInvalid,
        "usage: cargo run -p graphbench-harness --bin score_trace -- <turn-ledger.json> <evidence-spec.json>",
        graphbench_core::ErrorContext {
            component: "score_trace",
            operation: "parse_args",
        },
    )
}
