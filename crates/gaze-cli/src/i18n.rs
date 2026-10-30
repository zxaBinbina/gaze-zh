// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use clap::{
    ArgAction, Command,
    error::{ContextKind, ErrorKind},
};
use console::{Alignment, pad_str};
use dialoguer::theme::{ColorfulTheme, Theme};
use std::fmt;

/// Keep command names, option names and values stable; only translate presentation.
pub fn command(mut command: Command) -> Command {
    // Let clap build its help tree before changing presentation, so `gaze help`
    // and nested help keep their original parsing and completion behavior.
    command.build();
    translate_command(command)
}

fn translate_command(mut command: Command) -> Command {
    if command.get_name() == "help" {
        command = command.about("显示此帮助或指定子命令的帮助");
    }
    command
        .subcommand_help_heading("命令")
        .subcommand_value_name("命令")
        .help_template("{about-with-newline}\n用法：{usage}\n\n{all-args}{after-help}")
        .mut_args(|arg| {
            let positional = arg.is_positional();
            let arg = match arg.get_id().as_str() {
                "user" => arg.value_name("用户名"),
                "face" => arg.value_name("人脸名称"),
                "from" => arg.value_name("旧名称"),
                "to" => arg.value_name("新名称"),
                "key" => arg.value_name("配置项").help("配置键，例如 security.level"),
                "value" => arg.value_name("值").help("配置值，使用原有标识，例如 high"),
                _ => arg,
            };
            let arg = match arg.get_action() {
                ArgAction::Help => arg.help("显示帮助"),
                ArgAction::Version => arg.help("显示版本"),
                _ => arg,
            };
            arg.help_heading(if positional { "参数" } else { "选项" })
        })
        .mut_subcommands(translate_command)
}

pub fn exit<T>(error: clap::Error) -> T {
    let code = error.exit_code();
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp
            | ErrorKind::DisplayVersion
            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    ) {
        let color = if error.use_stderr() {
            console::colors_enabled_stderr()
        } else {
            console::colors_enabled()
        };
        let text = if color {
            error.render().ansi().to_string()
        } else {
            error.to_string()
        }
        .replace("[OPTIONS]", "[选项]")
        .replace("[default:", "[默认：")
        .replace("[possible values:", "[可选值：")
        .replace("[aliases:", "[别名：")
        .replace("[short aliases:", "[短别名：")
        .replace("[long aliases:", "[长别名：");
        if error.use_stderr() {
            eprint!("{text}");
        } else {
            print!("{text}");
        }
    } else {
        eprint!("{}", error_text(&error));
    }
    std::process::exit(code);
}

fn error_text(error: &clap::Error) -> String {
    let message = match error.kind() {
        ErrorKind::UnknownArgument => "未知参数",
        ErrorKind::InvalidSubcommand => "未知命令",
        ErrorKind::ArgumentConflict => "参数不能同时使用或重复指定",
        ErrorKind::MissingRequiredArgument => "缺少必填参数",
        ErrorKind::MissingSubcommand => "缺少命令",
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => "参数值无效",
        ErrorKind::TooManyValues => "参数值过多",
        ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => "参数值数量不正确",
        ErrorKind::NoEquals => "请用等号连接选项和值",
        ErrorKind::InvalidUtf8 => "参数不是有效的 UTF-8 文本",
        _ => "无法解析命令行参数",
    };
    let mut out = format!("错误：{message}\n");
    for (key, label) in [
        (ContextKind::InvalidSubcommand, "命令"),
        (ContextKind::InvalidArg, "参数"),
        (ContextKind::PriorArg, "已指定"),
        (ContextKind::InvalidValue, "输入值"),
        (ContextKind::ValidValue, "可选值"),
        (ContextKind::SuggestedSubcommand, "可能的命令"),
        (ContextKind::SuggestedArg, "可能的参数"),
        (ContextKind::SuggestedValue, "可能的值"),
    ] {
        if let Some(value) = error.get(key) {
            out.push_str(&format!("  {label}：{value}\n"));
        }
    }
    if let Some(usage) = error.get(ContextKind::Usage) {
        out.push_str(&format!(
            "\n{}\n",
            usage
                .to_string()
                .replacen("Usage:", "用法：", 1)
                .replace("[OPTIONS]", "[选项]")
        ));
    }
    out.push_str("\n使用 --help 查看帮助。\n");
    out
}

