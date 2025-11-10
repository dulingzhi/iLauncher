// 单位转换插件

use crate::core::types::*;
use crate::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;

pub struct UnitConverterPlugin {
    metadata: PluginMetadata,
}

impl UnitConverterPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "unit_converter".to_string(),
                name: "Unit Converter".to_string(),
                description: "Convert between different units".to_string(),
                author: "iLauncher".to_string(),
                version: "1.0.0".to_string(),
                icon: WoxImage::emoji("📏"),
                trigger_keywords: vec![],
                commands: vec![],
                settings: vec![],
                supported_os: vec!["windows".to_string(), "macos".to_string(), "linux".to_string()],
                plugin_type: PluginType::Native,
            },
        }
    }

    fn parse_conversion(input: &str) -> Option<(f64, String, String)> {
        // 匹配格式: "10 km to miles" 或 "100 usd to cny"
        let re = Regex::new(r"^(\d+\.?\d*)\s*([a-z]+)\s+to\s+([a-z]+)$").ok()?;
        
        if let Some(caps) = re.captures(input.to_lowercase().as_str()) {
            let value: f64 = caps[1].parse().ok()?;
            let from_unit = caps[2].to_string();
            let to_unit = caps[3].to_string();
            
            return Some((value, from_unit, to_unit));
        }
        
        None
    }

    fn convert(value: f64, from: &str, to: &str) -> Option<(f64, &'static str, &'static str)> {
        match (from, to) {
            // 长度转换
            ("km", "m") => Some((value * 1000.0, "kilometers", "meters")),
            ("km", "miles") | ("km", "mi") => Some((value * 0.621371, "kilometers", "miles")),
            ("m", "km") => Some((value / 1000.0, "meters", "kilometers")),
            ("m", "ft") | ("m", "feet") => Some((value * 3.28084, "meters", "feet")),
            ("miles", "km") | ("mi", "km") => Some((value * 1.60934, "miles", "kilometers")),
            ("ft", "m") | ("feet", "m") => Some((value / 3.28084, "feet", "meters")),
            ("cm", "inch") | ("cm", "in") => Some((value / 2.54, "centimeters", "inches")),
            ("inch", "cm") | ("in", "cm") => Some((value * 2.54, "inches", "centimeters")),

            // 重量转换
            ("kg", "lb") | ("kg", "lbs") => Some((value * 2.20462, "kilograms", "pounds")),
            ("lb", "kg") | ("lbs", "kg") => Some((value / 2.20462, "pounds", "kilograms")),
            ("g", "oz") => Some((value / 28.3495, "grams", "ounces")),
            ("oz", "g") => Some((value * 28.3495, "ounces", "grams")),

            // 温度转换
            ("c", "f") | ("celsius", "fahrenheit") => {
                Some((value * 9.0 / 5.0 + 32.0, "Celsius", "Fahrenheit"))
            }
            ("f", "c") | ("fahrenheit", "celsius") => {
                Some(((value - 32.0) * 5.0 / 9.0, "Fahrenheit", "Celsius"))
            }
            ("c", "k") | ("celsius", "kelvin") => Some((value + 273.15, "Celsius", "Kelvin")),
            ("k", "c") | ("kelvin", "celsius") => Some((value - 273.15, "Kelvin", "Celsius")),

            // 面积转换
            ("sqm", "sqft") => Some((value * 10.7639, "square meters", "square feet")),
            ("sqft", "sqm") => Some((value / 10.7639, "square feet", "square meters")),

            // 体积转换
            ("l", "gal") | ("liter", "gallon") => Some((value * 0.264172, "liters", "gallons")),
            ("gal", "l") | ("gallon", "liter") => Some((value * 3.78541, "gallons", "liters")),
            ("ml", "floz") => Some((value / 29.5735, "milliliters", "fluid ounces")),
            ("floz", "ml") => Some((value * 29.5735, "fluid ounces", "milliliters")),

            // 速度转换
            ("kmh", "mph") => Some((value * 0.621371, "km/h", "mph")),
            ("mph", "kmh") => Some((value * 1.60934, "mph", "km/h")),
            ("ms", "kmh") => Some((value * 3.6, "m/s", "km/h")),
            ("kmh", "ms") => Some((value / 3.6, "km/h", "m/s")),

            // 数据大小转换
            ("kb", "mb") => Some((value / 1024.0, "KB", "MB")),
            ("mb", "kb") => Some((value * 1024.0, "MB", "KB")),
            ("mb", "gb") => Some((value / 1024.0, "MB", "GB")),
            ("gb", "mb") => Some((value * 1024.0, "GB", "MB")),
            ("gb", "tb") => Some((value / 1024.0, "GB", "TB")),
            ("tb", "gb") => Some((value * 1024.0, "TB", "GB")),

            // 时间转换
            ("s", "min") | ("sec", "min") => Some((value / 60.0, "seconds", "minutes")),
            ("min", "s") | ("min", "sec") => Some((value * 60.0, "minutes", "seconds")),
            ("min", "h") | ("min", "hour") => Some((value / 60.0, "minutes", "hours")),
            ("h", "min") | ("hour", "min") => Some((value * 60.0, "hours", "minutes")),
            ("h", "day") | ("hour", "day") => Some((value / 24.0, "hours", "days")),
            ("day", "h") | ("day", "hour") => Some((value * 24.0, "days", "hours")),

            _ => None,
        }
    }

    fn format_number(num: f64) -> String {
        if num.abs() < 0.001 || num.abs() > 1_000_000.0 {
            format!("{:.6e}", num)
        } else if num.fract() == 0.0 {
            format!("{:.0}", num)
        } else {
            format!("{:.4}", num).trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }
}

