use std::fmt::Write;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub(crate) const ALGORITHM: &str = "AWS4-HMAC-SHA256";
const WRITE: &str = "writing into a String cannot fail";

pub(crate) struct SignableRequest<'a> {
    pub method: &'a str,
    pub uri: &'a str,
    pub query: &'a [(String, String)],
    pub headers: &'a [(String, String)],
    pub payload_hash: &'a str,
    pub timestamp: &'a str,
    pub date: &'a str,
}

pub(crate) struct Credentials<'a> {
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub region: &'a str,
}

#[must_use]
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[must_use]
pub(crate) fn uri_encode(value: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(byte));
            }
            b'/' if !encode_slash => out.push('/'),
            _ => write!(out, "%{byte:02X}").expect(WRITE),
        }
    }
    out
}

#[must_use]
pub(crate) fn canonical_query(pairs: &[(String, String)]) -> String {
    let mut encoded: Vec<(String, String)> = pairs
        .iter()
        .map(|(name, value)| (uri_encode(name, true), uri_encode(value, true)))
        .collect();
    encoded.sort();
    let mut out = String::new();
    for (index, (name, value)) in encoded.iter().enumerate() {
        if index > 0 {
            out.push('&');
        }
        write!(out, "{name}={value}").expect(WRITE);
    }
    out
}

#[must_use]
pub(crate) fn signed_headers(headers: &[(String, String)]) -> String {
    let mut names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names.join(";")
}

#[must_use]
pub(crate) fn canonical_request(request: &SignableRequest<'_>) -> String {
    let mut headers: Vec<(&str, &str)> = request
        .headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    headers.sort_unstable();
    let mut out = String::new();
    write!(
        out,
        "{}
{}
{}
",
        request.method,
        request.uri,
        canonical_query(request.query)
    )
    .expect(WRITE);
    for (name, value) in &headers {
        writeln!(out, "{name}:{}", value.trim()).expect(WRITE);
    }
    write!(
        out,
        "
{}
{}",
        signed_headers(request.headers),
        request.payload_hash
    )
    .expect(WRITE);
    out
}

#[must_use]
pub(crate) fn credential_scope(date: &str, region: &str, service: &str) -> String {
    format!("{date}/{region}/{service}/aws4_request")
}

#[must_use]
pub(crate) fn string_to_sign(request: &SignableRequest<'_>, scope: &str) -> String {
    let digest = sha256_hex(canonical_request(request).as_bytes());
    format!(
        "{ALGORITHM}
{}
{scope}
{digest}",
        request.timestamp
    )
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

#[must_use]
pub(crate) fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let date_key = hmac(format!("AWS4{secret}").as_bytes(), date);
    let region_key = hmac(&date_key, region);
    let service_key = hmac(&region_key, service);
    hmac(&service_key, "aws4_request")
}

#[must_use]
pub(crate) fn signature(
    credentials: &Credentials<'_>,
    service: &str,
    request: &SignableRequest<'_>,
) -> String {
    let scope = credential_scope(request.date, credentials.region, service);
    let key = signing_key(
        credentials.secret_key,
        request.date,
        credentials.region,
        service,
    );
    hex::encode(hmac(&key, &string_to_sign(request, &scope)))
}

#[must_use]
pub(crate) fn authorization(
    credentials: &Credentials<'_>,
    request: &SignableRequest<'_>,
) -> String {
    let scope = credential_scope(request.date, credentials.region, "s3");
    let value = signature(credentials, "s3", request);
    format!(
        "{ALGORITHM} Credential={}/{scope}, SignedHeaders={}, Signature={value}",
        credentials.access_key,
        signed_headers(request.headers)
    )
}

