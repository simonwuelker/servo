/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![deny(unsafe_code)]
#![crate_name = "servo_url"]
#![crate_type = "rlib"]

pub mod encoding;
pub mod origin;

use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::net::IpAddr;
use std::ops::{Deref, Index, Range, RangeFrom, RangeFull, RangeTo};
use std::path::Path;
use std::str::FromStr;

use malloc_size_of::MallocSizeOf;
use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};
use servo_arc::Arc;
pub use url::Host;
use url::{Position, Url};

pub use crate::origin::{ImmutableOrigin, MutableOrigin, OpaqueOrigin, OriginSnapshot};

const DATA_URL_DISPLAY_LENGTH: usize = 40;

#[derive(Debug)]
pub enum UrlError {
    SetUsername,
    SetIpHost,
    SetPassword,
    ToFilePath,
    FromFilePath,
}

pub trait BlobToken:
    Clone
    + PartialEq
    + Eq
    + Hash
    + PartialOrd
    + Ord
    + Serialize
    + MallocSizeOf
    + for<'a> Deserialize<'a>
    + Send
{
    fn refresh(&self) -> Self;
    fn neuter(&mut self);
}

impl<T: BlobToken> serde::Serialize for TokenSerializationGuard<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut new_token = self.token.refresh();
        let result = new_token.serialize(serializer);
        if result.is_ok() {
            // This token belongs to whoever receives the serialized message, so don't free it.
            new_token.neuter();
        }
        result
    }
}

impl<'a, T: BlobToken> serde::Deserialize<'a> for TokenSerializationGuard<T> {
    fn deserialize<D>(de: D) -> Result<Self, <D as serde::Deserializer<'a>>::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct MethodVisitor<T>(PhantomData<T>);

        impl<'de, T: BlobToken> serde::de::Visitor<'de> for MethodVisitor<T> {
            type Value = TokenSerializationGuard<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a TokenSerializationGuard")
            }

            fn visit_newtype_struct<D>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, <D as serde::Deserializer<'de>>::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Ok(TokenSerializationGuard {
                    token: Arc::new(T::deserialize(deserializer)?),
                })
            }
        }

        de.deserialize_newtype_struct("TokenSerializationGuard", MethodVisitor::<T>(PhantomData))
    }
}

#[derive(Deserialize, Serialize)]
pub struct BlobStoreAgnosticServoUrl<T: BlobToken> {
    url: Arc<Url>,
    /// A token that guarantees that the `Blob` referenced by this URL is not removed
    /// from the blob storage before this URL is dropped. `None` if the scheme of the URL
    /// is not `blob`.
    #[serde(deserialize_with = "<Option<TokenSerializationGuard<T>>>::deserialize")]
    token: Option<TokenSerializationGuard<T>>,
}

/// Guarantees that blob entries kept alive the contained token are not deallocated even
/// if this token is serialized, dropped, and then later deserialized (possibly in a different thread).
#[derive(Clone, PartialEq, Eq, Hash, MallocSizeOf, Ord, PartialOrd)]
pub struct TokenSerializationGuard<T: BlobToken> {
    #[conditional_malloc_size_of]
    token: Arc<T>,
}

impl<T: BlobToken> TokenSerializationGuard<T> {
    pub fn new(token: T) -> Self {
        Self {
            token: Arc::new(token),
        }
    }
}

impl<T: BlobToken> BlobStoreAgnosticServoUrl<T> {
    pub fn blob_token(&self) -> &Option<TokenSerializationGuard<T>> {
        &self.token
    }

    pub fn from_url_with_token(url: Url, token: Option<TokenSerializationGuard<T>>) -> Self {
        debug_assert!(token.is_some() || url.scheme() != "blob");
        Self {
            url: Arc::new(url),
            token,
        }
    }

    pub fn from_shared_non_blob_url(url: Arc<Url>) -> Result<Self, IsBlobUrlError> {
        if url.scheme() == "blob" {
            return Err(IsBlobUrlError);
        }

        Ok(Self { url, token: None })
    }

