//! The agent skill must stay true to the CLI: every `bibu ...` example has to parse.

use clap::Parser;

fn skill() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/bibu/SKILL.md");
    std::fs::read_to_string(path)
        .expect("skills/bibu/SKILL.md exists")
        .replace("\r\n", "\n")
}

/// Commands from fenced code blocks: a line starting with `bibu `, or following a `| `.
fn examples(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_block = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if !in_block {
            continue;
        }
        for segment in line.split(" | ") {
            let segment = segment.trim();
            if segment.starts_with("bibu ") {
                found.push(segment.to_string());
            }
        }
    }
    found
}

#[test]
fn frontmatter_names_the_skill_and_says_when_to_use_it() {
    let text = skill();
    let front = text
        .strip_prefix("---\n")
        .and_then(|t| t.split_once("\n---\n"))
        .expect("frontmatter")
        .0;
    assert!(front.lines().any(|l| l == "name: bibu"), "{front}");
    let description = front
        .lines()
        .find_map(|l| l.strip_prefix("description: "))
        .expect("description");
    assert!(
        description.len() > 80,
        "description should say when to use the skill"
    );
    assert!(description.contains("Bitbucket"));
}

#[test]
fn every_example_command_parses_with_the_real_cli() {
    let commands = examples(&skill());
    assert!(
        commands.len() >= 25,
        "expected the skill to show many commands, found {}",
        commands.len()
    );
    let mut failures = Vec::new();
    for command in &commands {
        let words =
            shell_words::split(command).unwrap_or_else(|e| panic!("cannot split {command:?}: {e}"));
        if let Err(error) = bibu::cli::Cli::try_parse_from(words) {
            failures.push(format!(
                "{command}\n    {}",
                error.to_string().lines().next().unwrap_or("")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "skill examples that do not parse:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_skill_covers_every_command_group() {
    let text = skill();
    for group in [
        "pr list",
        "pr comment add",
        "pr reviewers add",
        "pr task",
        "pipeline logs",
        "branch create",
        "member find",
        "schema",
    ] {
        assert!(
            text.contains(&format!("bibu {group}")),
            "skill never mentions `bibu {group}`"
        );
    }
}

#[test]
fn the_skill_states_the_safety_rules() {
    let text = skill();
    for rule in [
        "Always pass `--json`",
        "Destructive commands need the user's intent",
        "Never ask the user to paste a token",
        "Decide on the exit code",
    ] {
        assert!(text.contains(rule), "missing rule: {rule}");
    }
}
