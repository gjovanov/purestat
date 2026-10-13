use bson::{DateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub org_id: ObjectId,
    pub domain: String,
    pub name: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub is_public: bool,
    /// Hostnames, beyond the domain and `www.` + the domain, whose pages count
    /// for this site (a staging host, for example).
    #[serde(default)]
    pub allowed_hostnames: Vec<String>,
    /// The site's id in ClickHouse, assigned when the site is created: a
    /// random positive value that no other site's id equals. Sites created
    /// before it existed have none and keep the id `clickhouse_id` derives
    /// from their ObjectId, because that is what their rows carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch_id: Option<u64>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

impl Site {
    pub const COLLECTION: &'static str = "sites";

    /// The `site_id` this site's rows carry in ClickHouse: `ch_id` when the
    /// site has one, otherwise the id derived from its ObjectId.
    pub fn clickhouse_id(&self) -> u64 {
        match (self.ch_id, self.id) {
            (Some(ch_id), _) => ch_id,
            (None, Some(id)) => Self::legacy_clickhouse_id(&id),
            (None, None) => 0,
        }
    }

    /// The ClickHouse id of a site without a `ch_id`: the first 8 bytes of
    /// its ObjectId, big-endian.
    pub fn legacy_clickhouse_id(id: &ObjectId) -> u64 {
        let bytes = id.bytes();
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(id: Option<ObjectId>, ch_id: Option<u64>) -> Site {
        Site {
            id,
            org_id: ObjectId::new(),
            domain: "example.com".into(),
            name: "Example".into(),
            timezone: "UTC".into(),
            is_public: false,
            allowed_hostnames: vec![],
            ch_id,
            created_at: DateTime::now(),
            updated_at: DateTime::now(),
        }
    }

    #[test]
    fn an_assigned_id_wins_over_the_derived_one() {
        let id = ObjectId::new();
        assert_eq!(site(Some(id), Some(42)).clickhouse_id(), 42);
        assert_eq!(
            site(Some(id), None).clickhouse_id(),
            Site::legacy_clickhouse_id(&id)
        );
    }

    #[test]
    fn the_derived_id_is_the_first_eight_bytes_big_endian() {
        let id = ObjectId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(Site::legacy_clickhouse_id(&id), 0x0102030405060708);
    }

    #[test]
    fn a_site_document_without_ch_id_still_deserializes() {
        let doc = bson::doc! {
            "_id": ObjectId::new(),
            "org_id": ObjectId::new(),
            "domain": "example.com",
            "name": "Example",
            "created_at": DateTime::now(),
            "updated_at": DateTime::now(),
        };
        let site: Site = bson::from_document(doc).unwrap();
        assert_eq!(site.ch_id, None);
        assert!(site.allowed_hostnames.is_empty());
    }

    #[test]
    fn ch_id_round_trips_through_bson() {
        let stored = bson::to_document(&site(None, Some(i64::MAX as u64))).unwrap();
        assert_eq!(stored.get_i64("ch_id").unwrap(), i64::MAX);
        let back: Site = bson::from_document(stored).unwrap();
        assert_eq!(back.ch_id, Some(i64::MAX as u64));
    }
}