    pub fn from_non_blob_url(url: Url) -> Result<Self, IsBlobUrlError> {
        if url.scheme() == "blob" {
            return Err(IsBlobUrlError);
        }

        Ok(Self {
            url: Arc::new(url),
            token: None,
        })
    }

    pub fn into_string(self) -> String {
        String::from(self.into_url())
    }

    pub fn into_url(self) -> Url {
        self.as_url().clone()
    }

    pub fn get_arc(&self) -> Arc<Url> {
        self.url.clone()
    }

    pub fn as_url(&self) -> &Url {
        &self.url
    }

    pub fn cannot_be_a_base(&self) -> bool {
        self.url.cannot_be_a_base()
    }

    pub fn domain(&self) -> Option<&str> {
        self.url.domain()
    }

    pub fn fragment(&self) -> Option<&str> {
        self.url.fragment()
    }

    pub fn path(&self) -> &str {
        self.url.path()
    }

    pub fn origin(&self) -> ImmutableOrigin {
        ImmutableOrigin::new(self.url.origin())
    }

    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    pub fn is_secure_scheme(&self) -> bool {
        let scheme = self.scheme();
        scheme == "https" || scheme == "wss"
    }

    /// <https://fetch.spec.whatwg.org/#local-scheme>
    pub fn is_local_scheme(&self) -> bool {
        let scheme = self.scheme();
        scheme == "about" || scheme == "blob" || scheme == "data"
    }

    /// <https://url.spec.whatwg.org/#special-scheme>
    pub fn is_special_scheme(&self) -> bool {
        let scheme = self.scheme();
        scheme == "ftp" ||
            scheme == "file" ||
            scheme == "http" ||
            scheme == "https" ||
            scheme == "ws" ||
            scheme == "wss"
    }

    /// <https://url.spec.whatwg.org/#url-equivalence>
    /// In the future this may be removed if the helper is added upstream in rust-url
    /// see <https://github.com/servo/rust-url/issues/1063> for details
    pub fn is_equal_excluding_fragments<Other: BlobToken>(
        &self,
        other: &BlobStoreAgnosticServoUrl<Other>,
    ) -> bool {
        self.url[..Position::AfterQuery] == other.url[..Position::AfterQuery]
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    pub fn as_mut_url(&mut self) -> &mut Url {
        Arc::make_mut(&mut self.url)
    }

    pub fn set_username(&mut self, user: &str) -> Result<(), UrlError> {
        self.as_mut_url()
            .set_username(user)
            .map_err(|_| UrlError::SetUsername)
    }

    pub fn set_ip_host(&mut self, addr: IpAddr) -> Result<(), UrlError> {
        self.as_mut_url()
            .set_ip_host(addr)
            .map_err(|_| UrlError::SetIpHost)
    }

    pub fn set_password(&mut self, pass: Option<&str>) -> Result<(), UrlError> {
        self.as_mut_url()
            .set_password(pass)
            .map_err(|_| UrlError::SetPassword)
    }

    pub fn set_fragment(&mut self, fragment: Option<&str>) {
        self.as_mut_url().set_fragment(fragment)
    }

    pub fn username(&self) -> &str {
        self.url.username()
    }

    pub fn password(&self) -> Option<&str> {
        self.url.password()
    }

    pub fn to_file_path(&self) -> Result<::std::path::PathBuf, UrlError> {
        self.url.to_file_path().map_err(|_| UrlError::ToFilePath)
    }

    pub fn host(&self) -> Option<url::Host<&str>> {
        self.url.host()
    }

    pub fn host_str(&self) -> Option<&str> {
        self.url.host_str()
    }

    pub fn port(&self) -> Option<u16> {
        self.url.port()
    }

    pub fn port_or_known_default(&self) -> Option<u16> {
        self.url.port_or_known_default()
    }

    pub fn join(&self, input: &str) -> Result<Self, url::ParseError> {
        let url = self.url.join(input)?;
        Ok(Self::from_url_with_token(url, self.token.clone()))
    }

    pub fn path_segments(&self) -> Option<::std::str::Split<'_, char>> {
        self.url.path_segments()
    }

    pub fn query(&self) -> Option<&str> {
        self.url.query()
    }

