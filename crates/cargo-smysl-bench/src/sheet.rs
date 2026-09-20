//! What a person labels from, and the file they write into.
//!
//! One commit is one document: why it is here, its message, then its diff. Labelling costs the person's
//! attention, and the cost of it is switching windows — the message in one place, the diff in another,
//! the definitions in a third. So the sheet holds all of it, and the template beside it holds nothing
//! but the shape of an answer.
//!
//! **The sheet never mentions an extraction.** A label written after reading what the tool found
//! measures agreement with the tool, not the commit, and an instrument that agrees with itself measures
//! nothing.

use cargo_smysl_git::read_commit;

/// The template a person fills in, with the definitions they need in front of them.
pub fn template(repo: &str, sha: &str, message: &str, files: &[String]) -> String {
    let quoted = |text: &str| {
        text.trim_end()
            .lines()
            .map(|l| format!("#   {l}\n"))
            .collect::<String>()
    };
    format!(
        "# Label for {repo} {sha}. Read {sha}.diff beside this file; label from the message AND the diff.\n\
         #\n\
         # A decision is a choice this commit made: `act` for something done, `decline` for something\n\
         # deliberately not done. A commit with no choice in it has no decisions, and that is a label.\n\
         # A prerequisite is what had to be true for a decision to be right, which the change relies on\n\
         # and does not itself establish — not the motivation, not the argument, not the decision\n\
         # restated. An alternative is an option turned down.\n\
         #\n\
         # Label blind: do not read what the tool extracted for this commit until this file says done.\n\
         #\n\
         # Message:\n{message}#\n# Files:\n{files}\n\
         schema = 1\n\
         repo = \"{repo}\"\n\
         commit = \"{sha}\"\n\
         labeller = \"\"\n\
         status = \"todo\"   # set to \"done\" when finished\n\n\
         # Copy a block per item. Ids are unique; prerequisites and alternatives name a decision.\n\
         #\n\
         # [[decision]]\n# id = \"D1\"\n# text = \"\"\n# kind = \"act\"   # act | decline\n# evidence = \"\"\n#\n\
         # [[prerequisite]]\n# id = \"P1\"\n# decision = \"D1\"\n# text = \"\"\n\
         # kind = \"existing-behaviour\"   # existing-behaviour | invariant | tool-setting | prior-change | assumption\n\
         # normative = false\n# evidence = \"\"\n#\n\
         # [[alternative]]\n# id = \"A1\"\n# decision = \"D1\"\n# text = \"\"\n# evidence = \"\"\n",
        message = quoted(message),
        files = quoted(&files.join("\n")),
    )
}

/// The commit as one document, each file cut to `file_lines` with a note saying what was cut.
pub fn reading(
    repo: &str,
    sha: &str,
    message: &str,
    files: &[(String, String)],
    file_lines: usize,
) -> String {
    let mut body = String::new();
    for (path, text) in files {
        let lines: Vec<&str> = text.lines().collect();
        body.push_str(&format!("\n--- {path} ---\n"));
        if lines.len() > file_lines {
            body.push_str(&lines[..file_lines].join("\n"));
            body.push_str(&format!(
                "\n… {} more line(s) of this file not shown; read them with `git show {sha} -- {path}`\n",
                lines.len() - file_lines
            ));
        } else {
            body.push_str(text);
            body.push('\n');
        }
    }
    format!(
        "Reading sheet — {repo} {sha}\n\
         Label in {sha}.toml. Label blind: do not read what the tool extracted for this commit first.\n\
         \n=== message ===\n\n{message}\n=== diff ===\n{body}"
    )
}

/// Read one commit and build both files: the sheet to read, and the template to fill in.
///
/// Self-contained: git is read through `gix`, never by running the `git` binary.
pub fn of_commit(
    root: &std::path::Path,
    repo: &str,
    rev: &str,
    file_lines: usize,
) -> Result<(String, String, String), String> {
    let commit = read_commit(root, rev).map_err(|e| e.to_string())?;
    let sha = commit.sha.chars().take(12).collect::<String>();
    let files: Vec<(String, String)> = commit
        .files
        .iter()
        .map(|f| (f.path.clone(), f.text()))
        .collect();
    let paths: Vec<String> = files.iter().map(|(p, _)| p.clone()).collect();
    Ok((
        sha.clone(),
        template(repo, &sha, &commit.message, &paths),
        reading(repo, &sha, &commit.message, &files, file_lines),
    ))
}