#[async_trait]
impl Plugin for UnitConverterPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let search = ctx.search.trim();
        
        // 至少需要输入 "X unit to unit" 格式
        if !search.contains("to") {
            return Ok(Vec::new());
        }
        
        if let Some((value, from_unit, to_unit)) = Self::parse_conversion(search) {
            if let Some((result, from_name, to_name)) = Self::convert(value, &from_unit, &to_unit) {
                let formatted_result = Self::format_number(result);
                let formatted_value = Self::format_number(value);
                
                return Ok(vec![QueryResult {
                    id: formatted_result.clone(),
                    title: format!("{} {}", formatted_result, to_name),
                    subtitle: format!("{} {} = {} {}", formatted_value, from_name, formatted_result, to_name),
                    icon: WoxImage::emoji("🔄"),
                    preview: None,
                    score: 100,
                    context_data: serde_json::Value::Null,
                    group: Some("Unit Converter".to_string()),
                    plugin_id: self.metadata.id.clone(),
                    refreshable: false,
                    actions: vec![
                        Action {
                            id: "copy".to_string(),
                            name: "Copy Result".to_string(),
                            icon: None,
                            is_default: true,
                            prevent_hide: false,
                            hotkey: None,
                        },
                    ],
                }]);
            }
        }
        
        Ok(Vec::new())
    }
    
    async fn execute(&self, result_id: &str, _action_id: &str) -> Result<()> {
        // 复制结果到剪贴板
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard};
            use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
            use windows::Win32::Foundation::HWND;
            
            unsafe {
                OpenClipboard(HWND::default())?;
                EmptyClipboard()?;
                
                let text_bytes = result_id.as_bytes();
                let h_mem = GlobalAlloc(GMEM_MOVEABLE, text_bytes.len() + 1)?;
                let locked = GlobalLock(h_mem);
                
                if !locked.is_null() {
                    std::ptr::copy_nonoverlapping(
                        text_bytes.as_ptr(),
                        locked as *mut u8,
                        text_bytes.len(),
                    );
                    GlobalUnlock(h_mem)?;
                }
                
                CloseClipboard()?;
            }
        }
        
        tracing::info!("Copied conversion result: {}", result_id);
        Ok(())
    }
}
