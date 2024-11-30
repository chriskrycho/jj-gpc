use std::process::{self, Output};

use clap::{Parser as _, ValueEnum as _};
use ollama_rs::{
    generation::{completion::request::GenerationRequest, options::GenerationOptions},
    Ollama,
};
use regex::Regex;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();
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

    let model = args
        .model
        .to_possible_value()
        .expect("there should always be a model value")
        .get_name()
        .to_owned();

    let request = GenerationRequest::new(model, prompt).options(
    log::debug!("prompt: {prompt}");
        GenerationOptions::default()
            .top_k(args.top_k)
            .top_p(args.top_p)
            .temperature(args.temperature),
    );

    let response = ollama
        .generate(request)
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .response;

    let branch_name = Regex::new(r"\s+")
        .unwrap()
        .replace_all(response.trim(), "-");

    let branch_name = args.prefix.map_or(branch_name.to_string(), |prefix| {
        format!("{prefix}/{branch_name}")
    });

    if args.dry_run {
        println!(
            "[dry run] jj bookmark create {branch_name} --revision {}",
            args.change
        );
        println!("[dry run] jj git push --bookmark {branch_name}");
        return;
    }

    println!(
        "jj bookmark create {branch_name} --revision {}",
        args.change
    );
    let branch_output = execute(process::Command::new("jj").args(&[
        "bookmark",
        "create",
        &branch_name,
        "--revision",
        &args.change,
    ]));
    if !branch_output.trim().is_empty() {
        println!("{branch_output}");
    }

    println!("jj git push --bookmark {branch_name}");
    let push_output =
        execute(process::Command::new("jj").args(&["git", "push", "--bookmark", &branch_name]));

    if !push_output.trim().is_empty() {
        println!("{push_output}");
    }
}

fn execute(command: &mut process::Command) -> String {
    log::trace!("{command:?}");
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

    /// The temperature of the model. Increasing the temperature will make the
    /// model answer more creatively.
    #[arg(long, default_value = "0.8")]
    temperature: f32,

    /// Reduces the probability of generating nonsense. A higher value (e.g. 100)
    /// will give more diverse answers, while a lower.
    #[arg(long, default_value = "40")]
    top_k: u32,

    /// Works together with top-k. A higher value (e.g., 0.95) will lead to more
    /// diverse text, while a lower value (e.g., 0.5) will generate more focused
    /// and conservative text.
    #[arg(long, default_value = "0.9")]
    top_p: f32,

    #[arg(long, value_enum, default_value = "llama3.2")]
    model: Model,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Model {
    #[clap(name = "llama3.2")]
    Llama3_2,
    #[clap(name = "llama3.2:1b")]
    Llama3_2_1b,
}

const LOG_TEMPLATE: &'static str = r#"if(description, description) ++ "\n\n---\n\n""#;

const LLM_PROMPT: &'static str = "Summarize all of these messages in a single phrase. The phrase should consist of 2–4-words, all lowercase. Do not mention branches. Do not include more words. Reply with only the phrase.";
