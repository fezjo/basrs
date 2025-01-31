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

fn parse_env(env_str: &str) -> HashMap<String, String> {
    let mut env_map = HashMap::new();
    for line in env_str.lines() {
        if let Some((key, value)) = line.split_once('=') {
            env_map.insert(key.to_string(), value.to_string());
        }
    }
    env_map
}

fn process_env_changes(old_env_str: &str, new_env_str: &str) -> Vec<String> {
    let old_env = parse_env(old_env_str);
    let new_env = parse_env(new_env_str);
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

fn parse_funcs(func_str: &str) -> Vec<String> {
    // "declare -f func_name" -> "func_name"
    func_str
        .lines()
        .map(|line| line.split_whitespace().last().unwrap().to_string())
        .collect()
}

fn process_func_changes(old_func_str: &str, new_func_str: &str) -> Vec<String> {
    let old_funcs = parse_funcs(old_func_str);
    let new_funcs = parse_funcs(new_func_str);
    let mut script_lines = Vec::new();

    // Find added functions
    for func in new_funcs.iter() {
        if !old_funcs.contains(func) {
            script_lines.push(format!("# Adding function {}", func));
            // TODO
        }
    }

    // Find removed functions
    for func in old_funcs.iter() {
        if !new_funcs.contains(func) {
            script_lines.push(format!("# Removing function {}", func));
            // TODO
        }
    }

    // TODO track changed definitions

    script_lines
}

fn eval_and_get_new_env(command: &str) -> io::Result<(String, String, String)> {
    // Returns raw sections: env, aliases, and functions
    const SECTION_SEPARATOR: &str = "---SECTION---";
    let bash_script = format!(
        "eval \"{}\" >/dev/null; env; echo '{}'; alias; echo '{}'; declare -F",
        command, SECTION_SEPARATOR, SECTION_SEPARATOR
    );
    let output = Command::new("bash")
        .arg("-c")
        .arg(&bash_script)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Command execution failed",
        ));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let sections: Vec<String> = output_str
        .split(SECTION_SEPARATOR)
        .map(|s| s.trim().to_string())
        .collect();

    Ok((
        sections[0].clone(),
        sections[1].clone(),
        sections[2].clone(),
    ))
}

fn gen_script() -> io::Result<String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.join(" ");

    let (old_env_str, _, old_func_str) = eval_and_get_new_env("")?;
    let (new_env_str, new_alias_str, new_func_str) = eval_and_get_new_env(&command)?;

    let env_lines = process_env_changes(&old_env_str, &new_env_str);
    let alias_lines = parse_aliases(&new_alias_str);
    let func_lines = process_func_changes(&old_func_str, &new_func_str);

    Ok(format!(
        "{}\n{}\n{}\n",
        env_lines.join("\n"),
        alias_lines.join("\n"),
        func_lines.join("\n")
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
