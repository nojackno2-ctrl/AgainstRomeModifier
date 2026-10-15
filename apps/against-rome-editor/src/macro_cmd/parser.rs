//! DSL 地圖編輯指令解析器與 Tab 自動補全引擎 (Command Parser & Auto-completion)

use std::collections::HashMap;

use crate::macro_cmd::execution::EditorCommand;
use crate::macro_cmd::models::{CommandExecutionContext, CommandParameterType, ParsedCommand};

/// 解析單行指令文字為結構化 ParsedCommand。
/// 若為註解行或空白行，回傳 None。
pub fn parse(
    line: &str,
    variables: Option<&HashMap<String, String>>,
) -> Option<ParsedCommand> {
    if line.trim().is_empty() {
        return None;
    }

    let mut trimmed = line.trim();
    if trimmed.starts_with('#') || trimmed.starts_with("//") {
        return None; // 註解行
    }

    // 變數替換 ($var)
    let substituted;
    if let Some(vars) = variables {
        if !vars.is_empty() && trimmed.contains('$') {
            substituted = substitute_variables(trimmed, vars);
            trimmed = &substituted;
        }
    }

    let tokens = tokenize(trimmed);
    if tokens.is_empty() {
        return None;
    }

    let command_token = &tokens[0];
    let command_name = if command_token.starts_with('/') {
        &command_token[1..]
    } else {
        command_token.as_str()
    };

    let mut positional_args = Vec::new();
    let mut named_flags = HashMap::new();

    let mut i = 1;
    while i < tokens.len() {
        let token = &tokens[i];
        if token.starts_with("--") && token.len() > 2 {
            let flag_body = &token[2..];
            if let Some(eq_idx) = flag_body.find('=') {
                let flag_name = &flag_body[..eq_idx];
                let flag_val = &flag_body[(eq_idx + 1)..];
                named_flags.insert(flag_name.to_string(), Some(flag_val.to_string()));
            } else {
                // 檢查下一個 token 是否為參數值 (非旗標)
                if i + 1 < tokens.len() && !tokens[i + 1].starts_with('-') {
                    named_flags.insert(flag_body.to_string(), Some(tokens[i + 1].clone()));
                    i += 1;
                } else {
                    named_flags.insert(flag_body.to_string(), None); // 布林旗標開關
                }
            }
        } else if token.starts_with('-')
            && token.len() > 1
            && !token.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            // 短旗標 -f val
            let flag_body = &token[1..];
            if let Some(eq_idx) = flag_body.find('=') {
                let flag_name = &flag_body[..eq_idx];
                let flag_val = &flag_body[(eq_idx + 1)..];
                named_flags.insert(flag_name.to_string(), Some(flag_val.to_string()));
            } else if i + 1 < tokens.len() && !tokens[i + 1].starts_with('-') {
                named_flags.insert(flag_body.to_string(), Some(tokens[i + 1].clone()));
                i += 1;
            } else {
                named_flags.insert(flag_body.to_string(), None);
            }
        } else {
            positional_args.push(token.clone());
        }
        i += 1;
    }

    Some(ParsedCommand::new(
        trimmed,
        command_name,
        positional_args,
        named_flags,
    ))
}

/// 將指令行切分為 Token，妥善處理引號字串 ("...") 與逸出字元。
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    if text.trim().is_empty() {
        return tokens;
    }

    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape_next = false;

    for c in text.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' {
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_quotes = !in_quotes;
            continue;
        }

        if c.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// 執行變數替換 ($varName -> varValue)。
pub fn substitute_variables(text: &str, variables: &HashMap<String, String>) -> String {
    let mut sb = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$'
            && i + 1 < chars.len()
            && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_')
        {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            let var_name: String = chars[start..end].iter().collect();
            if let Some(val) = variables.get(&var_name) {
                sb.push_str(val);
            } else {
                sb.push('$');
                sb.push_str(&var_name);
            }
            i = end;
        } else {
            sb.push(chars[i]);
            i += 1;
        }
    }

    sb
}

