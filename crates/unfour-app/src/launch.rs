//! OS envelope guard only. AccessClient formats are deliberately unsupported.
//! The existing account plugin and metadata CLI remain their own consumers.
use std::ffi::OsString;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LaunchDisposition {
    NoRequest,
    ExistingEntry,
    Unsupported,
}

pub(crate) fn classify(arguments: &[OsString]) -> LaunchDisposition {
    match arguments {
        [] => LaunchDisposition::NoRequest,
        [first, ..] if first == "--write-build-metadata" => LaunchDisposition::ExistingEntry,
        [uri]
            if uri
                .to_str()
                .is_some_and(|value| value.starts_with("unfour://")) =>
        {
            LaunchDisposition::ExistingEntry
        }
        _ => LaunchDisposition::Unsupported,
    }
}

pub(crate) fn inspect(arguments: &[OsString]) {
    if classify(arguments) == LaunchDisposition::Unsupported {
        // Never log argv, cwd, URIs or driver messages.
        eprintln!("BASTION_LAUNCH_UNSUPPORTED");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn cold_and_forwarded_envelopes_keep_existing_consumers() {
        assert_eq!(classify(&[]), LaunchDisposition::NoRequest);
        assert_eq!(
            classify(&args(&[
                "unfour://auth/callback?authorizationCode=SYNTHETIC&state=SYNTHETIC"
            ])),
            LaunchDisposition::ExistingEntry
        );
        assert_eq!(
            classify(&args(&[
                "--write-build-metadata",
                "C:\\fixture\\metadata.json"
            ])),
            LaunchDisposition::ExistingEntry
        );
        for argv in [
            vec!["-url", "ssh://SYNTHETIC:SYNTHETIC@fixture.invalid"],
            vec!["accessclient:SYNTHETIC"],
            vec!["session.xsh"],
            vec!["--execute", "SYNTHETIC"],
            vec!["unfour://auth/callback", "extra"],
        ] {
            assert_eq!(classify(&args(&argv)), LaunchDisposition::Unsupported);
        }
    }
}
