use std::path::PathBuf;
use uuid::Uuid;

use crate::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Download {
        url: String,
        output: Option<PathBuf>,
        connections: Option<usize>,
        referer: Option<String>,
        checksum: Option<String>,
        headers: Vec<(String, String)>,
        cookies: Option<String>,
        speed_limit: Option<u64>,
    },
    Info {
        url: String,
        referer: Option<String>,
    },
    List,
    Resume {
        id: Uuid,
    },
    Refresh {
        id: Uuid,
        url: String,
        referer: Option<String>,
    },
    Limit {
        id: Uuid,
        bytes_per_sec: u64,
    },
    Delete {
        id: Uuid,
        delete_file: bool,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct CliArgs {
    pub command: CliCommand,
}

impl CliArgs {
    pub fn parse() -> Result<Option<Self>, CliError> {
        Self::parse_from(std::env::args().skip(1))
    }

    pub fn parse_from<I, T>(args: I) -> Result<Option<Self>, CliError>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let items: Vec<String> = args.into_iter().map(Into::into).collect();
        if items.is_empty() {
            Self::print_help();
            return Ok(None);
        }

        match items[0].as_str() {
            "-h" | "--help" => {
                Self::print_help();
                Ok(None)
            }
            "list" | "ls" => Ok(Some(Self {
                command: CliCommand::List,
            })),
            "info" => {
                let mut url = None;
                let mut referer = None;
                let mut iter = items[1..].iter();
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "-r" | "--referer" => {
                            let val = iter.next().ok_or_else(|| {
                                CliError::InvalidArgument("Missing value for --referer".to_string())
                            })?;
                            referer = Some(val.clone());
                        }
                        other if other.starts_with('-') => {
                            return Err(CliError::InvalidArgument(format!(
                                "Unknown option: {other}"
                            )));
                        }
                        positional => {
                            if url.is_none() {
                                url = Some(positional.to_string());
                            } else {
                                return Err(CliError::InvalidArgument(format!(
                                    "Unexpected argument: {positional}"
                                )));
                            }
                        }
                    }
                }
                let url = url.ok_or(CliError::MissingUrl)?;
                Ok(Some(Self {
                    command: CliCommand::Info { url, referer },
                }))
            }
            "resume" => {
                if items.len() < 2 {
                    return Err(CliError::InvalidArgument(
                        "Missing task ID to resume".to_string(),
                    ));
                }
                let id = Uuid::parse_str(&items[1])
                    .map_err(|e| CliError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
                Ok(Some(Self {
                    command: CliCommand::Resume { id },
                }))
            }
            "refresh" => {
                if items.len() < 3 {
                    return Err(CliError::InvalidArgument(
                        "Usage: refresh <TASK_ID> <NEW_URL> [--referer <URL>]".to_string(),
                    ));
                }
                let id = Uuid::parse_str(&items[1])
                    .map_err(|e| CliError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
                let mut new_url = None;
                let mut referer = None;
                let mut iter = items[2..].iter();
                while let Some(arg) = iter.next() {
                    match arg.as_str() {
                        "-r" | "--referer" => {
                            let val = iter.next().ok_or_else(|| {
                                CliError::InvalidArgument("Missing value for --referer".to_string())
                            })?;
                            referer = Some(val.clone());
                        }
                        other if other.starts_with('-') => {
                            return Err(CliError::InvalidArgument(format!(
                                "Unknown option: {other}"
                            )));
                        }
                        positional => {
                            if new_url.is_none() {
                                new_url = Some(positional.to_string());
                            } else {
                                return Err(CliError::InvalidArgument(format!(
                                    "Unexpected argument: {positional}"
                                )));
                            }
                        }
                    }
                }
                let url = new_url.ok_or(CliError::MissingUrl)?;
                Ok(Some(Self {
                    command: CliCommand::Refresh { id, url, referer },
                }))
            }
            "limit" => {
                if items.len() != 3 {
                    return Err(CliError::InvalidArgument(
                        "Usage: limit <TASK_ID> <BYTES_PER_SEC>".to_string(),
                    ));
                }
                let id = Uuid::parse_str(&items[1])
                    .map_err(|e| CliError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
                let bytes_per_sec: u64 = items[2].parse().map_err(|_| {
                    CliError::InvalidArgument(format!("Invalid speed limit: {}", items[2]))
                })?;
                Ok(Some(Self {
                    command: CliCommand::Limit { id, bytes_per_sec },
                }))
            }
            "delete" | "rm" => {
                if items.len() < 2 {
                    return Err(CliError::InvalidArgument(
                        "Missing task ID to delete".to_string(),
                    ));
                }
                let mut id = None;
                let mut delete_file = false;
                for arg in &items[1..] {
                    match arg.as_str() {
                        "-f" | "--delete-file" => {
                            delete_file = true;
                        }
                        uuid_str => {
                            if id.is_none() {
                                let parsed = Uuid::parse_str(uuid_str).map_err(|e| {
                                    CliError::InvalidArgument(format!("Invalid task UUID: {e}"))
                                })?;
                                id = Some(parsed);
                            } else {
                                return Err(CliError::InvalidArgument(format!(
                                    "Unexpected argument: {uuid_str}"
                                )));
                            }
                        }
                    }
                }
                let id = id.ok_or_else(|| {
                    CliError::InvalidArgument("Missing task ID to delete".to_string())
                })?;
                Ok(Some(Self {
                    command: CliCommand::Delete { id, delete_file },
                }))
            }
            "download" => Self::parse_download(&items[1..]),
            _ => Self::parse_download(&items),
        }
    }

    fn parse_download(args: &[String]) -> Result<Option<Self>, CliError> {
        let mut url = None;
        let mut output = None;
        let mut connections = None;
        let mut referer = None;
        let mut checksum = None;
        let mut headers = Vec::new();
        let mut cookies = None;
        let mut speed_limit = None;

        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    Self::print_help();
                    return Ok(None);
                }
                "-o" | "--output" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --output".to_string())
                    })?;
                    output = Some(PathBuf::from(val));
                }
                "-c" | "--connections" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --connections".to_string())
                    })?;
                    let parsed: usize = val.parse().map_err(|_| {
                        CliError::InvalidArgument(format!("Invalid connections number: {val}"))
                    })?;
                    if parsed == 0 {
                        return Err(CliError::InvalidArgument(
                            "Connections count must be greater than 0".to_string(),
                        ));
                    }
                    connections = Some(parsed);
                }
                "-r" | "--referer" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --referer".to_string())
                    })?;
                    referer = Some(val.clone());
                }
                "--checksum" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --checksum".to_string())
                    })?;
                    checksum = Some(val.clone());
                }
                "-H" | "--header" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --header".to_string())
                    })?;
                    let Some((name, value)) = val.split_once(':') else {
                        return Err(CliError::InvalidArgument(format!(
                            "Invalid header format '{val}', expected 'Name: Value'"
                        )));
                    };
                    headers.push((name.trim().to_string(), value.trim().to_string()));
                }
                "--cookie" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --cookie".to_string())
                    })?;
                    cookies = Some(val.clone());
                }
                "--speed-limit" => {
                    let val = iter.next().ok_or_else(|| {
                        CliError::InvalidArgument("Missing value for --speed-limit".to_string())
                    })?;
                    let parsed: u64 = val.parse().map_err(|_| {
                        CliError::InvalidArgument(format!("Invalid speed limit: {val}"))
                    })?;
                    speed_limit = Some(parsed);
                }
                other if other.starts_with('-') => {
                    return Err(CliError::InvalidArgument(format!(
                        "Unknown option: {other}"
                    )));
                }
                positional => {
                    if url.is_none() {
                        url = Some(positional.to_string());
                    } else {
                        return Err(CliError::InvalidArgument(format!(
                            "Unexpected extra argument: {positional}"
                        )));
                    }
                }
            }
        }

        let url = url.ok_or(CliError::MissingUrl)?;

        Ok(Some(Self {
            command: CliCommand::Download {
                url,
                output,
                connections,
                referer,
                checksum,
                headers,
                cookies,
                speed_limit,
            },
        }))
    }

    pub fn print_help() {
        println!("rDM - High Performance Internet Download Manager CLI\n");
        println!("USAGE:");
        println!("    rdm-cli <COMMAND> [OPTIONS]\n");
        println!("COMMANDS:");
        println!("    [download] <URL> [OPTIONS]   Download a file from URL (default)");
        println!("    info <URL> [OPTIONS]         Probe URL and inspect remote metadata");
        println!("    list                         List download history and tasks");
        println!("    resume <TASK_ID>             Resume a paused or interrupted download");
        println!("    refresh <TASK_ID> <URL>      Update URL for a paused/failed download");
        println!("    limit <TASK_ID> <BYTES/S>    Set per-task speed limit (0 = unlimited)");
        println!(
            "    delete <TASK_ID> [-f]        Remove download from history (-f to delete file)\n"
        );
        println!("DOWNLOAD OPTIONS:");
        println!("    -o, --output <PATH>          Target file or directory path");
        println!("    -c, --connections <NUM>      Number of concurrent segment connections");
        println!("    -r, --referer <URL>          Custom HTTP Referer header");
        println!("    --checksum <HASH>            Verify SHA-256 or MD5 (e.g. sha256:...)");
        println!("    -H, --header <NAME: VALUE>   Custom HTTP header (repeatable)");
        println!("    --cookie <COOKIES>           Custom Cookie header");
        println!("    --speed-limit <BYTES/S>      Per-task speed limit in bytes/sec");
        println!("    -h, --help                   Show this help message\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parse_minimal() {
        let args = ["https://example.com/file.zip"];
        let parsed = CliArgs::parse_from(args).unwrap().unwrap();
        assert_eq!(
            parsed.command,
            CliCommand::Download {
                url: "https://example.com/file.zip".to_string(),
                output: None,
                connections: None,
                referer: None,
                checksum: None,
                headers: Vec::new(),
                cookies: None,
                speed_limit: None,
            }
        );
    }

    #[test]
    fn test_args_parse_full() {
        let args = [
            "https://example.com/file.zip",
            "-o",
            "downloads/my_file.zip",
            "-c",
            "16",
            "-r",
            "https://example.com/",
            "--checksum",
            "md5:d41d8cd98f00b204e9800998ecf8427e",
            "-H",
            "Authorization: Bearer tok",
            "--cookie",
            "sid=123",
            "--speed-limit",
            "1048576",
        ];
        let parsed = CliArgs::parse_from(args).unwrap().unwrap();
        assert_eq!(
            parsed.command,
            CliCommand::Download {
                url: "https://example.com/file.zip".to_string(),
                output: Some(PathBuf::from("downloads/my_file.zip")),
                connections: Some(16),
                referer: Some("https://example.com/".to_string()),
                checksum: Some("md5:d41d8cd98f00b204e9800998ecf8427e".to_string()),
                headers: vec![("Authorization".to_string(), "Bearer tok".to_string())],
                cookies: Some("sid=123".to_string()),
                speed_limit: Some(1_048_576),
            }
        );
    }

    #[test]
    fn test_args_parse_subcommands() {
        let list_args = ["list"];
        let list_parsed = CliArgs::parse_from(list_args).unwrap().unwrap();
        assert_eq!(list_parsed.command, CliCommand::List);

        let info_args = [
            "info",
            "https://example.com/file.zip",
            "-r",
            "https://ref.com/",
        ];
        let info_parsed = CliArgs::parse_from(info_args).unwrap().unwrap();
        assert_eq!(
            info_parsed.command,
            CliCommand::Info {
                url: "https://example.com/file.zip".to_string(),
                referer: Some("https://ref.com/".to_string()),
            }
        );

        let id = Uuid::new_v4();
        let resume_args = ["resume", &id.to_string()];
        let resume_parsed = CliArgs::parse_from(resume_args).unwrap().unwrap();
        assert_eq!(resume_parsed.command, CliCommand::Resume { id });

        let del_args = ["delete", &id.to_string(), "-f"];
        let del_parsed = CliArgs::parse_from(del_args).unwrap().unwrap();
        assert_eq!(
            del_parsed.command,
            CliCommand::Delete {
                id,
                delete_file: true
            }
        );
    }

    #[test]
    fn test_args_parse_help() {
        let args = ["--help"];
        let parsed = CliArgs::parse_from(args).unwrap();
        assert_eq!(parsed, None);
    }

    #[test]
    fn test_args_parse_missing_url() {
        let args = ["download"];
        assert!(CliArgs::parse_from(args).is_err());
    }

    #[test]
    fn test_args_parse_zero_connections() {
        let args = ["https://example.com/file.zip", "-c", "0"];
        assert!(CliArgs::parse_from(args).is_err());
    }
}
