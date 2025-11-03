// 计算器插件

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;

pub struct CalculatorPlugin {
    metadata: PluginMetadata,
    expr_regex: Regex,
}

impl CalculatorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "calculator".to_string(),
                name: "Calculator".to_string(),
                description: "Basic calculator".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("🧮"),
                trigger_keywords: vec![],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            // 匹配数学表达式：数字、运算符、括号、小数点
            expr_regex: Regex::new(r"^[\d+\-*/().\s]+$").unwrap(),
        }
    }
    
    /// 计算表达式
    fn calculate(&self, expr: &str) -> Result<f64> {
        // 简单的表达式解析器（支持 +、-、*、/、括号）
        let expr = expr.replace(" ", "");
        self.eval_expr(&expr)
    }
    
    fn eval_expr(&self, expr: &str) -> Result<f64> {
        // 处理加减
        let parts: Vec<&str> = expr.split(&['+', '-'][..]).collect();
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
        let parts: Vec<&str> = term.split(&['*', '/'][..]).collect();
        let ops: Vec<char> = term.chars().filter(|c| *c == '*' || *c == '/').collect();
        
        if parts.len() > 1 {
            let mut result = self.eval_factor(parts[0])?;
            for (i, part) in parts.iter().enumerate().skip(1) {
                let val = self.eval_factor(part)?;
                match ops.get(i - 1) {
                    Some('*') => result *= val,
                    Some('/') => {
                        if val == 0.0 {
                            return Err(anyhow::anyhow!("Division by zero"));
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
        if factor.starts_with('(') && factor.ends_with(')') {
            return self.eval_expr(&factor[1..factor.len()-1]);
        }
        
        // 解析数字
        factor.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Invalid number: {}", e))
    }
}

#[async_trait]
impl Plugin for CalculatorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        // 检查是否是数学表达式
        if !self.expr_regex.is_match(query) || query.is_empty() {
            return Ok(vec![]);
        }
        
        // 计算结果
        match self.calculate(query) {
            Ok(result) => {
                let result_str = if result.fract() == 0.0 {
                    format!("{}", result as i64)
                } else {
                    format!("{:.6}", result).trim_end_matches('0').trim_end_matches('.').to_string()
                };
                
                Ok(vec![
                    QueryResult::new(result_str.clone())
                        .with_subtitle(format!("{} = {}", query, result_str))
                        .with_icon(WoxImage::emoji("🧮"))
                        .with_score(100)
                        .with_action(
                            Action::new("copy")
                                .default()
                        )
                ])
            }
            Err(_) => Ok(vec![]),
        }
    }
    
    async fn execute(&self, result_id: &str, action_id: &str) -> Result<()> {
        if action_id == "copy" {
            // 复制到剪贴板（后续实现）
            tracing::info!("Copy result: {}", result_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Unknown action"))
        }
    }
}