/// Tab 自動補全建議生成器。
pub fn get_completions(
    current_input: &str,
    cursor_position: usize,
    context: Option<&CommandExecutionContext>,
    registered_commands: &[Box<dyn EditorCommand>],
) -> Vec<String> {
    if current_input.is_empty() {
        return Vec::new();
    }
    let before_cursor = if cursor_position <= current_input.len() {
        &current_input[..cursor_position]
    } else {
        current_input
    };

    let tokens = tokenize(before_cursor);
    let ends_with_space = before_cursor.ends_with(' ');

    // 1. 若處於第一個 token 且尚未輸入空格：補全指令名稱
    if tokens.is_empty() || (tokens.len() == 1 && !ends_with_space) {
        let prefix = if tokens.len() == 1 { &tokens[0] } else { "" };
        let clean_prefix = if prefix.starts_with('/') {
            &prefix[1..]
        } else {
            prefix
        };

        let mut matches = Vec::new();
        for cmd in registered_commands {
            if cmd.name().to_ascii_lowercase().starts_with(&clean_prefix.to_ascii_lowercase()) {
                matches.push(format!("/{}", cmd.name()));
            }
            for alias in cmd.aliases() {
                if alias.to_ascii_lowercase().starts_with(&clean_prefix.to_ascii_lowercase()) {
                    matches.push(format!("/{}", alias));
                }
            }
        }
        matches.sort();
        matches.dedup();
        return matches;
    }

    // 2. 指令已確定，正在輸入引數或旗標
    let command_token = &tokens[0];
    let cmd_name = if command_token.starts_with('/') {
        &command_token[1..]
    } else {
        command_token.as_str()
    };

    let target_cmd = registered_commands.iter().find(|c| {
        c.name().eq_ignore_ascii_case(cmd_name)
            || c.aliases().iter().any(|a| a.eq_ignore_ascii_case(cmd_name))
    });

    let target_cmd = match target_cmd {
        Some(c) => c,
        None => return Vec::new(),
    };

    let last_token = if ends_with_space {
        ""
    } else {
        tokens.last().map(|s| s.as_str()).unwrap_or("")
    };

    // 2.1 正在輸入旗標 (--flag)
    if last_token.starts_with("--") || (ends_with_space && last_token.is_empty()) {
        let flag_prefix = if last_token.starts_with("--") {
            &last_token[2..]
        } else {
            ""
        };
        let flag_completions: Vec<String> = target_cmd
            .parameters()
            .iter()
            .filter(|p| p.param_type == CommandParameterType::Flag || !p.is_required)
            .map(|p| format!("--{}", p.name))
            .filter(|f| f[2..].to_ascii_lowercase().starts_with(&flag_prefix.to_ascii_lowercase()))
            .collect();

        if !flag_completions.is_empty() {
            return flag_completions;
        }
    }

    // 2.2 正在輸入物件範本 (Template / Object)
    if let Some(ctx) = context {
        if target_cmd
            .parameters()
            .iter()
            .any(|p| p.param_type == CommandParameterType::TemplateName)
        {
            let template_matches: Vec<String> = ctx
                .available_object_types
                .iter()
                .filter(|n| n.to_ascii_lowercase().starts_with(&last_token.to_ascii_lowercase()))
                .take(20)
                .cloned()
                .collect();

            if !template_matches.is_empty() {
                return template_matches;
            }
        }

        // 2.3 正在輸入材質名稱 (Texture)
        if target_cmd
            .parameters()
            .iter()
            .any(|p| p.param_type == CommandParameterType::TextureName)
        {
            let texture_matches: Vec<String> = ctx
                .known_textures
                .iter()
                .filter(|t| t.to_ascii_lowercase().starts_with(&last_token.to_ascii_lowercase()))
                .take(20)
                .cloned()
                .collect();

            if !texture_matches.is_empty() {
                return texture_matches;
            }
        }
    }

    Vec::new()
}
