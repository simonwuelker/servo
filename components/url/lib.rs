/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![deny(unsafe_code)]
#![crate_name = "servo_url"]
#![crate_type = "rlib"]

pub mod origin;

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;
use std::net::IpAddr;
use std::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};
use std::path::Path;
use std::str::FromStr;

use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};
use servo_arc::Arc;
pub use url::Host;
use url::{Position, Url};
use uuid::Uuid;

pub use crate::origin::{ImmutableOrigin, MutableOrigin, OpaqueOrigin};

const DATA_URL_DISPLAY_LENGTH: usize = 40;

#[derive(Debug)]
pub enum UrlError {
    SetUsername,
    SetIpHost,
    SetPassword,
    ToFilePath,
    FromFilePath,
}

#[derive(Clone, Deserialize, Eq, Hash, MallocSizeOf, PartialEq, Serialize)]
pub struct ServoUrl{
    #[conditional_malloc_size_of]
    url: Arc<Url>,

    /// <https://url.spec.whatwg.org/#concept-url-blob-entry>
    blob_url_entry: Option<Box<BlobUrlEntry>>,
}

/// Represents an intermediate state during URL parsing, where the url itself
/// is parsed but the data of `blob` urls is not obtained yet.
///
/// The only useful thing you can do with such an URL is to immediately call [resolving_blob_urls_with](ServoUrlWithPotentialUnresolvedBlobReference::resolving_blob_urls_with)
/// to get a usable [ServoUrl].
pub struct ServoUrlWithPotentialUnresolvedBlobReference {
    url: Arc<Url>,
}

impl ServoUrlWithPotentialUnresolvedBlobReference {
    pub fn parse_with_base(base: Option<&ServoUrl>, input: &str) -> Result<Self, url::ParseError> {
        Url::options()
            .base_url(base.map(|b| &*b.url))
            .parse(input)
            .map(Self::from)
    }

    fn blob_id_and_origin(&self) -> Result<(Uuid, String), String> {
        let Some(mut segments) = self.url.path_segments() else {
            return Err(format!(
                "Blob url without path segments: {:?}",
                self.url.to_string()
            ));
        };

        let Some(id) = segments
            .next()
            .map(Uuid::from_str)
            .transpose()
            .ok()
            .flatten()
        else {
            return Err(format!(
                "Blob url with zero segments: {:?}",
                self.url.to_string()
            ));
        };

        if segments.next().is_some() {
            return Err(format!(
                "Blob url more than one path segment: {:?}",
                self.url.to_string()
            ));
        }

        let origin = self.url.origin().ascii_serialization();

        Ok((id, origin))
    }

    pub fn resolving_blob_urls_with<F>(self, f: F) -> Result<ServoUrl, String>
    where
        F: FnOnce(Uuid, String) -> Option<BlobUrlEntry>,
    {
        let blob_url_entry = if self.url.scheme() == "blob" {
            self
            .blob_id_and_origin()
            .ok()
            .and_then(|(id, origin)| f(id, origin))
            .map(Box::new)
        } else {
            None
        };

        let resolved_url = ServoUrl {
            url: self.url,
            blob_url_entry,
        };
        Ok(resolved_url)
    }

    pub fn as_non_blob_url(self) -> Option<ServoUrl> {
        if self.url.scheme() == "blob" {
            None
        } else {
            let url = ServoUrl {
                url: self.url,
                blob_url_entry: None,
            };
            Some(url)
        }
    }
}

/// <https://w3c.github.io/FileAPI/#blob-url-entry>
///
/// `MediaSource` objects are not supported yet.
#[derive(Clone, Deserialize, Eq, Hash, MallocSizeOf, PartialEq, Serialize)]
pub struct BlobUrlEntry {
    pub mime_type: String,
    pub data: Vec<u8>,
    pub origin: ImmutableOrigin,
}

impl ServoUrl {
    /// Use this method when you need a [ServoUrl], but don't want to deal with blob urls
    pub fn from_non_blob_url(input: &str) -> Result<Option<Self>, url::ParseError> {
        let potentially_blob_url: ServoUrlWithPotentialUnresolvedBlobReference = input.parse()?;
        Ok(potentially_blob_url.as_non_blob_url())
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

    pub fn join(&self, input: &str) -> Result<ServoUrl, url::ParseError> {
        let new_url = self.url.join(input)?;

        let result = Self {
            url: Arc::new(new_url),
            blob_url_entry: None,
        };

        Ok(result)
    }

    pub fn path_segments(&self) -> Option<::std::str::Split<char>> {
        self.url.path_segments()
    }

    pub fn query(&self) -> Option<&str> {
        self.url.query()
    }

    pub fn from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, UrlError> {
        let url = Url::from_file_path(path)
            .map(ServoUrlWithPotentialUnresolvedBlobReference::from)
            .map_err(|_| UrlError::FromFilePath)?
            .as_non_blob_url()
            .expect("file:// URLs are not blobs");

        Ok(url)
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

    /// <https://url.spec.whatwg.org/#concept-url-blob-entry>
    pub fn blob_url_entry(&self) -> Option<&BlobUrlEntry> {
        self.blob_url_entry.as_deref()
    }
}

impl fmt::Display for ServoUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.url.fmt(formatter)
    }
}

impl fmt::Debug for ServoUrl {
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

impl Index<RangeFull> for ServoUrl {
    type Output = str;
    fn index(&self, _: RangeFull) -> &str {
        &self.url[..]
    }
}

impl Index<RangeFrom<Position>> for ServoUrl {
    type Output = str;
    fn index(&self, range: RangeFrom<Position>) -> &str {
        &self.url[range]
    }
}

impl Index<RangeTo<Position>> for ServoUrl {
    type Output = str;
    fn index(&self, range: RangeTo<Position>) -> &str {
        &self.url[range]
    }
}

impl Index<Range<Position>> for ServoUrl {
    type Output = str;
    fn index(&self, range: Range<Position>) -> &str {
        &self.url[range]
    }
}

impl From<Url> for ServoUrlWithPotentialUnresolvedBlobReference {
    fn from(url: Url) -> Self {
        Self { url: Arc::new(url) }
    }
}

impl From<Arc<Url>> for ServoUrlWithPotentialUnresolvedBlobReference {
    fn from(url: Arc<Url>) -> Self {
        Self { url }
    }
}

impl FromStr for ServoUrlWithPotentialUnresolvedBlobReference {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::parse(s).map(Self::from)
    }
}

impl PartialOrd for ServoUrl {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.url.partial_cmp(&other.url)
    }
}

impl Ord for ServoUrl {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.url.cmp(&other.url)
    }
}
