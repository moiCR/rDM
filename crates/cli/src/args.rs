use std::path::PathBuf;

use crate::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub struct CliArgs {
    pub url: String,
    pub output: Option<PathBuf>,
    pub connections: Option<usize>,
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
        let mut url = None;
        let mut output = None;
        let mut connections = None;

        let mut iter = args.into_iter().map(Into::into);

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
            url,
            output,
            connections,
        }))
    }

    pub fn print_help() {
        println!("rDM - High Performance Internet Download Manager CLI\n");
        println!("USAGE:");
        println!("    rdm-cli <URL> [OPTIONS]\n");
        println!("ARGUMENTS:");
        println!("    <URL>                     The direct URL of the file to download\n");
        println!("OPTIONS:");
        println!("    -o, --output <PATH>       Target file or directory path");
        println!("    -c, --connections <NUM>   Number of concurrent segment connections");
        println!("    -h, --help                Show this help message\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parse_minimal() {
        let args = ["https://example.com/file.zip"];
        let parsed = CliArgs::parse_from(args).unwrap().unwrap();
        assert_eq!(parsed.url, "https://example.com/file.zip");
        assert_eq!(parsed.output, None);
        assert_eq!(parsed.connections, None);
    }

    #[test]
    fn test_args_parse_full() {
        let args = [
            "https://example.com/file.zip",
            "-o",
            "downloads/my_file.zip",
            "-c",
            "16",
        ];
        let parsed = CliArgs::parse_from(args).unwrap().unwrap();
        assert_eq!(parsed.url, "https://example.com/file.zip");
        assert_eq!(parsed.output, Some(PathBuf::from("downloads/my_file.zip")));
        assert_eq!(parsed.connections, Some(16));
    }

    #[test]
    fn test_args_parse_help() {
        let args = ["--help"];
        let parsed = CliArgs::parse_from(args).unwrap();
        assert_eq!(parsed, None);
    }

    #[test]
    fn test_args_parse_missing_url() {
        let args: [&str; 0] = [];
        assert!(CliArgs::parse_from(args).is_err());
    }

    #[test]
    fn test_args_parse_zero_connections() {
        let args = ["https://example.com/file.zip", "-c", "0"];
        assert!(CliArgs::parse_from(args).is_err());
    }
}
