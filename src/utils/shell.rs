use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    pub fn detect() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_default();
        if shell.contains("zsh") {
            Shell::Zsh
        } else if shell.contains("fish") {
            Shell::Fish
        } else {
            Shell::Bash
        }
    }

    pub fn profile_path(&self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        match self {
            Shell::Zsh => home.join(".zshrc"),
            Shell::Bash => home.join(".bashrc"),
            Shell::Fish => home.join(".config/fish/config.fish"),
        }
    }

    pub fn node_options_line(&self) -> String {
        let inject_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".vision-bridge/inject.js");

        match self {
            Shell::Zsh | Shell::Bash => {
                format!(r#"export NODE_OPTIONS="--require {} $NODE_OPTIONS""#, inject_path.display())
            }
            Shell::Fish => {
                format!(r#"set -gx NODE_OPTIONS "--require {} $NODE_OPTIONS""#, inject_path.display())
            }
        }
    }

    pub fn add_to_profile(&self) -> Result<()> {
        let profile = self.profile_path();
        let line = self.node_options_line();

        // 读取现有内容
        let content = if profile.exists() {
            std::fs::read_to_string(&profile)?
        } else {
            String::new()
        };

        // 检查是否已存在
        if content.contains(&line) {
            return Ok(());
        }

        // 追加配置
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&profile)?;

        writeln!(file, "\n# Vision Bridge")?;
        writeln!(file, "{}", line)?;

        Ok(())
    }

    pub fn remove_from_profile(&self) -> Result<()> {
        let profile = self.profile_path();
        if !profile.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&profile)?;
        let line = self.node_options_line();

        // 移除配置行和注释
        let new_content: String = content
            .lines()
            .filter(|l| !l.contains(&line) && !l.contains("# Vision Bridge"))
            .collect::<Vec<&str>>()
            .join("\n");

        std::fs::write(&profile, new_content)?;

        Ok(())
    }
}