    pub fn from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, UrlError> {
        Url::from_file_path(path)
            .map(|url| Self::from_non_blob_url(url).unwrap())
            .map_err(|_| UrlError::FromFilePath)
    }

    /// Return a non-standard shortened form of the URL. Mainly intended to be
    /// used for debug printing in a constrained space (e.g., thread names).
    pub fn debug_compact(&self) -> impl std::fmt::Display + '_ {
        match self.scheme() {
            "http" | "https" => {
                // Strip `scheme://`, which is hardly useful for identifying websites
                let mut st = self.as_str();
                st = st.strip_prefix(self.scheme()).unwrap_or(st);
                st = st.strip_prefix(':').unwrap_or(st);
                st = st.trim_start_matches('/');

                // Don't want to return an empty string
                if st.is_empty() {
                    st = self.as_str();
                }

                st
            },
            "file" => {
                // The only useful part in a `file` URL is usually only the last
                // few components
                let path = self.path();
                let i = path.rfind('/');
                let i = i.map(|i| path[..i].rfind('/').unwrap_or(i));
                match i {
                    None | Some(0) => path,
                    Some(i) => &path[i + 1..],
                }
            },
            _ => self.as_str(),
        }
    }

    /// <https://w3c.github.io/webappsec-secure-contexts/#potentially-trustworthy-url>
    pub fn is_potentially_trustworthy(&self) -> bool {
        // Step 1
        if self.as_str() == "about:blank" || self.as_str() == "about:srcdoc" {
            return true;
        }
        // Step 2
        if self.scheme() == "data" {
            return true;
        }
        // Step 3
        self.origin().is_potentially_trustworthy()
    }

    /// <https://html.spec.whatwg.org/multipage/#matches-about:blank>
    pub fn matches_about_blank(&self) -> bool {
        // A URL matches about:blank if

        // its scheme is "about",
        let scheme_is_about = self.scheme() == "about";

        // its path contains a single string "blank",
        let path_is_blank = self.url.path() == "blank";

        // its username and password are the empty string,
        let empty_username_and_password =
            self.url.username().is_empty() && self.url.password().is_none();

        // and its host is null.
        let null_host = self.url.host().is_none();

        scheme_is_about && path_is_blank && empty_username_and_password && null_host
    }

    pub fn as_non_blob_url(&self) -> Option<ServoUrl> {
        if self.token.is_some() {
            return None;
        }
        debug_assert_ne!(self.scheme(), "blob");

        Some(ServoUrl {
            url: self.url.clone(),
            token: None,
        })
    }

    pub fn parse_with_base(base: Option<&Url>, input: &str) -> Result<Self, ParseNonBlobUrlError> {
        Url::options()
            .base_url(base)
            .parse(input)
            .map_err(ParseNonBlobUrlError::Url)
            .and_then(|url| Self::from_non_blob_url(url).map_err(Into::into))
    }

    /// Convert this URL into one a more general type that might contain a `blob:`.
    ///
    /// The URL itself remains unchanged by this operation.
    pub fn with_blob_storage<T2: BlobToken>(self) -> Option<BlobStoreAgnosticServoUrl<T2>> {
        if self.scheme() == "blob" {
            return None;
        }

        Some(BlobStoreAgnosticServoUrl {
            url: self.url,
            token: None,
        })
    }

    pub fn unlock_blob(self) -> ServoUrl {
        ServoUrl {
            url: self.url,
            token: None,
        }
    }
}

impl<T: BlobToken> Deref for BlobStoreAgnosticServoUrl<T> {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.url
    }
}

impl<T: BlobToken> fmt::Display for BlobStoreAgnosticServoUrl<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.url.fmt(formatter)
    }
}

impl<T: BlobToken> fmt::Debug for BlobStoreAgnosticServoUrl<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let url_string = self.url.as_str();
        if self.scheme() != "data" || url_string.len() <= DATA_URL_DISPLAY_LENGTH {
            return url_string.fmt(formatter);
        }

        let mut hasher = DefaultHasher::new();
        hasher.write(self.url.as_str().as_bytes());

        format!(
            "{}... ({:x})",
            url_string
                .chars()
                .take(DATA_URL_DISPLAY_LENGTH)
                .collect::<String>(),
            hasher.finish()
        )
        .fmt(formatter)
    }
}

