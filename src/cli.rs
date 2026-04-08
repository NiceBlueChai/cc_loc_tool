use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::export::{ExportFormat, export_results};
use crate::history::{compare_with_snapshot, create_snapshot, load_snapshot, save_snapshot};
use crate::loc::{Language, LocSummary, scan_directory_simple, scan_directory_with_complexity};

/// CLI 配置选项
#[derive(Debug)]
pub struct CliOptions {
    pub directory: PathBuf,
    pub exclude_dirs: Vec<String>,
    pub exclude_files: Vec<String>,
    pub languages: Vec<Language>,
    pub custom_extensions: Vec<String>,
    pub export_path: Option<PathBuf>,
    pub export_format: Option<ExportFormat>,
    pub save_snapshot_path: Option<PathBuf>,
    pub compare_with_path: Option<PathBuf>,
    /// 是否启用复杂度分析
    pub analyze_complexity: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            directory: PathBuf::new(),
            exclude_dirs: Vec::new(),
            exclude_files: Vec::new(),
            languages: Language::all().to_vec(),
            custom_extensions: Vec::new(),
            export_path: None,
            export_format: None,
            save_snapshot_path: None,
            compare_with_path: None,
            analyze_complexity: false,
        }
    }
}

/// 解析命令行参数
pub fn parse_args() -> Result<CliOptions> {
    parse_args_from(std::env::args().skip(1))
}

