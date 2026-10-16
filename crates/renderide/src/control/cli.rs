//! `--control-port` parsing. Present only when the `control-api` feature is on.

/// Returns the requested port when `--control-port` is present.
pub(crate) fn parse_control_port(args: &[String]) -> Result<Option<u16>, String> {
    let mut port = None;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if !arg.eq_ignore_ascii_case("--control-port") {
            index += 1;
            continue;
        }
        let Some(value) = args.get(index + 1) else {
            return Err("--control-port requires a port".to_owned());
        };
        let parsed = value
            .parse::<u16>()
            .map_err(|_parse| format!("invalid --control-port value '{value}'"))?;
        port = Some(parsed);
        index += 2;
    }
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::parse_control_port;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn absent_flag_is_none() {
        assert_eq!(
            parse_control_port(&args(&["renderide"])).expect("parse"),
            None
        );
    }

    #[test]
    fn parses_port_case_insensitively() {
        let parsed =
            parse_control_port(&args(&["renderide", "--Control-Port", "9876"])).expect("parse");
        assert_eq!(parsed, Some(9876));
    }

    #[test]
    fn missing_value_is_an_error() {
        let err = parse_control_port(&args(&["renderide", "--control-port"])).expect_err("missing");
        assert!(err.contains("requires"));
    }
}
