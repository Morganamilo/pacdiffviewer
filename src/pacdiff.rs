use crate::config::Config;
use crate::error::{Error, Result};

use std::env;
use std::io::BufRead;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use alpm::Alpm;

use chrono::{DateTime, Utc};

#[derive(PartialEq)]
enum Kind {
    Pacsave,
    Pacnew,
}

struct Backup {
    package: String,
    file: PathBuf,
    pacfiles: Vec<PathBuf>,
    kind: Kind,
}

pub fn run(config: &Config) -> Result<()> {
    let mut pacconf = pacmanconf::Config::with_opts(
        None,
        config.config.as_ref().map(|s| s.as_str()),
        config.root.as_ref().map(|s| s.as_str()),
    )?;
    if let Some(ref db_path) = config.dbpath {
        pacconf.db_path = db_path.clone();
    }
    let alpm = Alpm::new(&pacconf.root_dir, &pacconf.db_path)
        .map_err(|e| Error::AlpmInit(e, pacconf.root_dir, pacconf.db_path))?;

    let mut backups = get_backups(&config, &alpm)?;

    if !config.all && !backups.is_empty() {
        print_backups(config, &backups);
        let input =
            readline(config, "Files to manage (eg: all, 1 2 3, 1-3 or ^4): ")?.to_lowercase();
        backups = filter_backups(backups, &input);
    }

    if config.output {
        for backup in &backups {
            for file in &backup.pacfiles {
                println!("{}", file.display());
            }
        }
    } else {
        for (n, backup) in backups.iter().enumerate() {
            if backup.manage(config, n + 1, backups.len())? {
                break;
            }
        }
    }

    Ok(())
}

fn readline(config: &Config, prompt: &str) -> Result<String> {
    let p = config.color.prompt;
    let b = config.color.bold;
    print!("{} {}", p.paint("::"), b.paint(prompt));
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    if !line.ends_with('\n') {
        println!();
    }
    Ok(line)
}

impl Backup {
    fn format_pacfiles(&self) -> String {
        if self.pacfiles.len() == 1 {
            return self.pacfiles[0].to_string_lossy().into_owned();
        }

        let orig_file = self.file.to_string_lossy();
        let mut pacfiles = format!("{}{{", orig_file);

        let mut iter = self.pacfiles.iter();
        iter.next_back();

        for file in iter {
            let file = file.to_string_lossy();
            let file = file.trim_start_matches(&*orig_file);
            let file = match self.kind {
                Kind::Pacnew => file.trim_start_matches(".pacnew"),
                Kind::Pacsave => file.trim_start_matches(".pacsave"),
            };

            if !file.is_empty() {
                pacfiles.push_str(file.trim_start_matches('.'));
                pacfiles.push_str(", ");
            }
        }

        let file = self.pacfiles.last().unwrap().to_string_lossy();
        let file = file.trim_start_matches(&*orig_file);
        let file = match self.kind {
            Kind::Pacnew => file.trim_start_matches(".pacnew"),
            Kind::Pacsave => file.trim_start_matches(".pacsave"),
        };

        if !file.is_empty() {
            pacfiles.push_str(file.trim_start_matches('.'));
        }

        pacfiles.push('}');
        pacfiles
    }

    fn view(&self, config: &Config) -> Result<()> {
        use std::ffi::OsString;

        let bin;
        let mut args = Vec::<OsString>::new();

        if config.nosudoedit || config.sudouser.is_none() {
            let mut split = config.diffprog.split_whitespace();
            bin = split.next().unwrap();
            args.extend(split.map(|e| e.into()));
            args.push(self.file.clone().into());
            args.extend(self.pacfiles.iter().map(|p| p.into()));
        } else {
            let user = config.sudouser.as_ref().unwrap();
            bin = "sudo";

            args.push(format!("SUDO_EDITOR={}", &config.diffprog).into());
            args.push("-u".into());
            args.push(user.into());
            args.push("sudo".into());
            args.push("-e".into());
            args.push(self.file.clone().into());
            args.extend(self.pacfiles.iter().map(|p| p.into()));
        }

        let mut command = Command::new(bin);
        command.args(&args);

        let exit = match command.spawn() {
            Err(err) => {
                return Err(Error::CommandFailed(
                    bin.to_string(),
                    args.iter()
                        .map(|s| s.to_string_lossy().to_string())
                        .collect(),
                    err,
                ));
            }
            Ok(o) => o,
        }
        .wait()?;

        if !exit.success() {
            let e = config.color.error;
            let err = Error::CommandNonZero(
                bin.to_string(),
                args.iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect(),
                exit.code(),
            );
            eprintln!("{} {}", e.paint("error:"), err);
        }

        Ok(())
    }