/// 从指定参数迭代器解析命令行参数，便于测试
pub fn parse_args_from<I, S>(args: I) -> Result<CliOptions>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut options = CliOptions::default();
    let mut args = args.into_iter().map(Into::into).peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-d" | "--directory" => {
                if let Some(dir) = args.next() {
                    options.directory = PathBuf::from(dir);
                } else {
                    anyhow::bail!("--directory 选项需要一个路径参数");
                }
            }
            "-e" | "--exclude-dirs" => {
                if let Some(exclude) = args.next() {
                    options.exclude_dirs = exclude
                        .split([',', ';'])
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else {
                    anyhow::bail!("--exclude-dirs 选项需要一个目录列表参数");
                }
            }
            "-f" | "--exclude-files" => {
                if let Some(exclude) = args.next() {
                    options.exclude_files = exclude
                        .split([',', ';'])
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else {
                    anyhow::bail!("--exclude-files 选项需要一个文件列表参数");
                }
            }
            "-l" | "--languages" => {
                if let Some(langs) = args.next() {
                    options.languages = langs
                        .split([',', ';'])
                        .filter_map(|s| {
                            let s = s.trim().to_lowercase();
                            Language::all()
                                .iter()
                                .find(|l| {
                                    l.display_name().to_lowercase() == s
                                        || l.display_name().replace("+", "pp").to_lowercase() == s
                                })
                                .copied()
                        })
                        .collect();

                    if options.languages.is_empty() {
                        anyhow::bail!("没有找到有效的编程语言");
                    }
                } else {
                    anyhow::bail!("--languages 选项需要一个语言列表参数");
                }
            }
            "-x" | "--extensions" => {
                if let Some(exts) = args.next() {
                    options.custom_extensions = exts
                        .split([',', ';'])
                        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else {
                    anyhow::bail!("--extensions 选项需要一个扩展名列表参数");
                }
            }
            "-o" | "--output" => {
                if let Some(output) = args.next() {
                    options.export_path = Some(PathBuf::from(output));
                } else {
                    anyhow::bail!("--output 选项需要一个路径参数");
                }
            }
            "-t" | "--format" => {
                if let Some(format) = args.next() {
                    let format = format.trim().to_lowercase();
                    options.export_format = ExportFormat::all()
                        .iter()
                        .find(|f| f.name().to_lowercase() == format || f.extension() == format)
                        .copied();

                    if options.export_format.is_none() {
                        anyhow::bail!("不支持的导出格式: {}", format);
                    }
                } else {
                    anyhow::bail!("--format 选项需要一个格式参数");
                }
            }
            "--save-snapshot" => {
                if let Some(path) = args.next() {
                    options.save_snapshot_path = Some(PathBuf::from(path));
                } else {
                    anyhow::bail!("--save-snapshot 选项需要一个文件路径参数");
                }
            }
            "--compare-with" => {
                if let Some(path) = args.next() {
                    options.compare_with_path = Some(PathBuf::from(path));
                } else {
                    anyhow::bail!("--compare-with 选项需要一个快照路径参数");
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-v" | "--version" => {
                println!("cc_loc_tool v0.1.0");
                std::process::exit(0);
            }
            "-c" | "--complexity" => {
                options.analyze_complexity = true;
            }
            _ => {
                if options.directory.as_os_str().is_empty() {
                    options.directory = PathBuf::from(arg);
                } else {
                    anyhow::bail!("未知参数: {}", arg);
                }
            }
        }
    }

    // 确保指定了目录
    if options.directory.as_os_str().is_empty() {
        print_help();
        anyhow::bail!("必须指定要扫描的目录");
    }

    // 确保目录存在
    if !options.directory.exists() {
        anyhow::bail!("指定的目录不存在: {:?}", options.directory);
    }

    Ok(options)
}

/// 打印帮助信息
fn print_help() {
    println!("C/C++ 代码行统计工具 (cc_loc_cli) v0.1.0");
    println!("用法: cc_loc_cli [OPTIONS] DIRECTORY");
    println!();
    println!("选项:");
    println!("  -d, --directory DIRECTORY    要扫描的目录路径");
    println!("  -e, --exclude-dirs DIRS      要排除的目录列表，用逗号或分号分隔");
    println!("  -f, --exclude-files FILES    要排除的文件模式，用逗号或分号分隔");
    println!("  -l, --languages LANGS        要扫描的编程语言，用逗号或分号分隔");
    println!("                               支持的语言: C, C++, Java, Python, Go, Rust");
    println!("  -x, --extensions EXTS        自定义扫描后缀（如: tpp,ipp,cu）");
    println!("  -c, --complexity            启用代码复杂度分析");
    println!("      --save-snapshot PATH    将当前扫描结果保存为快照（JSON）");
    println!("      --compare-with PATH     与指定快照进行对比并输出差异摘要");
    println!("  -o, --output PATH            导出结果的文件路径");
    println!("  -t, --format FORMAT          导出格式: csv, json, html");
    println!("  -h, --help                   显示帮助信息");
    println!("  -v, --version                显示版本信息");
    println!();
    println!("示例:");
    println!("  cc_loc_cli ./my_project");
    println!("  cc_loc_cli -d ./my_project -e build,target -l C++,Java");
    println!("  cc_loc_cli ./my_project -x tpp,ipp,cu");
    println!("  cc_loc_cli ./my_project -o results.json -t json");
    println!("  cc_loc_cli ./my_project --save-snapshot snap_v1.json");
    println!("  cc_loc_cli ./my_project --compare-with snap_v1.json");
    println!("  cc_loc_cli ./my_project -c -o complexity.html");
}

/// 运行 CLI 模式
pub fn run_cli() -> Result<()> {
    let options = parse_args()?;

    println!("正在扫描目录: {:?}", options.directory);
    println!("排除目录: {:?}", options.exclude_dirs);
    println!("排除文件: {:?}", options.exclude_files);
    println!(
        "扫描语言: {:?}",
        options
            .languages
            .iter()
            .map(|l| l.display_name())
            .collect::<Vec<_>>()
    );
    println!("自定义后缀: {:?}", options.custom_extensions);
    println!(
        "复杂度分析: {}",
        if options.analyze_complexity {
            "启用"
        } else {
            "禁用"
        }
    );
    println!("快照保存路径: {:?}", options.save_snapshot_path);
    println!("对比快照路径: {:?}", options.compare_with_path);
    println!();

    // 转换排除目录为 HashSet
    let exclude_dirs: HashSet<String> = options.exclude_dirs.into_iter().collect();

    let results;
    let summary: LocSummary;

    if options.analyze_complexity {
        // 使用带复杂度分析的扫描
        results = scan_directory_with_complexity(
            &options.directory,
            &exclude_dirs,
            &options.exclude_files,
            &options.languages,
            &options.custom_extensions,
            None,
        )?;
        summary = LocSummary::from_files_with_complexity(&results);
    } else {
        // 使用简单扫描
        results = scan_directory_simple(
            &options.directory,
            &exclude_dirs,
            &options.exclude_files,
            &options.languages,
            &options.custom_extensions,
        )?;
        summary = LocSummary::from_files(&results);
    }

    if let Some(compare_path) = &options.compare_with_path {
        let baseline = load_snapshot(compare_path)?;
        let comparison =
            compare_with_snapshot(Some(&options.directory), &summary, &results, &baseline);

        println!();
        println!("=== 历史对比结果 ===");
        println!("基线快照: {:?}", compare_path);
        println!("快照时间: {}", baseline.created_at);
        println!("新增文件: {}", comparison.added_files);
        println!("删除文件: {}", comparison.removed_files);
        println!("变更文件: {}", comparison.changed_files);
        println!("未变更文件: {}", comparison.unchanged_files);
        println!(
            "行数变化: 总计 {:+}, 代码 {:+}, 注释 {:+}, 空白 {:+}",
            comparison.total_delta,
            comparison.code_delta,
            comparison.comments_delta,
            comparison.blanks_delta
        );
    }

    // 打印结果
    println!("扫描完成，共找到 {} 个文件:", summary.files);
    println!("代码行: {}", summary.code);
    println!("注释行: {}", summary.comments);
    println!("空白行: {}", summary.blanks);
    println!("总行数: {}", summary.total());

    // 如果启用了复杂度分析，打印复杂度统计
    if options.analyze_complexity
        && let Some(ref c) = summary.complexity
    {
        println!();
        println!("=== 复杂度分析 ===");
        println!("平均圈复杂度: {:.1}", c.avg_cyclomatic);
        println!("函数总数: {}", c.total_functions);
        println!("高复杂度函数(>10): {}", c.high_complexity_functions);
        println!("长函数(>50行): {}", c.long_functions);
        println!("平均函数长度: {:.1}", c.avg_function_length);
    }
    println!();

    // 如果需要导出
    if let Some(export_path) = options.export_path {
        let format = options.export_format.unwrap_or(ExportFormat::Csv);
        println!(
            "正在导出结果到: {:?} (格式: {})",
            export_path,
            format.name()
        );

        export_results(&export_path, format, &summary, &results)?;
        println!("导出成功!");
    }

    if let Some(snapshot_path) = options.save_snapshot_path {
        let snapshot = create_snapshot(Some(&options.directory), &summary, &results);
        save_snapshot(&snapshot_path, &snapshot)?;
        println!("已保存快照到: {:?}", snapshot_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cc_loc_tool_cli_test_{}", timestamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_args_supports_snapshot_and_extension_flags() {
        let root = make_temp_dir();
        let args = vec![
            "-d".to_string(),
            root.to_string_lossy().to_string(),
            "-e".to_string(),
            "build,target".to_string(),
            "-f".to_string(),
            "moc_*,*.generated.cpp".to_string(),
            "-l".to_string(),
            "C++,Python".to_string(),
            "-x".to_string(),
            "tpp,ipp,.cu".to_string(),
            "--save-snapshot".to_string(),
            "snap.json".to_string(),
            "--compare-with".to_string(),
            "base.json".to_string(),
            "-c".to_string(),
        ];

        let options = parse_args_from(args).unwrap();
        assert_eq!(options.directory, root);
        assert_eq!(
            options.exclude_dirs,
            vec!["build".to_string(), "target".to_string()]
        );
        assert_eq!(
            options.exclude_files,
            vec!["moc_*".to_string(), "*.generated.cpp".to_string()]
        );
        assert_eq!(options.languages, vec![Language::Cpp, Language::Python]);
        assert_eq!(
            options.custom_extensions,
            vec!["tpp".to_string(), "ipp".to_string(), "cu".to_string()]
        );
        assert_eq!(options.save_snapshot_path, Some(PathBuf::from("snap.json")));
        assert_eq!(options.compare_with_path, Some(PathBuf::from("base.json")));
        assert!(options.analyze_complexity);

        let _ = fs::remove_dir_all(root);
    }
}
