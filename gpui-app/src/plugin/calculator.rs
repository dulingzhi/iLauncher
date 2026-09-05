//! 计算器插件：数学表达式、进制转换、单位转换
//! （对齐 src-tauri/src/plugin/calculator.rs 行为）。
//! 无 gpui 依赖，可单元测试。
//!
//! 相对 Tauri 版的偏离：
//!   - 正则由手写字符类谓词替代（不引入 regex 依赖；模式本身 trivial）
//!   - execute 的 copy 动作返回 ExecuteOutcome::Copy（副作用上移 Launcher 层），
//!     且先走沙盒 ClipboardAccess 权限检查——检查事件落审计管道（Tauri 版 copy 是空实现）

use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::plugin::sandbox::{PluginPermission, SandboxManager};
use crate::plugin::{ExecuteOutcome, Plugin, PluginMetadata, PluginAction, QueryContext, QueryResult};

pub struct CalculatorPlugin {
    metadata: PluginMetadata,
    sandbox: Arc<SandboxManager>,
}

impl CalculatorPlugin {
    pub fn new(sandbox: Arc<SandboxManager>) -> Self {
        Self {
            metadata: PluginMetadata::new("calculator", "Calculator")
                .with_description("数学计算、进制转换、单位转换")
                .with_icon("🧮")
                .with_trigger_keywords(vec!["=".to_string(), "calc".to_string()]),
            sandbox,
        }
    }

