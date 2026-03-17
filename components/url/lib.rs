/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![deny(unsafe_code)]
#![crate_name = "servo_url"]
#![crate_type = "rlib"]

pub mod encoding;
pub mod origin;

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::net::IpAddr;
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};
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

pub trait BlobStorage {
    type Token: BlobToken;

    fn acquire_blob_token(
        &self,
        url: &Url,
    ) -> Result<Option<TokenSerializationGuard<Self::Token>>, ()>;
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
pub struct BlobStoreAgnosticServoUrl<B: BlobStorage> {
    url: Arc<Url>,
    token: Option<TokenSerializationGuard<B::Token>>,
}

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

impl<B: BlobStorage> BlobStoreAgnosticServoUrl<B> {
    pub fn blob_token(&self) -> &Option<TokenSerializationGuard<B::Token>> {
        &self.token
    }

    pub fn from_url_with_token(url: Url, token: Option<TokenSerializationGuard<B::Token>>) -> Self {
        debug_assert!(token.is_some() || url.scheme() != "blob");
        Self {
            url: Arc::new(url),
            token,
        }
    }

    pub fn from_url_without_token(url: Url) -> Self {
        debug_assert_ne!(url.scheme(), "blob");
        Self {
            url: Arc::new(url),
            token: None,
        }
    }

    pub fn parse_with_base_and_blob_store(
        base: Option<&Self>,
        input: &str,
        blob_store: &B,
    ) -> Result<Self, url::ParseError> {
        let parsed = Url::options()
            .base_url(base.map(|b| &*b.url))
            .parse(input)?;
        let Ok(token) = blob_store.acquire_blob_token(&parsed) else {
            return Ok(Self::from_url_without_token(parsed));
        };
        Ok(Self::from_url_with_token(parsed, token))
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
    pub fn is_equal_excluding_fragments<Other: BlobStorage>(
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
            .map(Self::from_url_without_token)
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
}

impl<B: BlobStorage> fmt::Display for BlobStoreAgnosticServoUrl<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.url.fmt(formatter)
    }
}

impl<B: BlobStorage> fmt::Debug for BlobStoreAgnosticServoUrl<B> {
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

impl<B: BlobStorage> Index<RangeFull> for BlobStoreAgnosticServoUrl<B> {
    type Output = str;
    fn index(&self, _: RangeFull) -> &str {
        &self.url[..]
    }
}

impl<B: BlobStorage> Index<RangeFrom<Position>> for BlobStoreAgnosticServoUrl<B> {
    type Output = str;
    fn index(&self, range: RangeFrom<Position>) -> &str {
        &self.url[range]
    }
}

impl<B: BlobStorage> Index<RangeTo<Position>> for BlobStoreAgnosticServoUrl<B> {
    type Output = str;
    fn index(&self, range: RangeTo<Position>) -> &str {
        &self.url[range]
    }
}

impl<B: BlobStorage> Index<Range<Position>> for BlobStoreAgnosticServoUrl<B> {
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

pub struct DummyBlobStorage;

impl BlobStorage for DummyBlobStorage {
    type Token = DummyBlobToken;

    fn acquire_blob_token(
        &self,
        url: &Url,
    ) -> Result<Option<TokenSerializationGuard<Self::Token>>, ()> {
        assert_ne!(
            url.scheme(),
            "blob",
            "No blob store attached, cannot use blob url"
        );
        Err(())
    }
}

/// A reference-counted URL type.
///
/// This URL does not have blob store attached, so trying to use it for
/// `blob:` URLs will panic.
pub type ServoUrl = BlobStoreAgnosticServoUrl<DummyBlobStorage>;

impl ServoUrl {
    pub fn from_url(url: Url) -> Self {
        Self::from_url_without_token(url)
    }

    pub fn parse_with_base(base: Option<&Self>, input: &str) -> Result<Self, url::ParseError> {
        Url::options()
            .base_url(base.map(|b| &*b.url))
            .parse(input)
            .map(Self::from_url)
    }

    pub fn parse(input: &str) -> Result<Self, url::ParseError> {
        Url::parse(input).map(Self::from_url)
    }
}

impl From<Url> for ServoUrl {
    fn from(url: Url) -> Self {
        Self::from_url(url)
    }
}

impl FromStr for ServoUrl {
    type Err = <Url as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let url = Url::from_str(value)?;
        Ok(url.into())
    }
}

// Need manual trait impls due to https://github.com/rust-lang/rust/issues/26925
impl<B: BlobStorage> Clone for BlobStoreAgnosticServoUrl<B> {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            token: self.token.clone(),
        }
    }
}

impl<B: BlobStorage> PartialEq for BlobStoreAgnosticServoUrl<B> {
    fn eq(&self, other: &Self) -> bool {
        self.url.eq(&other.url)
    }
}

impl<B: BlobStorage> Eq for BlobStoreAgnosticServoUrl<B> {}

impl<B: BlobStorage> Hash for BlobStoreAgnosticServoUrl<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

impl<B: BlobStorage> PartialOrd for BlobStoreAgnosticServoUrl<B> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.url.partial_cmp(&other.url)
    }
}

impl<B: BlobStorage> Ord for BlobStoreAgnosticServoUrl<B> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.url.cmp(&other.url)
    }
}

impl<B: BlobStorage> MallocSizeOf for BlobStoreAgnosticServoUrl<B> {
    fn size_of(&self, ops: &mut malloc_size_of::MallocSizeOfOps) -> usize {
        self.url.size_of(ops) + self.token.size_of(ops)
    }
}
