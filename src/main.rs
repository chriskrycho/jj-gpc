use std::process;

use clap::Parser as _;
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

    let log_template = match args.log_format {
        Some(LogFormat::Full) => LOG_FULL,
        Some(LogFormat::OneLine) | None => LOG_ONE_LINE,
    };

    let commits = execute(process::Command::new("jj").args(&[
        "log",
        "-T",
        log_template,
        "-r",
        &revset,
        "--no-graph",
    ]));

    if commits.stdout.trim().is_empty() {
        eprintln!("No commits to summarize");
        std::process::exit(1);
    }

    let ollama = Ollama::default();
    let prompt = format!(
        "{PROMPT_START}\n\n```\n{commits}\n```\n\n{PROMPT_END}",
        commits = commits.stdout
    );
    log::debug!("prompt: {prompt}");

    let request = GenerationRequest::new(args.model, prompt.clone()).options(
        GenerationOptions::default()
            .top_k(args.top_k)
            .top_p(args.top_p)
            .temperature(args.temperature)
            .num_predict(10),
    );

    let generation_response = ollama
        .generate(request)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let response = generation_response.response.trim().trim_end_matches("-");

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
    if !branch_output.stdout.trim().is_empty() {
        println!("{}", branch_output.stdout);
    }
    if !branch_output.stderr.trim().is_empty() {
        println!("{}", branch_output.stderr);
    }

    println!("jj git push --bookmark {branch_name} --allow-new");
    let push_output = execute(process::Command::new("jj").args(&[
        "git",
        "push",
        "--bookmark",
        &branch_name,
        "--allow-new",
    ]));
    push_output.to_console();
}

fn execute(command: &mut process::Command) -> Output {
    log::trace!("{command:?}");
    let process::Output {
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

    Output {
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
    }
}

struct Output {
    stdout: String,
    stderr: String,
}

impl Output {
    fn to_console(&self) {
        let Self { stdout, stderr } = self;
        if !stdout.trim().is_empty() {
            print!("{}", stdout);
        }

        if !stderr.trim().is_empty() {
            eprint!("{}", stderr);
        }
    }
}

/// Generate a branch name for use with jj.
#[derive(clap::Parser, Debug)]
#[command(version, author)]
struct Cli {
    #[arg(default_value = "@")]
    change: String,

    #[arg(long, value_enum)]
    log_format: Option<LogFormat>,

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
    #[arg(long, default_value = "2")]
    temperature: f32,

    /// Reduces the probability of generating nonsense. A higher value (e.g. 100)
    /// will give more diverse answers, while a lower.
    #[arg(long, default_value = "20")]
    top_k: u32,

    /// Works together with top-k. A higher value (e.g., 0.95) will lead to more
    /// diverse text, while a lower value (e.g., 0.5) will generate more focused
    /// and conservative text.
    #[arg(long, default_value = "0.7")]
    top_p: f32,

    /// Which model to use. Can be any model available in Ollama on your system.
    ///
    /// The model you choose to use will significantly alters the quality of the
    /// output, so you may need to tune the parameters as well. If this is not a
    /// model available in Ollama on your system, the request will fail.
    #[arg(long, default_value = "phi3")]
    model: String,
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum LogFormat {
    OneLine,
    Full,
}

const LOG_ONE_LINE: &'static str = r#"if(description, description.first_line(), '') ++ "\n""#;
const LOG_FULL: &'static str = "builtin_log_compact_full_description";

const PROMPT_START: &'static str = "Here is the Git commit log for this branch:";
const PROMPT_END: &'static str = r#"Reply with the shortest descriptive branch name (*not* a pull request description, just a branch name) for a Git branch containing these commits, using no more than 7 lowercase words separated by hyphens.

- A good branch name is always derived from the summary of changes in the log.
- Never return branch names which:
    - have too little information, like `ab-1234`
    - are too long, for example `this-branch-name-is-fifteen-words-long-instead-of-maxxing-out-at-seven-as-instructed`
    - simply include a date
- Do not include any explanation, only the branch name.
"#;
