use itertools::Itertools;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
// use std::str::pattern::Pattern;
use std::{
    fmt,
    path::{Path, PathBuf},
};

pub const IMPORT_STATEMENT: &str = "include";
pub const COMMENT_SYMBOL: char = '#';

#[derive(Debug)]
pub enum Error {
    ConfigNotFound,
    Io(std::io::Error),
    InvalidConfig(ParseError),
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    // u32 is the line number where an error occured
    UnknownSymbol(PathBuf, u32),
    InvalidModifier(PathBuf, u32),
    InvalidKeysym(PathBuf, u32),
}

impl From<std::io::Error> for Error {
    fn from(val: std::io::Error) -> Self {
        if val.kind() == std::io::ErrorKind::NotFound {
            Error::ConfigNotFound
        } else {
            Error::Io(val)
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self {
            Error::ConfigNotFound => "Config file not found.".fmt(f),

            Error::Io(io_err) => format!("I/O Error while parsing config file: {}", io_err).fmt(f),
            Error::InvalidConfig(parse_err) => match parse_err {
                ParseError::UnknownSymbol(path, line_nr) => format!(
                    "Error parsing config file {:?}. Unknown symbol at line {}.",
                    path, line_nr
                )
                .fmt(f),
                ParseError::InvalidKeysym(path, line_nr) => format!(
                    "Error parsing config file {:?}. Invalid keysym at line {}.",
                    path, line_nr
                )
                .fmt(f),
                ParseError::InvalidModifier(path, line_nr) => format!(
                    "Error parsing config file {:?}. Invalid modifier at line {}.",
                    path, line_nr
                )
                .fmt(f),
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub contents: String,
    pub imports: Vec<PathBuf>,
}

pub fn load_file_contents(path: &Path) -> Result<String, Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

impl Config {
    // Go through the file by line and check if it is an import statement.
    // If it is, load the path and add it to the imports vector.
    pub fn get_imports(contents: &str) -> Result<Vec<PathBuf>, Error> {
        let mut imports = Vec::new();
        for line in contents.lines() {
            if line.split(' ').next().unwrap() == IMPORT_STATEMENT {
                if let Some(import_path) = line.split(' ').nth(1) {
                    imports.push(Path::new(import_path).to_path_buf());
                }
            }
        }
        Ok(imports)
    }

    pub fn new(path: &Path) -> Result<Self, Error> {
        let contents = load_file_contents(path)?;
        let imports = Self::get_imports(&contents)?;
        Ok(Config { path: path.to_path_buf(), contents, imports })
    }

    // Go through the files in the imports vector and load them.
    pub fn load_to_configs(&self) -> Result<Vec<Self>, Error> {
        let mut configs = Vec::new();
        for import in &self.imports {
            configs.push(Self::new(import)?)
        }
        Ok(configs)
    }

    pub fn load_and_merge(mut configs: Vec<Self>) -> Result<Vec<Self>, Error> {
        let mut prev_count = 0;
        let mut current_count = configs.len();
        while prev_count != current_count {
            prev_count = configs.len();
            // Load all the imports and handle duplications
            for config in configs.clone() {
                for import in Self::load_to_configs(&config)? {
                    if !configs.contains(&import) {
                        configs.push(import);
                    }
                }
            }
            current_count = configs.len();
        }
        Ok(configs)
    }
}

// pub fn load(path: &Path) -> Result<Vec<Hotkey>, Error> {
//     let mut hotkeys = Vec::new();
//     let configs = vec![Config::new(path)?];
//     for config in Config::load_and_merge(configs)? {
//         for hotkey in parse_contents(path.to_path_buf(), config.contents)? {
//             if !hotkeys.contains(&hotkey) {
//                 hotkeys.push(hotkey);
//             }
//         }
//     }
//     Ok(hotkeys)
// }

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub keysym: evdev::Key,
    pub modifiers: Vec<Modifier>,
    pub send: bool,
    pub on_release: bool,
}

impl PartialEq for KeyBinding {
    fn eq(&self, other: &Self) -> bool {
        self.keysym == other.keysym
            && self.modifiers.iter().all(|modifier| other.modifiers.contains(modifier))
            && self.modifiers.len() == other.modifiers.len()
            && self.send == other.send
            && self.on_release == other.on_release
    }
}

pub trait Prefix {
    fn send(self) -> Self;
    fn on_release(self) -> Self;
}

pub trait Value {
    fn keysym(&self) -> evdev::Key;
    fn modifiers(&self) -> Vec<Modifier>;
    fn is_send(&self) -> bool;
    fn is_on_release(&self) -> bool;
}

impl KeyBinding {
    pub fn new(keysym: evdev::Key, modifiers: Vec<Modifier>) -> Self {
        KeyBinding { keysym, modifiers, send: false, on_release: false }
    }
}

impl Prefix for KeyBinding {
    fn send(mut self) -> Self {
        self.send = true;
        self
    }
    fn on_release(mut self) -> Self {
        self.on_release = true;
        self
    }
}

impl Value for KeyBinding {
    fn keysym(&self) -> evdev::Key {
        self.keysym
    }
    fn modifiers(&self) -> Vec<Modifier> {
        self.clone().modifiers
    }
    fn is_send(&self) -> bool {
        self.send
    }
    fn is_on_release(&self) -> bool {
        self.on_release
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hotkey {
    pub keybinding: KeyBinding,
    pub command: String,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum Modifier {
    Super,
    Alt,
    Control,
    Shift,
}

impl Hotkey {
    pub fn from_keybinding(keybinding: KeyBinding, command: String) -> Self {
        Hotkey { keybinding, command }
    }
    #[cfg(test)]
    pub fn new(keysym: evdev::Key, modifiers: Vec<Modifier>, command: String) -> Self {
        Hotkey { keybinding: KeyBinding::new(keysym, modifiers), command }
    }
}

impl Prefix for Hotkey {
    fn send(mut self) -> Self {
        self.keybinding.send = true;
        self
    }
    fn on_release(mut self) -> Self {
        self.keybinding.on_release = true;
        self
    }
}

impl Value for &Hotkey {
    fn keysym(&self) -> evdev::Key {
        self.keybinding.keysym
    }
    fn modifiers(&self) -> Vec<Modifier> {
        self.keybinding.clone().modifiers
    }
    fn is_send(&self) -> bool {
        self.keybinding.send
    }
    fn is_on_release(&self) -> bool {
        self.keybinding.on_release
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum LineType {
    Key,
    Command,
    // In case we want to add more statements
    Statement,
    // Other stands for comments and empty lines
    Other,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Line {
    pub content: String,
    pub linetype: LineType,
    pub linenumber: u32,
}

impl Line {
    pub fn new(content: String, linetype: LineType, linenumber: u32) -> Self {
        Line { content, linetype, linenumber }
    }

    pub fn mark_line(line: &str) -> LineType {
        if line.trim().is_empty() || line.trim().starts_with(COMMENT_SYMBOL) {
            LineType::Other
        } else if line.starts_with(' ') || line.starts_with('\t') {
            LineType::Command
        } else {
            LineType::Key
        }
    }

    pub fn from_str(content: &str, linenumber: u32) -> Self {
        Line { content: content.to_string(), linetype: Self::mark_line(content), linenumber }
    }

    pub fn join_line(self, other: &Self) -> Self {
        if self.linetype == other.linetype {
            Line {
                content: self.content.strip_suffix('\\').unwrap().to_owned() + &other.content,
                linetype: self.linetype,
                linenumber: self.linenumber,
            }
        } else {
            Line {
                content: self.content.strip_suffix('\\').unwrap().to_string(),
                linetype: self.linetype,
                linenumber: self.linenumber,
            }
        }
    }

    pub fn trim(&self) -> Self {
        Line {
            content: self.content.trim().to_string(),
            linetype: self.clone().linetype,
            linenumber: self.linenumber,
        }
    }
    pub fn is_to_join(&self) -> bool {
        self.content.ends_with('\\')
    }
}

pub fn load_to_lines(content: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut linenumber = 0;
    for line in content.lines() {
        linenumber += 1;
        let current_line = Line::from_str(line, linenumber);
        if current_line.linetype == LineType::Other {
            continue;
        }
        lines.push(current_line);
    }
    lines
}

pub fn join_lines(lines: Vec<Line>) -> Vec<Line> {
    let mut joined_lines = Vec::new();
    let mut prev_line = lines[0].clone().trim();
    for line in lines.iter().skip(1) {
        if !prev_line.is_to_join() {
            joined_lines.push(prev_line.clone());
            prev_line = line.clone().trim();
            continue;
        }
        if prev_line.is_to_join() {
            prev_line = prev_line.join_line(&line.trim());
        }
    }
    joined_lines.push(prev_line);
    joined_lines
}

mod test_parse {
    use crate::parse::*;
    #[test]
    fn join_two_lines() {
        let line1 = Line::new("ctrl+shift+\\".to_string(), LineType::Key, 3);
        let line2 = Line::new("b".to_string(), LineType::Key, 3);
        assert_eq!(
            line1.join_line(&line2),
            Line::new("ctrl+shift+b".to_string(), LineType::Key, 3)
        );
    }

    #[test]
    fn mark_line() {
        let key = "ctrl+shift+\\".to_string();
        let command = " a".to_string();
        let comment = "# a".to_string();
        let empty = "".to_string();
        assert_eq!(LineType::Key, Line::mark_line(&key));
        assert_eq!(LineType::Command, Line::mark_line(&command));
        assert_eq!(LineType::Other, Line::mark_line(&comment));
        assert_eq!(LineType::Other, Line::mark_line(&empty));
    }

    #[test]
    fn join_all_lines() {
        let content = "super + b
    b
super + \\
a
    a\\
    a";
        let lines = load_to_lines(content);
        let joined_lines = join_lines(lines);
        assert_eq!(
            joined_lines,
            vec![
                Line::new("super + b".to_string(), LineType::Key, 1),
                Line::new("b".to_string(), LineType::Command, 2),
                Line::new("super + a".to_string(), LineType::Key, 3),
                Line::new("aa".to_string(), LineType::Command, 5),
            ]
        );
    }
}