use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};

// List of read-only and ignored environment variables
const FISH_READONLY: &[&str] = &[
    "PWD",
    "SHLVL",
    "history",
    "pipestatus",
    "status",
    "version",
    "FISH_VERSION",
    "fish_pid",
    "hostname",
    "_",
    "fish_private_mode",
];

const IGNORED: &[&str] = &["PS1", "XPC_SERVICE_NAME"];

fn ignored(name: &str) -> bool {
    if name == "PWD" {
        return false; // PWD has special handling
    }
    FISH_READONLY.contains(&name)
        || IGNORED.contains(&name)
        || name.starts_with("BASH_FUNC")
        || name.starts_with('%')
}

// Escapes strings safely for Fish shell
fn escape(value: &str) -> String {
    let escaped = value
        .replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("$", "\\$");
    format!("\"{}\"", escaped)
}

// Extracts aliases properly from Bash output
fn parse_aliases(alias_output: &str) -> Vec<String> {
    alias_output
        .lines()
        .filter(|line| line.starts_with("alias ")) // Ensure it's a valid alias
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                let name = parts[0].trim_start_matches("alias ").trim();
                let value = parts[1].trim_matches('\''); // Remove surrounding single quotes
                Some(format!("alias {} {}", name, escape(value)))
            } else {
                None
            }
        })
        .collect()
}

// Runs a command and returns the environment variables
fn get_env() -> io::Result<HashMap<String, String>> {
    let env_output = Command::new("bash")
        .arg("-c")
        .arg("env")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    let env_str = String::from_utf8_lossy(&env_output.stdout);
    let mut env_map = HashMap::new();

    for line in env_str.lines() {
        if let Some((key, value)) = line.split_once('=') {
            env_map.insert(key.to_string(), value.to_string());
        }
    }

    Ok(env_map)
}

fn process_env_changes(
    new_env: &HashMap<String, String>,
    old_env: &HashMap<String, String>,
) -> Vec<String> {
    let mut script_lines = Vec::new();

    // Find added or modified environment variables
    for (k, v) in new_env.iter() {
        if ignored(k) {
            continue;
        }
        match old_env.get(k) {
            None => script_lines.push(format!("# Adding {}", k)),
            Some(old_value) if old_value != v => {
                script_lines.push(format!("# Updating {}: '{}' -> '{}'", k, old_value, v))
            }
            _ => continue,
        }
        script_lines.push(if k == "PWD" {
            format!("cd {}", escape(v))
        } else {
            format!("set -g -x {} {}", k, escape(v))
        });
    }

    // Find removed environment variables
    for k in old_env.keys() {
        if !new_env.contains_key(k) {
            script_lines.push(format!("# Removing {}", k));
            script_lines.push(format!("set -e {}", k));
        }
    }

    script_lines
}

fn eval_and_get_new_env(command: &str) -> io::Result<(HashMap<String, String>, String)> {
    const ALIAS_SEPARATOR: &str = "---ALIAS---";
    let bash_script = format!(
        "eval \"{}\" >/dev/null; env; echo '\n{}\n'; alias",
        command, ALIAS_SEPARATOR
    );
    let output = Command::new("bash")
        .arg("-c")
        .arg(&bash_script)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut lines = output_str.lines();

    let mut new_env = HashMap::new();
    while let Some(line) = lines.next() {
        if line == ALIAS_SEPARATOR {
            break;
        }
        if let Some((key, value)) = line.split_once('=') {
            new_env.insert(key.to_string(), value.to_string());
        }
    }

    let alias_str: String = lines.collect::<Vec<&str>>().join("\n");
    Ok((new_env, alias_str))
}

fn gen_script() -> io::Result<String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.join(" ");

    let old_env = get_env()?;
    let (new_env, alias_str) = eval_and_get_new_env(&command)?;

    let script_lines = process_env_changes(&new_env, &old_env);
    let alias_lines = parse_aliases(&alias_str);

    Ok(format!(
        "{}\n{}",
        script_lines.join("\n"),
        alias_lines.join("\n")
    ))
}

fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());

    if env::args().len() == 1 {
        writeln!(writer, "Usage: basrs <bash-command>")?;
        return Ok(());
    }

    match gen_script() {
        Ok(script) => writer.write_all(script.as_bytes())?,
        Err(e) => {
            eprintln!("Basrs internal error: {}", e);
            return Err(e);
        }
    }
    Ok(())
}
