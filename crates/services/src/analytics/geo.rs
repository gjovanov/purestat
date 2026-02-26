use maxminddb::{geoip2, Reader};
use std::net::IpAddr;
use std::path::Path;
use tracing::warn;

pub struct GeoResult {
    pub country: String,
    pub region: String,
    pub city: String,
}

pub struct GeoService {
    reader: Option<Reader<Vec<u8>>>,
}

impl GeoService {
    pub fn new(db_path: &str) -> Self {
        let path = Path::new(db_path);
        if !path.exists() {
            warn!(path = %db_path, "GeoIP database not found, geo lookup disabled");
            return Self { reader: None };
        }
        match Reader::open_readfile(db_path) {
            Ok(reader) => {
                tracing::info!(path = %db_path, "GeoIP database loaded");
                Self {
                    reader: Some(reader),
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to open GeoIP database, geo lookup disabled");
                Self { reader: None }
            }
        }
    }

    pub fn lookup(&self, ip_str: &str) -> GeoResult {
        let default = GeoResult {
            country: String::new(),
            region: String::new(),
            city: String::new(),
        };

        let reader = match &self.reader {
            Some(r) => r,
            None => return default,
        };

        let ip: IpAddr = match ip_str.parse() {
            Ok(ip) => ip,
            Err(_) => return default,
        };

        match reader.lookup::<geoip2::City>(ip) {
            Ok(city_result) => {
                let country = city_result
                    .country
                    .and_then(|c| c.iso_code)
                    .unwrap_or("")
                    .to_string();
                let region = city_result
                    .subdivisions
                    .and_then(|s| s.into_iter().next())
                    .and_then(|s| s.names)
                    .and_then(|n| n.get("en").copied())
                    .unwrap_or("")
                    .to_string();
                let city = city_result
                    .city
                    .and_then(|c| c.names)
                    .and_then(|n| n.get("en").copied())
                    .unwrap_or("")
                    .to_string();
                GeoResult {
                    country,
                    region,
                    city,
                }
            }
            Err(_) => default,
        }
    }
}
