use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub trait HashableId: Sized + Send + Sync {
    fn to_hash(&self) -> String;
    fn from_hash(hash: &str) -> Option<Self>;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ClientId(Uuid);

impl ClientId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub fn sqlite_hash(&self) -> String {
        self.to_string()
    }
}

impl Default for ClientId {
    fn default() -> Self {
        Self::new()
    }
}

impl HashableId for ClientId {
    fn to_hash(&self) -> String {
        self.to_string()
    }
    fn from_hash(hash: &str) -> Option<Self> {
        hash.strip_prefix("Client-")
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(Self)
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client-{}", self.0)
    }
}

impl From<String> for ClientId {
    fn from(value: String) -> Self {
        Self::from_hash(&value).unwrap_or_default()
    }
}

impl FromStr for ClientId {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_hash(value).ok_or(())
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum SyncId {
    ClientId(ClientId),
    ServerId(ServerId),
}

impl fmt::Display for SyncId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientId(id) => id.fmt(f),
            Self::ServerId(id) => id.fmt(f),
        }
    }
}

impl Serialize for SyncId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ClientId(id) => id.to_hash().serialize(serializer),
            Self::ServerId(id) => id.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SyncId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(ClientId::from_hash(&value)
            .map(Self::ClientId)
            .unwrap_or_else(|| Self::ServerId(ServerId::from_string_lossy(value))))
    }
}

impl From<ServerId> for SyncId {
    fn from(value: ServerId) -> Self {
        Self::ServerId(value)
    }
}

impl SyncId {
    pub fn into_server(self) -> Option<ServerId> {
        match self {
            Self::ServerId(id) => Some(id),
            Self::ClientId(_) => None,
        }
    }
    pub fn into_client(self) -> Option<ClientId> {
        match self {
            Self::ClientId(id) => Some(id),
            Self::ServerId(_) => None,
        }
    }
    pub fn uid(&self) -> ObjectUid {
        self.to_string()
    }

    pub fn from_object_id<T: ToServerId>(id: T) -> Self {
        Self::ServerId(id.to_server_id())
    }
}

impl settings_value::SettingsValue for SyncId {}

pub type ObjectUid = String;
pub type HashedSqliteId = String;
pub type ApiKeyUid = String;

#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
pub struct ServerId([char; 22]);

impl ServerId {
    pub fn from_string_lossy(value: impl AsRef<str>) -> Self {
        let value = value.as_ref();
        Self::try_from(value).unwrap_or_else(|_| {
            let value = if value.len() > 22 {
                &value[value.len() - 22..]
            } else {
                value
            };
            Self::try_from(format!("{value:0>22}").as_str()).expect("normalized server ID")
        })
    }
    pub fn uid(&self) -> ObjectUid {
        (*self).into()
    }
}

impl FromStr for ServerId {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value)
    }
}

impl TryFrom<&str> for ServerId {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value
            .chars()
            .collect::<Vec<_>>()
            .try_into()
            .map(Self)
            .map_err(|_| ())
    }
}

impl TryFrom<String> for ServerId {
    type Error = ();
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl From<ServerId> for String {
    fn from(value: ServerId) -> Self {
        value.0.into_iter().collect()
    }
}

impl Serialize for ServerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        String::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ServerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("invalid server ID"))
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from(*self))
    }
}

impl fmt::Debug for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ServerId")
            .field(&String::from(*self))
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerIdAndType {
    pub id: ServerId,
    pub id_type: String,
}

impl ServerIdAndType {
    pub fn sqlite_type_and_uid_hash(&self) -> HashedSqliteId {
        format!("{}-{}", self.id_type, self.id)
    }
}

pub trait ToServerId {
    fn to_server_id(&self) -> ServerId;
}

pub fn parse_sqlite_id_to_uid(value: HashedSqliteId) -> Result<ObjectUid, ()> {
    value
        .rsplit_once('-')
        .map(|(_, uid)| uid.to_owned())
        .ok_or(())
}

#[macro_export]
macro_rules! server_id_traits {
    ($t:ty, $prefix:literal) => {
        #[cfg(any(test, feature = "test-util"))]
        impl From<i64> for $t {
            fn from(value: i64) -> Self {
                Self(value.into())
            }
        }

        impl From<String> for $t {
            fn from(value: String) -> Self {
                Self($crate::server::ids::ServerId::from_string_lossy(value))
            }
        }
        impl From<$t> for String {
            fn from(value: $t) -> Self {
                value.0.into()
            }
        }
        impl std::fmt::Display for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl $crate::server::ids::HashableId for $t {
            fn to_hash(&self) -> String {
                format!("{}-{}", $prefix, self)
            }
            fn from_hash(value: &str) -> Option<Self> {
                value
                    .strip_prefix(concat!($prefix, "-"))
                    .map(|value| value.to_owned().into())
            }
        }
        impl From<$t> for $crate::server::ids::ServerId {
            fn from(value: $t) -> Self {
                value.0
            }
        }
        impl From<$crate::server::ids::ServerId> for $t {
            fn from(value: $crate::server::ids::ServerId) -> Self {
                Self(value)
            }
        }
        impl $crate::server::ids::ToServerId for $t {
            fn to_server_id(&self) -> $crate::server::ids::ServerId {
                self.0
            }
        }
    };
}