    fn manage(&self, config: &Config, curr: usize, total: usize) -> Result<bool> {
        let e = config.color.error;
        let b = config.color.bold;
        let maxnum = total.to_string().len();

        loop {
            let input;

            if let Some(ref action) = config.action {
                input = action.to_string();
            } else {
                println!(
                    "\n{} [{:0num$}/{:num$}] {}",
                    config.color.info.paint("==>"),
                    curr,
                    total,
                    b.paint(self.format_pacfiles()),
                    num = maxnum,
                );
                let line = readline(config, "[V]iew [S]kip [R]emove [O]verwrite [Q]uit: ")?;
                input = line.to_lowercase();
            }

            if input.starts_with('v') {
                self.view(config)?;
            } else if input.starts_with('s') {
                break;
            } else if input.starts_with('r') {
                for file in &self.pacfiles {
                    if let Err(err) = fs::remove_file(file) {
                        eprintln!(
                            "{} failed to remove '{}': {}",
                            e.paint("error:",),
                            file.display(),
                            err
                        );
                    }
                }
                break;
            } else if input.starts_with('o') {
                let mut iter = self.pacfiles.iter();

                if let Some(file) = iter.next_back() {
                    if let Err(err) = fs::rename(file, &self.file) {
                        eprintln!(
                            "{} failed to move '{}' to '{}': {}",
                            e.paint("error:"),
                            file.display(),
                            self.file.display(),
                            err
                        );
                    }
                }

                for file in iter {
                    if let Err(err) = fs::remove_file(file) {
                        eprintln!(
                            "{} failed to remove '{}': {}",
                            e.paint("error:"),
                            file.display(),
                            err
                        );
                    }
                }
                break;
            } else if input.starts_with('q') {
                return Ok(true);
            } else {
                break;
            }
        }

        Ok(false)
    }
}

fn print_backups(config: &Config, backups: &[Backup]) {
    let mut maxnum = "".len();
    let mut maxpkg = "Package".len();
    let mut maxfile = "File".len();
    let b = config.color.bold;

    for (n, backup) in backups.iter().enumerate() {
        for (m, _) in backup.pacfiles.iter().enumerate() {
            maxnum = m + n + 1;
            maxpkg = maxpkg.max(backup.package.len());
        }

        let files = backup.format_pacfiles();
        maxfile = maxfile.max(files.len());
    }

    let maxnum = maxnum.to_string().len();

    let header = format!(
        "{:num$}  {:pkg$}  {:file$}  {}",
        "",
        "Package",
        "File",
        "Modified",
        num = maxnum,
        pkg = maxpkg,
        file = maxfile
    );

    println!("{}", b.paint(header));

    for (n, backup) in backups.iter().enumerate() {
        let mut time = None;

        for file in &backup.pacfiles.iter().last() {
            let metadata = file.metadata();
            let metadata = match metadata {
                Ok(o) => o,
                Err(err) => {
                    println!("failed to stat '{}': {}", file.display(), err);
                    continue;
                }
            };
            let modified = metadata.modified();
            let modified = match modified {
                Ok(o) => o,
                Err(err) => {
                    println!("{}", err);
                    continue;
                }
            };

            time = Some(DateTime::<Utc>::from(modified));
        }

        let time = if let Some(time) = time {
            time.format("%c").to_string()
        } else {
            "Unknown".to_string()
        };

        println!(
            "{:0num$}  {:pkg$}  {:file$}  {}",
            n + 1,
            backup.package,
            backup.format_pacfiles(),
            time,
            num = maxnum,
            pkg = maxpkg,
            file = maxfile
        );
    }
}