    /// 数学表达式字符类（对齐 Tauri 正则 ^[\d+\-*/().\s]+$）
    fn is_expr_chars(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || "+-*/(). ".contains(c))
    }

    fn is_hex(s: &str) -> bool {
        let body = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
        body.is_some_and(|b| !b.is_empty() && b.chars().all(|c| c.is_ascii_hexdigit()))
    }

    fn is_bin(s: &str) -> bool {
        let body = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B"));
        body.is_some_and(|b| !b.is_empty() && b.chars().all(|c| c == '0' || c == '1'))
    }

    fn is_oct(s: &str) -> bool {
        let body = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O"));
        body.is_some_and(|b| !b.is_empty() && b.chars().all(|c| ('0'..='7').contains(&c)))
    }

    /// 计算表达式（对齐 Tauri calculate/eval_expr/eval_term/eval_factor 递归下降）
    fn calculate(&self, expr: &str) -> Result<f64> {
        let expr = expr.replace(' ', "");
        self.eval_expr(&expr)
    }

    fn eval_expr(&self, expr: &str) -> Result<f64> {
        // 处理加减
        let parts: Vec<&str> = expr.split(['+', '-']).collect();
        let ops: Vec<char> = expr.chars().filter(|c| *c == '+' || *c == '-').collect();

        if parts.len() > 1 {
            let mut result = self.eval_term(parts[0])?;
            for (i, part) in parts.iter().enumerate().skip(1) {
                let val = self.eval_term(part)?;
                match ops.get(i - 1) {
                    Some('+') => result += val,
                    Some('-') => result -= val,
                    _ => {}
                }
            }
            return Ok(result);
        }
        self.eval_term(expr)
    }

    fn eval_term(&self, term: &str) -> Result<f64> {
        // 处理乘除
        let parts: Vec<&str> = term.split(['*', '/']).collect();
        let ops: Vec<char> = term.chars().filter(|c| *c == '*' || *c == '/').collect();

        if parts.len() > 1 {
            let mut result = self.eval_factor(parts[0])?;
            for (i, part) in parts.iter().enumerate().skip(1) {
                let val = self.eval_factor(part)?;
                match ops.get(i - 1) {
                    Some('*') => result *= val,
                    Some('/') => {
                        if val == 0.0 {
                            return Err(anyhow!("Division by zero"));
                        }
                        result /= val;
                    }
                    _ => {}
                }
            }
            return Ok(result);
        }
        self.eval_factor(term)
    }

    fn eval_factor(&self, factor: &str) -> Result<f64> {
        // 处理括号
        if factor.starts_with('(') && factor.ends_with(')') && factor.len() > 1 {
            return self.eval_expr(&factor[1..factor.len() - 1]);
        }
        factor.parse::<f64>().map_err(|e| anyhow!("Invalid number: {e}"))
    }

    /// 数字格式：整数直出，小数 trim 尾零（对齐 Tauri 输出格式）
    fn format_number(v: f64) -> String {
        if v.fract() == 0.0 && v.abs() < 9.0e15 {
            format!("{}", v as i64)
        } else {
            format!("{:.6}", v).trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }

    /// 进制转换
    fn convert_base(&self, input: &str) -> Option<QueryResult> {
        let input_lower = input.to_lowercase();
        let (num, base_name) = if Self::is_hex(input) {
            (i64::from_str_radix(&input_lower[2..], 16).ok()?, "十六进制")
        } else if Self::is_bin(input) {
            (i64::from_str_radix(&input_lower[2..], 2).ok()?, "二进制")
        } else if Self::is_oct(input) {
            (i64::from_str_radix(&input_lower[2..], 8).ok()?, "八进制")
        } else {
            return None;
        };

        let conversions = format!(
            "十进制: {num} | 十六进制: 0x{num:X} | 二进制: 0b{num:b} | 八进制: 0o{num:o}"
        );
        Some(
            QueryResult::new(conversions.clone(), conversions)
                .with_subtitle(format!("{base_name} 进制转换"))
                .with_icon("🔢")
                .with_score(900)
                .with_action(PluginAction::default_action("copy", "复制")),
        )
    }

    /// 单位转换（对齐 Tauri convert_unit 的换算表）
    fn convert_unit(&self, input: &str) -> Option<Vec<QueryResult>> {
        // 解析 "数字+单位"（对齐 Tauri 正则 ^([\d.]+)\s*([a-zA-Z]+)$）
        let split_at = input.find(|c: char| c.is_ascii_alphabetic())?;
        let (num_str, unit_raw) = input.split_at(split_at);
        let value: f64 = num_str.trim().parse().ok()?;
        let unit = unit_raw.trim().to_lowercase();
        if unit_raw.chars().any(|c| !c.is_ascii_alphabetic() && !c.is_whitespace()) {
            return None;
        }

        let mut results = Vec::new();
        let mut push = |conversions: String, subtitle: String, icon: &str| {
            results.push(
                QueryResult::new(conversions.clone(), conversions)
                    .with_subtitle(subtitle)
                    .with_icon(icon)
                    .with_score(850)
                    .with_action(PluginAction::default_action("copy", "复制")),
            );
        };

        match unit.as_str() {
            // 长度单位
            "m" => push(
                format!("{:.3}km | {:.0}cm | {:.0}mm | {:.2}ft", value / 1000.0, value * 100.0, value * 1000.0, value * 3.28084),
                format!("长度转换: {value}m"), "📏",
            ),
            "km" => push(format!("{:.0}m | {:.2}mi", value * 1000.0, value * 0.621371), format!("长度转换: {value}km"), "📏"),
            "cm" => push(format!("{:.3}m | {:.0}mm | {:.2}in", value / 100.0, value * 10.0, value * 0.393701), format!("长度转换: {value}cm"), "📏"),
            "mm" => push(format!("{:.3}m | {:.2}cm", value / 1000.0, value / 10.0), format!("长度转换: {value}mm"), "📏"),
            // 重量单位
            "kg" => push(format!("{:.0}g | {:.2}lb | {:.2}oz", value * 1000.0, value * 2.20462, value * 35.274), format!("重量转换: {value}kg"), "⚖️"),
            "g" => push(format!("{:.3}kg | {:.0}mg", value / 1000.0, value * 1000.0), format!("重量转换: {value}g"), "⚖️"),
            "mg" => push(format!("{:.3}g | {:.6}kg", value / 1000.0, value / 1_000_000.0), format!("重量转换: {value}mg"), "⚖️"),
            "lb" => push(format!("{:.3}kg | {:.0}g", value * 0.453592, value * 453.592), format!("重量转换: {value}lb"), "⚖️"),
            // 温度单位
            "c" => push(format!("{:.2}°F | {:.2}K", value * 1.8 + 32.0, value + 273.15), format!("温度转换: {value}°C"), "🌡️"),
            "f" => push(format!("{:.2}°C | {:.2}K", (value - 32.0) / 1.8, (value - 32.0) / 1.8 + 273.15), format!("温度转换: {value}°F"), "🌡️"),
            "k" => push(format!("{:.2}°C | {:.2}°F", value - 273.15, (value - 273.15) * 1.8 + 32.0), format!("温度转换: {value}K"), "🌡️"),
            // 存储单位
            "b" => push(format!("{:.2}KB | {:.3}MB | {:.4}GB", value / 1024.0, value / 1024.0 / 1024.0, value / 1024.0 / 1024.0 / 1024.0), format!("存储转换: {value}B"), "💾"),
            "kb" => push(format!("{:.0}B | {:.3}MB | {:.4}GB", value * 1024.0, value / 1024.0, value / 1024.0 / 1024.0), format!("存储转换: {value}KB"), "💾"),
            "mb" => push(format!("{:.0}KB | {:.3}GB | {:.0}B", value * 1024.0, value / 1024.0, value * 1024.0 * 1024.0), format!("存储转换: {value}MB"), "💾"),
            "gb" => push(format!("{:.0}MB | {:.3}TB | {:.0}KB", value * 1024.0, value / 1024.0, value * 1024.0 * 1024.0), format!("存储转换: {value}GB"), "💾"),
            "tb" => push(format!("{:.2}GB | {:.0}MB", value * 1024.0, value * 1024.0 * 1024.0), format!("存储转换: {value}TB"), "💾"),
            _ => {}
        }

        if results.is_empty() { None } else { Some(results) }
    }
}

