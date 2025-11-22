// 计算器插件 - 增强版
// 支持: 数学表达式、进制转换、单位转换

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;

pub struct CalculatorPlugin {
    metadata: PluginMetadata,
    expr_regex: Regex,
    hex_regex: Regex,
    bin_regex: Regex,
    oct_regex: Regex,
    unit_regex: Regex,
}

impl CalculatorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "calculator".to_string(),
                name: "Calculator".to_string(),
                description: "数学计算、进制转换、单位转换".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("🧮"),
                trigger_keywords: vec!["=".to_string(), "calc".to_string()],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
            // 匹配数学表达式：数字、运算符、括号、小数点
            expr_regex: Regex::new(r"^[\d+\-*/().\s]+$").unwrap(),
            // 匹配十六进制: 0x 或 0X 开头
            hex_regex: Regex::new(r"^0[xX][0-9a-fA-F]+$").unwrap(),
            // 匹配二进制: 0b 或 0B 开头
            bin_regex: Regex::new(r"^0[bB][01]+$").unwrap(),
            // 匹配八进制: 0o 或 0O 开头
            oct_regex: Regex::new(r"^0[oO][0-7]+$").unwrap(),
            // 匹配单位转换: 数字+单位
            unit_regex: Regex::new(r"^([\d.]+)\s*([a-zA-Z]+)$").unwrap(),
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
    
    /// 进制转换
    fn convert_base(&self, input: &str) -> Option<QueryResult> {
        let input_lower = input.to_lowercase();
        
        // 检测进制并转换
        let (num, base_name) = if self.hex_regex.is_match(input) {
            let hex_str = &input_lower[2..];
            let num = i64::from_str_radix(hex_str, 16).ok()?;
            (num, "十六进制")
        } else if self.bin_regex.is_match(input) {
            let bin_str = &input_lower[2..];
            let num = i64::from_str_radix(bin_str, 2).ok()?;
            (num, "二进制")
        } else if self.oct_regex.is_match(input) {
            let oct_str = &input_lower[2..];
            let num = i64::from_str_radix(oct_str, 8).ok()?;
            (num, "八进制")
        } else {
            return None;
        };
        
        // 生成所有进制的表示
        let conversions = format!(
            "十进制: {} | 十六进制: 0x{:X} | 二进制: 0b{:b} | 八进制: 0o{:o}",
            num, num, num, num
        );
        
        Some(
            QueryResult::new(conversions.clone())
                .with_subtitle(format!("{} 进制转换", base_name))
                .with_icon(WoxImage::emoji("🔢"))
                .with_score(900)
                .with_action(Action::new("copy").default())
        )
    }
    
    /// 单位转换
    fn convert_unit(&self, input: &str) -> Option<Vec<QueryResult>> {
        let caps = self.unit_regex.captures(input)?;
        let value: f64 = caps.get(1)?.as_str().parse().ok()?;
        let unit = caps.get(2)?.as_str().to_lowercase();
        
        let mut results = Vec::new();
        
        // 长度单位
        if matches!(unit.as_str(), "m" | "km" | "cm" | "mm") {
            let conversions = match unit.as_str() {
                "m" => format!("{:.3}km | {:.0}cm | {:.0}mm | {:.2}ft", 
                    value/1000.0, value*100.0, value*1000.0, value*3.28084),
                "km" => format!("{:.0}m | {:.2}mi", 
                    value*1000.0, value*0.621371),
                "cm" => format!("{:.3}m | {:.0}mm | {:.2}in", 
                    value/100.0, value*10.0, value*0.393701),
                "mm" => format!("{:.3}m | {:.2}cm", 
                    value/1000.0, value/10.0),
                _ => return None,
            };
            
            results.push(
                QueryResult::new(conversions)
                    .with_subtitle(format!("长度转换: {}{}", value, unit))
                    .with_icon(WoxImage::emoji("📏"))
                    .with_score(850)
                    .with_action(Action::new("copy").default())
            );
        }
        
        // 重量单位
        if matches!(unit.as_str(), "kg" | "g" | "mg" | "lb") {
            let conversions = match unit.as_str() {
                "kg" => format!("{:.0}g | {:.2}lb | {:.2}oz", 
                    value*1000.0, value*2.20462, value*35.274),
                "g" => format!("{:.3}kg | {:.0}mg", 
                    value/1000.0, value*1000.0),
                "mg" => format!("{:.3}g | {:.6}kg", 
                    value/1000.0, value/1_000_000.0),
                "lb" => format!("{:.3}kg | {:.0}g", 
                    value*0.453592, value*453.592),
                _ => return None,
            };
            
            results.push(
                QueryResult::new(conversions)
                    .with_subtitle(format!("重量转换: {}{}", value, unit))
                    .with_icon(WoxImage::emoji("⚖️"))
                    .with_score(850)
                    .with_action(Action::new("copy").default())
            );
        }
        
        // 温度单位
        if matches!(unit.as_str(), "c" | "f" | "k") {
            let conversions = match unit.as_str() {
                "c" => format!("{:.2}°F | {:.2}K", 
                    value*1.8+32.0, value+273.15),
                "f" => format!("{:.2}°C | {:.2}K", 
                    (value-32.0)/1.8, (value-32.0)/1.8+273.15),
                "k" => format!("{:.2}°C | {:.2}°F", 
                    value-273.15, (value-273.15)*1.8+32.0),
                _ => return None,
            };
            
            results.push(
                QueryResult::new(conversions)
                    .with_subtitle(format!("温度转换: {}{}", value, unit.to_uppercase()))
                    .with_icon(WoxImage::emoji("🌡️"))
                    .with_score(850)
                    .with_action(Action::new("copy").default())
            );
        }
        
        // 存储单位
        if matches!(unit.as_str(), "b" | "kb" | "mb" | "gb" | "tb") {
            let conversions = match unit.as_str() {
                "b" => format!("{:.2}KB | {:.3}MB | {:.4}GB", 
                    value/1024.0, value/1024.0/1024.0, value/1024.0/1024.0/1024.0),
                "kb" => format!("{:.0}B | {:.3}MB | {:.4}GB", 
                    value*1024.0, value/1024.0, value/1024.0/1024.0),
                "mb" => format!("{:.0}KB | {:.3}GB | {:.0}B", 
                    value*1024.0, value/1024.0, value*1024.0*1024.0),
                "gb" => format!("{:.0}MB | {:.3}TB | {:.0}KB", 
                    value*1024.0, value/1024.0, value*1024.0*1024.0),
                "tb" => format!("{:.2}GB | {:.0}MB", 
                    value*1024.0, value*1024.0*1024.0),
                _ => return None,
            };
            
            results.push(
                QueryResult::new(conversions)
                    .with_subtitle(format!("存储转换: {}{}", value, unit.to_uppercase()))
                    .with_icon(WoxImage::emoji("💾"))
                    .with_score(850)
                    .with_action(Action::new("copy").default())
            );
        }
        
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }
}

#[async_trait]
impl Plugin for CalculatorPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let query = ctx.search.trim();
        
        if query.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results = Vec::new();
        
        // 1. 尝试进制转换
        if let Some(result) = self.convert_base(query) {
            results.push(result);
        }
        
        // 2. 尝试单位转换
        if let Some(mut unit_results) = self.convert_unit(query) {
            results.append(&mut unit_results);
        }
        
        // 3. 尝试数学表达式计算
        if self.expr_regex.is_match(query) {
            match self.calculate(query) {
                Ok(result) => {
                    let result_str = if result.fract() == 0.0 {
                        format!("{}", result as i64)
                    } else {
                        format!("{:.6}", result).trim_end_matches('0').trim_end_matches('.').to_string()
                    };
                    
                    results.push(
                        QueryResult::new(result_str.clone())
                            .with_subtitle(format!("{} = {}", query, result_str))
                            .with_icon(WoxImage::emoji("🧮"))
                            .with_score(1000)
                            .with_action(Action::new("copy").default())
                    );
                }
                Err(_) => {}
            }
        }
        
        Ok(results)
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
