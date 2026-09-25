use std::io::Write;
use std::process::{Command, Stdio};

const COPIERS: [&[&str]; 3] = [
    &["pbcopy"],
    &["wl-copy"],
    &["xclip", "-selection", "clipboard"],
];
const OPENERS: [&str; 2] = ["open", "xdg-open"];

pub fn copy(text: &str) -> bool {
    COPIERS.iter().any(|argv| {
        let Ok(mut child) = Command::new(argv[0])
            .args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let written = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        written && child.wait().is_ok_and(|status| status.success())
    })
}

pub fn open(url: &str) -> bool {
    OPENERS.iter().any(|opener| {
        Command::new(opener)
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}