fn filter_backups(backups: Vec<Backup>, input: &str) -> Vec<Backup> {
    if input.trim().is_empty() {
        return backups;
    }

    let mut whitelist = vec![false; backups.len()];
    let tokens = input.split_whitespace();

    for token in tokens {
        let len = token.len();
        let token = token.trim_start_matches('^');
        let invert = token.len() != len;

        if token.starts_with('a') {
            for b in &mut whitelist {
                *b = true;
            }
        } else if token.starts_with('n') {
            for b in &mut whitelist {
                *b = false;
            }
        }

        let mut range = token.splitn(2, '-');
        let min = range.next().unwrap();

        let min = match min.parse::<usize>() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let max = match range.next() {
            Some(c) => c.parse::<usize>().unwrap_or(min),
            None => min,
        };

        let range = if min < max { min..=max } else { max..=min };

        for (n, b) in whitelist.iter_mut().enumerate() {
            if range.contains(&(n + 1)) != invert {
                *b = true
            }
        }
    }

    backups
        .into_iter()
        .enumerate()
        .filter(|(n, _)| whitelist[*n])
        .map(|(_, b)| b)
        .collect()
}

fn get_backups(config: &Config, alpm: &Alpm) -> Result<Vec<Backup>> {
    let root = Path::new(alpm.root());
    let mut backups = Vec::new();
    let mut pkgs = Vec::new();
    let e = config.color.error;
    let b = config.color.bold;

    if config.targets.is_empty() {
        pkgs.extend(alpm.localdb().pkgs()?);
    } else {
        for target in &config.targets {
            match alpm.localdb().pkg(target.clone()) {
                Ok(p) => pkgs.push(p),
                Err(_) => eprintln!(
                    "{} {}: {}",
                    e.paint("error:"),
                    "target not found",
                    b.paint(target)
                ),
            }
        }
    }

    for pkg in pkgs {
        for backup in pkg.backup() {
            let path = root.join(&backup.name());
            let (mut pacnew, mut pacsave) = find_backups_for_file(config, &path)?;
            pacnew.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
            pacsave.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));

            if !pacnew.is_empty() {
                let backup = Backup {
                    package: pkg.name().into(),
                    file: path.clone(),
                    pacfiles: pacnew,
                    kind: Kind::Pacnew,
                };

                backups.push(backup);
            }

            if !pacsave.is_empty() {
                let backup = Backup {
                    package: pkg.name().into(),
                    file: path,
                    pacfiles: pacsave,
                    kind: Kind::Pacsave,
                };

                backups.push(backup);
            }
        }
    }

    Ok(backups)
}

fn find_backups_for_file(config: &Config, file: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut newfiles = Vec::new();
    let mut savefiles = Vec::new();
    let e = config.color.error;

    let parent = match file.parent() {
        Some(o) => o,
        None => return Ok((newfiles, savefiles)),
    };

    let read = fs::read_dir(parent);
    let read = match read {
        Ok(read) => read,
        Err(err) => {
            if config.verbose {
                eprintln!("{} {}: {}", e.paint("error:"), parent.display(), err);
            }
            return Ok((newfiles, savefiles));
        }
    };

    let filename = match file.file_name() {
        Some(o) => o,
        None => return Ok((newfiles, savefiles)),
    }
    .to_string_lossy();

    let pacsave = format!("{}.pacnew", filename);
    let pacnew = format!("{}.pacsave", filename);

    for entry in read {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                if config.verbose {
                    eprintln!("{} {}: {}", e.paint("error:"), parent.display(), err);
                }
                continue;
            }
        };

        let pacfile = entry.file_name();
        let pacfile = pacfile.to_string_lossy();

        if pacfile.starts_with(&pacsave) {
            newfiles.push(entry.path());
        } else if pacfile.starts_with(&pacnew) {
            savefiles.push(entry.path());
        }
    }

    Ok((newfiles, savefiles))
}