impl<T: BlobToken> Index<RangeFull> for BlobStoreAgnosticServoUrl<T> {
    type Output = str;
    fn index(&self, _: RangeFull) -> &str {
        &self.url[..]
    }
}

impl<T: BlobToken> Index<RangeFrom<Position>> for BlobStoreAgnosticServoUrl<T> {
    type Output = str;
    fn index(&self, range: RangeFrom<Position>) -> &str {
        &self.url[range]
    }
}

impl<T: BlobToken> Index<RangeTo<Position>> for BlobStoreAgnosticServoUrl<T> {
    type Output = str;
    fn index(&self, range: RangeTo<Position>) -> &str {
        &self.url[range]
    }
}

impl<T: BlobToken> Index<Range<Position>> for BlobStoreAgnosticServoUrl<T> {
    type Output = str;
    fn index(&self, range: Range<Position>) -> &str {
        &self.url[range]
    }
}

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, MallocSizeOf, Hash,
)]
pub struct DummyBlobToken;

impl BlobToken for DummyBlobToken {
    fn refresh(&self) -> Self {
        Self
    }
    fn neuter(&mut self) {}
}

/// A reference-counted URL type.
pub type ServoUrl = BlobStoreAgnosticServoUrl<DummyBlobToken>;
pub type ServoNonLockingUrl = ServoUrl;

#[derive(Clone, Copy, Debug)]
pub struct IsBlobUrlError;

impl From<Url> for ServoUrl {
    fn from(url: Url) -> Self {
        Self {
            url: Arc::new(url),
            token: None,
        }
    }
}

impl From<Arc<Url>> for ServoUrl {
    fn from(url: Arc<Url>) -> Self {
        Self { url, token: None }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ParseNonBlobUrlError {
    Url(url::ParseError),
    IsBlobUrl,
}

impl fmt::Display for ParseNonBlobUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(url_parse_error) => url_parse_error.fmt(f),
            Self::IsBlobUrl => "Unexpected blob url".fmt(f),
        }
    }
}

impl From<url::ParseError> for ParseNonBlobUrlError {
    fn from(value: url::ParseError) -> Self {
        Self::Url(value)
    }
}

impl From<IsBlobUrlError> for ParseNonBlobUrlError {
    fn from(_: IsBlobUrlError) -> Self {
        Self::IsBlobUrl
    }
}

impl FromStr for ServoUrl {
    type Err = ParseNonBlobUrlError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let url = Url::from_str(value)?;
        Self::from_non_blob_url(url).map_err(From::from)
    }
}

// Need manual trait impls due to https://github.com/rust-lang/rust/issues/26925
impl<T: BlobToken> Clone for BlobStoreAgnosticServoUrl<T> {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            token: self.token.clone(),
        }
    }
}

impl<T1: BlobToken, T2: BlobToken> PartialEq<BlobStoreAgnosticServoUrl<T2>>
    for BlobStoreAgnosticServoUrl<T1>
{
    fn eq(&self, other: &BlobStoreAgnosticServoUrl<T2>) -> bool {
        self.url.eq(&other.url)
    }
}

impl<T: BlobToken> Eq for BlobStoreAgnosticServoUrl<T> {}

impl<T: BlobToken> Hash for BlobStoreAgnosticServoUrl<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

impl<T1: BlobToken, T2: BlobToken> PartialOrd<BlobStoreAgnosticServoUrl<T2>>
    for BlobStoreAgnosticServoUrl<T1>
{
    fn partial_cmp(&self, other: &BlobStoreAgnosticServoUrl<T2>) -> Option<Ordering> {
        self.url.partial_cmp(&other.url)
    }
}

impl<T: BlobToken> Ord for BlobStoreAgnosticServoUrl<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.url.cmp(&other.url)
    }
}

impl<T: BlobToken> MallocSizeOf for BlobStoreAgnosticServoUrl<T> {
    fn size_of(&self, ops: &mut malloc_size_of::MallocSizeOfOps) -> usize {
        self.url.size_of(ops) + self.token.size_of(ops)
    }
}