impl Plugin for CalculatorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }

        let mut results = Vec::new();

        // 1. 进制转换
        if let Some(result) = self.convert_base(query) {
            results.push(result);
        }

        // 2. 单位转换
        if let Some(mut unit_results) = self.convert_unit(query) {
            results.append(&mut unit_results);
        }

        // 3. 数学表达式
        if Self::is_expr_chars(query) {
            if let Ok(result) = self.calculate(query) {
                let result_str = Self::format_number(result);
                results.push(
                    QueryResult::new(result_str.clone(), result_str.clone())
                        .with_subtitle(format!("{query} = {result_str}"))
                        .with_icon("🧮")
                        .with_score(1000)
                        .with_action(PluginAction::default_action("copy", "复制")),
                );
            }
        }

        Ok(results)
    }

    fn execute(&self, result_id: &str, action_id: &str) -> Result<ExecuteOutcome> {
        if action_id == "copy" {
            // 剪贴板权限检查（事件落审计管道）；拒绝时 Err 上抛，Launcher 不执行复制
            self.sandbox.check_permission(&self.metadata.id, &PluginPermission::ClipboardAccess)?;
            return Ok(ExecuteOutcome::Copy(result_id.to_string()));
        }
        Err(anyhow!("Unknown action: {action_id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLogger;
    use crate::plugin::QueryContext;
    use crate::plugin::sandbox::{PluginPermission, SandboxConfig, SecurityLevel};

    fn plugin() -> CalculatorPlugin {
        CalculatorPlugin::new(Arc::new(SandboxManager::new(Arc::new(parking_lot::Mutex::new(
            AuditLogger::in_memory(50),
        )))))
    }

    #[test]
    fn expr_evaluation() {
        let p = plugin();
        assert_eq!(p.calculate("1+1").unwrap(), 2.0);
        assert_eq!(p.calculate("2*3+4").unwrap(), 10.0);
        assert_eq!(p.calculate("3*(4)").unwrap(), 12.0);
        assert_eq!(p.calculate("10/4").unwrap(), 2.5);
        assert!(p.calculate("1/0").is_err());
        // 括号包裹整个表达式：Tauri 原版同样报错（split 先于括号归约），保持语义一致
        assert!(p.calculate("(1+2)*3").is_err());
    }

    #[test]
    fn expr_char_class() {
        assert!(CalculatorPlugin::is_expr_chars("1+2*3"));
        assert!(CalculatorPlugin::is_expr_chars("(3.5-1)/2"));
        assert!(!CalculatorPlugin::is_expr_chars("abc"));
        assert!(!CalculatorPlugin::is_expr_chars(""));
    }

    #[test]
    fn base_conversion() {
        let p = plugin();
        let ctx = QueryContext::new("0xFF");
        let rs = p.query(&ctx).unwrap();
        assert_eq!(rs.len(), 1);
        assert!(rs[0].title.contains("十进制: 255"));
        assert!(rs[0].title.contains("0b11111111"));
        assert_eq!(rs[0].score, 900);

        assert!(p.query(&QueryContext::new("0b1010")).unwrap()[0].title.contains("十进制: 10"));
        assert!(p.query(&QueryContext::new("0o17")).unwrap()[0].title.contains("十进制: 15"));
    }

    #[test]
    fn unit_conversion() {
        let p = plugin();
        let rs = p.query(&QueryContext::new("100cm")).unwrap();
        assert_eq!(rs.len(), 1);
        assert!(rs[0].title.contains("1.000m"));
        assert!(rs[0].title.contains("1000mm"));

        let rs = p.query(&QueryContext::new("32f")).unwrap();
        assert!(rs[0].title.contains("0.00°C"));
    }

    #[test]
    fn expression_result_formatting() {
        let p = plugin();
        let rs = p.query(&QueryContext::new("1+1")).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].title, "2");
        assert_eq!(rs[0].subtitle, "1+1 = 2");
        assert_eq!(rs[0].score, 1000);

        let rs = p.query(&QueryContext::new("10/3")).unwrap();
        assert_eq!(rs[0].title, "3.333333");
    }

    #[test]
    fn empty_and_garbage_query() {
        let p = plugin();
        assert!(p.query(&QueryContext::new("")).unwrap().is_empty());
        assert!(p.query(&QueryContext::new("hello world")).unwrap().is_empty());
    }

    #[test]
    fn copy_action_checks_clipboard_permission() {
        // 沙盒表由 manager 注册；单插件场景下未注册 → 权限检查报错（不静默复制）
        let sandbox = Arc::new(SandboxManager::default());
        sandbox.register(SandboxConfig {
            plugin_id: "calculator".to_string(),
            security_level: SecurityLevel::Restricted,
            custom_permissions: Some([PluginPermission::ClipboardAccess].into_iter().collect()),
            enabled: true,
            timeout_ms: None,
            max_memory_mb: None,
        });
        let p = CalculatorPlugin::new(sandbox);
        assert_eq!(p.execute("42", "copy").unwrap(), ExecuteOutcome::Copy("42".to_string()));
        assert!(p.execute("42", "open").is_err());
    }
}