#[must_use]
pub(crate) fn amz_dates(seconds: u64) -> (String, String) {
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let time = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let date = format!("{year:04}{month:02}{day:02}");
    let stamp = format!(
        "{date}T{:02}{:02}{:02}Z",
        time / 3600,
        (time % 3600) / 60,
        time % 60
    );
    (date, stamp)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_PAYLOAD_HASH: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const SECRET: &str = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
    const TIMESTAMP: &str = "20150830T123600Z";
    const DATE: &str = "20150830";

    struct Vector {
        method: &'static str,
        uri: &'static str,
        query: Vec<(String, String)>,
        canonical_request: &'static str,
        signature: &'static str,
    }

    fn request(
        method: &'static str,
        uri: &'static str,
        query: Vec<(String, String)>,
    ) -> SignableRequest<'static> {
        let headers = vec![
            ("host".to_owned(), "example.amazonaws.com".to_owned()),
            ("x-amz-date".to_owned(), TIMESTAMP.to_owned()),
        ];
        SignableRequest {
            method,
            uri,
            query: Box::leak(query.into_boxed_slice()),
            headers: Box::leak(headers.into_boxed_slice()),
            payload_hash: EMPTY_PAYLOAD_HASH,
            timestamp: TIMESTAMP,
            date: DATE,
        }
    }

    fn credentials() -> Credentials<'static> {
        Credentials {
            access_key: "AKIDEXAMPLE",
            secret_key: SECRET,
            region: "us-east-1",
        }
    }

    fn vectors() -> Vec<Vector> {
        vec![
            Vector {
                method: "GET",
                uri: "/",
                query: Vec::new(),
                canonical_request: "GET
/

host:example.amazonaws.com
x-amz-date:20150830T123600Z

host;x-amz-date
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                signature: "5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31",
            },
            Vector {
                method: "GET",
                uri: "/",
                query: vec![
                    ("Param2".to_owned(), "value2".to_owned()),
                    ("Param1".to_owned(), "value1".to_owned()),
                ],
                canonical_request: "GET
/
Param1=value1&Param2=value2
host:example.amazonaws.com
x-amz-date:20150830T123600Z

host;x-amz-date
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                signature: "b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500",
            },
            Vector {
                method: "GET",
                uri: "/",
                query: vec![
                    ("Param-3".to_owned(), "Value3".to_owned()),
                    ("Param".to_owned(), "Value2".to_owned()),
                    ("ሴ".to_owned(), "Value1".to_owned()),
                ],
                canonical_request: "GET
/
%E1%88%B4=Value1&Param=Value2&Param-3=Value3
host:example.amazonaws.com
x-amz-date:20150830T123600Z

host;x-amz-date
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                signature: "371d3713e185cc334048618a97f809c9ffe339c62934c032af5a0e595648fcac",
            },
            Vector {
                method: "GET",
                uri: "/-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
                query: Vec::new(),
                canonical_request: "GET
/-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz

host:example.amazonaws.com
x-amz-date:20150830T123600Z

host;x-amz-date
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                signature: "07ef7494c76fa4850883e2b006601f940f8a34d404d0cfa977f52a65bbf5f24f",
            },
            Vector {
                method: "GET",
                uri: "/",
                query: vec![("Param1".to_owned(), "value1".to_owned())],
                canonical_request: "GET
/
Param1=value1
host:example.amazonaws.com
x-amz-date:20150830T123600Z

host;x-amz-date
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                signature: "a67d582fa61cc504c4bae71f336f98b97f1ea3c7a6bfe1b6e45aec72011b9aeb",
            },
            Vector {
                method: "POST",
                uri: "/",
                query: Vec::new(),
                canonical_request: "POST
/

host:example.amazonaws.com
x-amz-date:20150830T123600Z

host;x-amz-date
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                signature: "5da7c1a2acd57cee7505fc6676e4e544621c30862966e37dddb68e92efbe5d6b",
            },
        ]
    }

    #[test]
    fn canonical_requests_match_the_published_vectors() {
        for vector in vectors() {
            let request = request(vector.method, vector.uri, vector.query);
            assert_eq!(canonical_request(&request), vector.canonical_request);
        }
    }

    #[test]
    fn signatures_match_the_published_vectors() {
        for vector in vectors() {
            let request = request(vector.method, vector.uri, vector.query);
            assert_eq!(
                signature(&credentials(), "service", &request),
                vector.signature
            );
        }
    }

    #[test]
    fn authorization_binds_the_s3_service_and_access_key() {
        let request = request("GET", "/", Vec::new());
        let value = authorization(&credentials(), &request);
        assert!(value.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/s3/aws4_request"
        ));
        assert!(value.contains("SignedHeaders=host;x-amz-date"));
        assert_eq!(value.matches("Signature=").count(), 1);
    }

    #[test]
    fn dates_format_a_known_instant() {
        assert_eq!(
            amz_dates(0),
            ("19700101".to_owned(), "19700101T000000Z".to_owned())
        );
        assert_eq!(
            amz_dates(1_440_938_160),
            ("20150830".to_owned(), "20150830T123600Z".to_owned())
        );
    }
}
