use klynt_sandbox::helper_proto::{HelperMode, HelperPolicy};

pub struct ParsedArgs {
    pub policy: HelperPolicy,
    pub program: String,
    pub args: Vec<String>,
}

pub fn parse(argv: &[String]) -> Result<ParsedArgs, String> {
    // Expected forms:
    //   klynt-sandbox-helper --landlock      <base64-policy> -- <program> <args...>
    //   klynt-sandbox-helper --landlock-only <base64-policy> -- <program> <args...>
    if argv.len() < 5 {
        return Err(format!(
            "usage: {} --landlock|--landlock-only <base64-policy> -- <program> <args...>",
            argv.first().map(String::as_str).unwrap_or("klynt-sandbox-helper"),
        ));
    }
    let mode_flag = &argv[1];
    let mode = match mode_flag.as_str() {
        "--landlock" => HelperMode::WithBwrap,
        "--landlock-only" => HelperMode::LandlockOnly,
        other => return Err(format!("unknown mode flag: {other}")),
    };
    let policy_b64 = &argv[2];
    if argv[3] != "--" {
        return Err(format!("expected '--' delimiter at arg 3, got {:?}", argv[3]));
    }
    let policy = HelperPolicy::from_base64_json(policy_b64)
        .map_err(|e| format!("policy decode: {e}"))?;
    if policy.mode != mode {
        return Err(format!(
            "policy.mode={:?} but CLI flag={:?} — mismatch",
            policy.mode, mode
        ));
    }
    let program = argv[4].clone();
    let args = argv[5..].to_vec();
    Ok(ParsedArgs { policy, program, args })
}

#[cfg(test)]
mod tests {
    use super::*;
    use klynt_sandbox::policy::SandboxPolicy;
    use std::path::PathBuf;

    fn build_argv(mode_flag: &str) -> Vec<String> {
        let pol = HelperPolicy {
            mode: if mode_flag == "--landlock" { HelperMode::WithBwrap } else { HelperMode::LandlockOnly },
            sandbox: SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/work")),
        };
        let b64 = pol.to_base64_json().unwrap();
        vec![
            "klynt-sandbox-helper".into(), mode_flag.into(),
            b64, "--".into(), "/bin/echo".into(), "hi".into(),
        ]
    }

    #[test] fn parses_landlock_mode() {
        let p = parse(&build_argv("--landlock")).unwrap();
        assert_eq!(p.program, "/bin/echo"); assert_eq!(p.args, vec!["hi"]);
        assert_eq!(p.policy.mode, HelperMode::WithBwrap);
    }

    #[test] fn rejects_missing_delimiter() {
        let mut argv = build_argv("--landlock");
        argv[3] = "WRONG".into();
        assert!(parse(&argv).is_err());
    }

    #[test] fn rejects_unknown_mode_flag() {
        let mut argv = build_argv("--landlock");
        argv[1] = "--bogus".into();
        assert!(parse(&argv).is_err());
    }
}
