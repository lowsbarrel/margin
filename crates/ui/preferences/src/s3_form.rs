use margin_settings::S3Config;
use margin_sync::ConflictStrategy;

use crate::field::{FieldError, required};

pub(crate) const DEFAULT_REGION: &str = "us-east-1";

pub(crate) type S3Errors = [Option<FieldError>; 5];

pub(crate) fn s3_setting(config: S3Config) -> Result<Option<S3Config>, S3Errors> {
    let credentials = [
        &config.endpoint,
        &config.bucket,
        &config.access_key,
        &config.secret_key,
    ];
    if credentials.iter().all(|value| value.is_empty()) {
        return Ok(None);
    }
    let errors = [
        required(&config.endpoint)
            .or_else(|| (!is_http_url(&config.endpoint)).then_some(FieldError::NotHttpUrl)),
        required(&config.bucket),
        required(&config.region),
        required(&config.access_key),
        required(&config.secret_key),
    ];
    if errors.iter().all(Option::is_none) {
        Ok(Some(config))
    } else {
        Err(errors)
    }
}

pub(crate) fn conflict_strategy(setting: Option<&str>) -> ConflictStrategy {
    match setting {
        Some("keep_newer") => ConflictStrategy::KeepNewer,
        _ => ConflictStrategy::LocalWins,
    }
}

pub(crate) const fn conflict_setting(strategy: ConflictStrategy) -> &'static str {
    match strategy {
        ConflictStrategy::LocalWins => "local_wins",
        ConflictStrategy::KeepNewer => "keep_newer",
    }
}

fn is_http_url(value: &str) -> bool {
    value.split_once("://").is_some_and(|(scheme, rest)| {
        (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
            && !rest.is_empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(fields: [&str; 5]) -> S3Config {
        let [endpoint, bucket, region, access_key, secret_key] = fields.map(str::to_owned);
        S3Config {
            endpoint,
            bucket,
            region,
            access_key,
            secret_key,
        }
    }

    #[test]
    fn empty_credentials_turn_sync_off_even_with_the_default_region() {
        assert_eq!(
            s3_setting(config(["", "", DEFAULT_REGION, "", ""])),
            Ok(None)
        );
    }

    #[test]
    fn a_complete_config_is_kept() {
        let complete = config(["https://s3.test", "notes", "eu-west-1", "AK", "SK"]);
        assert_eq!(s3_setting(complete.clone()), Ok(Some(complete)));
    }

    #[test]
    fn a_partial_config_reports_each_missing_field() {
        let required = Some(FieldError::Required);
        assert_eq!(
            s3_setting(config(["https://s3.test", "", "", "AK", ""])),
            Err([None, required, required, None, required])
        );
    }

    #[test]
    fn the_endpoint_must_be_an_http_url() {
        for endpoint in ["s3.test", "ftp://s3.test", "https://"] {
            let errors = s3_setting(config([endpoint, "b", "r", "a", "s"]))
                .expect_err("the endpoint is not an http url");
            assert_eq!(errors[0], Some(FieldError::NotHttpUrl), "{endpoint}");
        }
        assert_eq!(
            s3_setting(config(["HTTP://localhost:9000", "b", "r", "a", "s"]))
                .map(|setting| setting.is_some()),
            Ok(true)
        );
    }

    #[test]
    fn conflict_strategies_round_trip_through_the_setting_string() {
        for strategy in [ConflictStrategy::LocalWins, ConflictStrategy::KeepNewer] {
            assert_eq!(
                conflict_strategy(Some(conflict_setting(strategy))),
                strategy
            );
        }
        assert_eq!(conflict_strategy(None), ConflictStrategy::LocalWins);
        assert_eq!(
            conflict_strategy(Some("unknown")),
            ConflictStrategy::LocalWins
        );
    }
}
