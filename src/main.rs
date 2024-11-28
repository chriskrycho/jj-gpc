use std::process::{self, Output};

use clap::Parser as _;
use lazy_static::lazy_static;
use ollama_rs::{
    generation::{completion::request::GenerationRequest, options::GenerationOptions},
    Ollama,
};
use regex::Regex;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Cli::parse();
    let revset = format!("{}..{}", args.from, args.change);

    let commits = execute(process::Command::new("jj").args(&[
        "log",
        "-T",
        LOG_TEMPLATE,
        "--no-graph",
        "-r",
        &revset,
    ]));

    if commits.trim().is_empty() {
        eprintln!("No commits to summarize");
        std::process::exit(1);
    }

    let ollama = Ollama::default();
    let prompt = format!("{LLM_PROMPT}\n\n```\n{commits}\n```");

    let request = GenerationRequest::new(LLM_MODEL.into(), prompt)
        .options(GenerationOptions::default().top_k(10));

    let response = ollama
        .generate(request)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .response;

    let branch_name = SPACES.replace_all(response.trim(), "-");
    let branch_name = args.prefix.map_or(branch_name.to_string(), |prefix| {
        format!("{prefix}/{branch_name}")
    });

    if args.dry_run {
        println!("[dry run] jj bookmark create {branch_name}");
        println!("[dry run] jj git push --bookmark {branch_name}");
        return;
    }

    let branch_output =
        execute(process::Command::new("jj").args(&["bookmark", "create", &branch_name]));

    println!("{branch_output}");

    let push_output =
        execute(process::Command::new("jj").args(&["git", "push", "--bookmark", &branch_name]));

    println!("{push_output}");
}

fn execute(command: &mut process::Command) -> String {
    let Output {
        status,
        stdout,
        stderr,
    } = command
        .output()
        .unwrap_or_else(|e| panic!("Could not execute command {command:?}.\nCause: {e}"));

    if !status.success() {
        eprintln!(
            "Error running '{command:?}' ({status}):\nCause: {}",
            String::from_utf8_lossy(&stderr)
        );
        process::exit(status.code().unwrap_or(1));
    }

    String::from_utf8_lossy(&stdout).to_string()
}

/// Generate a branch name for use with jj.
#[derive(clap::Parser, Debug)]
struct Cli {
    #[arg(default_value = "@")]
    change: String,

    #[arg(short, long, default_value = "trunk()")]
    from: String,

    /// Prefix for the generated branch name, `<prefix>/<generated>`
    #[arg(short, long)]
    prefix: Option<String>,

    /// Generate the branch name, but do not actually create or push it.
    #[arg(long = "dry-run", default_value = "false")]
    dry_run: bool,
}

lazy_static! {
    static ref SPACES: Regex = Regex::new(r"\s+").unwrap();
}

const LOG_TEMPLATE: &'static str = r#"if(description, description.first_line(), '') ++ "\n""#;

const LLM_MODEL: &'static str = "llama3.2";
const LLM_PROMPT: &'static str = "Summarizing *all* of these messages in a single phrase. The phrase should consist of 2–4-words, all lowercase. Do not mention branches. Do not include more words. Reply with only the phrase.";
