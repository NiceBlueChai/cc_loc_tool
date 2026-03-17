use anyhow::Result;
use html_escape::encode_text;
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::loc::{FileLoc, LocSummary};

/// 导出格式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportFormat {
    Csv,
    Json,
    Html,
}

impl ExportFormat {
    /// 获取文件扩展名
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Html => "html",
        }
    }
    
    /// 获取格式名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Html => "HTML",
        }
    }
    
    /// 获取所有支持的格式
    pub fn all() -> &'static [Self] {
        &[Self::Csv, Self::Json, Self::Html]
    }
}

/// JSON 导出结构
#[derive(Serialize)]
pub struct ExportData {
    pub summary: LocSummary,
    pub files: Vec<FileLoc>,
    pub export_time: String,
}

/// 导出统计结果到指定格式
pub fn export_results(
    path: &Path,
    format: ExportFormat,
    summary: &LocSummary,
    files: &[FileLoc],
) -> Result<()> {
    match format {
        ExportFormat::Csv => export_to_csv(path, summary, files),
        ExportFormat::Json => export_to_json(path, summary, files),
        ExportFormat::Html => export_to_html(path, summary, files),
    }
}

/// 导出为 CSV 格式
fn export_to_csv(path: &Path, summary: &LocSummary, files: &[FileLoc]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new().from_path(path)?;
    
    // 写入文件统计数据
    writer.write_record(&["文件路径", "代码行", "注释行", "空白行", "总行数"])?;
    
    for file in files {
        let path_str = file.path.to_string_lossy().to_string();
        writer.write_record(&[
            path_str,
            file.code.to_string(),
            file.comments.to_string(),
            file.blanks.to_string(),
            file.total().to_string(),
        ])?;
    }
    
    // 写入总计
    writer.write_record(&["总计", "", "", "", ""])?;
    
    // 为数值创建临时字符串
    let code_str = summary.code.to_string();
    let comments_str = summary.comments.to_string();
    let blanks_str = summary.blanks.to_string();
    let total_str = summary.total().to_string();
    
    writer.write_record(&[
        "",
        &code_str,
        &comments_str,
        &blanks_str,
        &total_str,
    ])?;
    
    writer.flush()?;
    Ok(())
}

/// 导出为 JSON 格式
fn export_to_json(path: &Path, summary: &LocSummary, files: &[FileLoc]) -> Result<()> {
    let export_data = ExportData {
        summary: summary.clone(),
        files: files.to_vec(),
        export_time: chrono::Local::now().to_string(),
    };
    
    let json_content = serde_json::to_string_pretty(&export_data)?;
    fs::write(path, json_content)?;
    Ok(())
}

/// 导出为 HTML 格式
fn export_to_html(path: &Path, summary: &LocSummary, files: &[FileLoc]) -> Result<()> {
    let html_content = generate_html_content(summary, files);
    fs::write(path, html_content)?;
    Ok(())
}

/// 生成 HTML 内容
fn generate_html_content(summary: &LocSummary, files: &[FileLoc]) -> String {
    let export_time = chrono::Local::now().to_string();
    
    // 提取统计数据到单独变量
    let files_count = summary.files;
    let code_lines = summary.code;
    let comments_lines = summary.comments;
    let blanks_lines = summary.blanks;
    let total_lines = summary.total();
    let file_rows = generate_file_rows(files);
    
    format!(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>代码行统计报告</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        h1 {{
            text-align: center;
            color: #2c3e50;
            margin-bottom: 30px;
        }}
        .summary-container {{
            display: flex;
            justify-content: space-around;
            margin-bottom: 40px;
            flex-wrap: wrap;
            gap: 20px;
        }}
        .summary-card {{
            background-color: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
            text-align: center;
            min-width: 150px;
        }}
        .summary-card h3 {{
            margin: 0 0 10px 0;
            color: #7f8c8d;
            font-size: 14px;
            text-transform: uppercase;
            letter-spacing: 1px;
        }}
        .summary-card .value {{
            font-size: 24px;
            font-weight: bold;
            color: #2c3e50;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            background-color: white;
            box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
            border-radius: 8px;
            overflow: hidden;
        }}
        th, td {{
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #eee;
        }}
        th {{
            background-color: #3498db;
            color: white;
            font-weight: 600;
        }}
        tr:hover {{
            background-color: #f9f9f9;
        }}
        tr:last-child td {{
            border-bottom: none;
        }}
        .total-row {{
            background-color: #ecf0f1;
            font-weight: bold;
        }}
        .footer {{
            text-align: center;
            margin-top: 40px;
            color: #7f8c8d;
            font-size: 12px;
        }}
    </style>
</head>
<body>
    <h1>代码行统计报告</h1>
    
    <div class="summary-container">
        <div class="summary-card">
            <h3>文件数量</h3>
            <div class="value">{}</div>
        </div>
        <div class="summary-card">
            <h3>代码行数</h3>
            <div class="value">{}</div>
        </div>
        <div class="summary-card">
            <h3>注释行数</h3>
            <div class="value">{}</div>
        </div>
        <div class="summary-card">
            <h3>空白行数</h3>
            <div class="value">{}</div>
        </div>
        <div class="summary-card">
            <h3>总行数</h3>
            <div class="value">{}</div>
        </div>
    </div>
    
    <table>
        <thead>
            <tr>
                <th>文件路径</th>
                <th>代码行</th>
                <th>注释行</th>
                <th>空白行</th>
                <th>总行数</th>
            </tr>
        </thead>
        <tbody>
            {}
            <tr class="total-row">
                <td><strong>总计</strong></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>
        </tbody>
    </table>
    
    <div class="footer">
        导出时间: {}
    </div>
</body>
</html>"#, 
        files_count,
        code_lines,
        comments_lines,
        blanks_lines,
        total_lines,
        file_rows,
        code_lines,
        comments_lines,
        blanks_lines,
        total_lines,
        export_time
    )
}

/// 生成文件行的 HTML 内容
fn generate_file_rows(files: &[FileLoc]) -> String {
    files.iter()
        .map(|file| {
            let path_str = encode_text(&file.path.to_string_lossy()).to_string();
            format!(r#"<tr>
                <td>{path}</td>
                <td>{code}</td>
                <td>{comments}</td>
                <td>{blanks}</td>
                <td>{total}</td>
            </tr>"#, 
                path = path_str,
                code = file.code,
                comments = file.comments,
                blanks = file.blanks,
                total = file.total()
            )
        })
        .collect::<Vec<String>>()
        .join("")
}