/// Rust's formatting width counts characters, while Chinese characters occupy two columns.
pub fn cell(text: &str, width: usize, left: bool) -> String {
    let truncated = console::truncate_str(text, width, "…");
    pad_str(
        &truncated,
        width,
        if left {
            Alignment::Left
        } else {
            Alignment::Right
        },
        None,
    )
    .into_owned()
}

#[derive(Default)]
pub struct ChineseTheme(ColorfulTheme);

impl Theme for ChineseTheme {
    fn format_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        self.0.format_prompt(f, prompt)
    }
    fn format_error(&self, f: &mut dyn fmt::Write, err: &str) -> fmt::Result {
        self.0.format_error(f, err)
    }
    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<&str>,
    ) -> fmt::Result {
        self.0.format_input_prompt(f, prompt, default)
    }
    fn format_input_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        value: &str,
    ) -> fmt::Result {
        self.0.format_input_prompt_selection(f, prompt, value)
    }
    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        self.0.format_select_prompt_item(f, text, active)
    }
    fn format_confirm_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<bool>,
    ) -> fmt::Result {
        let t = &self.0;
        if !prompt.is_empty() {
            write!(
                f,
                "{} {} ",
                t.prompt_prefix,
                t.prompt_style.apply_to(prompt)
            )?;
        }
        write!(f, "{} {}", t.hint_style.apply_to("(y/n)"), t.prompt_suffix)?;
        if let Some(default) = default {
            write!(
                f,
                " {}",
                t.defaults_style.apply_to(if default { "是" } else { "否" })
            )?;
        }
        Ok(())
    }
    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> fmt::Result {
        self.0.format_input_prompt_selection(
            f,
            prompt,
            match selection {
                Some(true) => "是",
                Some(false) => "否",
                None => "已取消",
            },
        )
    }
    fn format_password_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
    ) -> fmt::Result {
        self.0.format_input_prompt_selection(f, prompt, "[已隐藏]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};

    #[test]
    fn chinese_help_preserves_nested_commands_and_flags() {
        let cmd = command(crate::Cli::command());
        cmd.clone().debug_assert();
        for args in [
            vec!["gaze", "help"],
            vec!["gaze", "help", "config", "set"],
            vec!["gaze", "config", "help", "get"],
        ] {
            let help = cmd.clone().try_get_matches_from(args).unwrap_err();
            assert_eq!(help.kind(), ErrorKind::DisplayHelp);
            assert!(help.to_string().contains("用法："));
        }
        let matches = cmd
            .clone()
            .try_get_matches_from(["gaze", "config", "set", "security.level", "high"])
            .unwrap();
        let cli = crate::Cli::from_arg_matches(&matches).unwrap();
        assert!(
            matches!(cli.command, crate::Commands::Config { action: Some(crate::ConfigAction::Set { key, value }), .. } if key == "security.level" && value == "high")
        );
        let error = cmd
            .try_get_matches_from(["gaze", "auth", "--silent", "--verbose"])
            .unwrap_err();
        let text = error_text(&error);
        assert!(
            text.contains("--silent")
                && text.contains("--verbose")
                && text.contains("不能同时使用")
        );
    }

    #[test]
    fn table_cells_keep_terminal_column_width_for_chinese_and_long_names() {
        for (text, width) in [
            ("人脸", 20),
            ("RGB 相似度", 10),
            ("中文人脸名称超过原有表格宽度的情况", 20),
            ("work", 20),
        ] {
            for left in [true, false] {
                assert_eq!(console::measure_text_width(&cell(text, width, left)), width);
            }
        }
    }
}
