#![deny(missing_docs)]
#![doc(html_root_url = "http://alexcrichton.com/cookie-rs")]
#![cfg_attr(test, deny(warnings))]

//! HTTP Cookie parsing and Cookie Jar management
//!
//! Usage:
//!
//! add the following to the `[dependencies]` section of
//! your `Cargo.toml`:
//!
//! ```ignore
//! cookie = "0.4"
//! ```
//!
//! Then add the following line to your crate root:
//!
//! ```ignore
//! extern crate cookie;
//! ```

extern crate url;
extern crate time;
#[cfg(feature = "serialize-rustc")] extern crate rustc_serialize;
#[cfg(feature = "serialize-serde")] extern crate serde;

use std::ascii::AsciiExt;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "serialize-serde")] use serde::{Serialize, Deserialize};

pub use jar::CookieJar;
mod jar;

/// Holds all the data for a single cookie
#[derive(PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serialize-rustc", derive(RustcEncodable, RustcDecodable))]
pub struct Cookie {
    #[allow(missing_docs)]
    pub name: String,
    #[allow(missing_docs)]
    pub value: String,
    #[allow(missing_docs)]
    pub expires: Option<time::Tm>,
    #[allow(missing_docs)]
    pub max_age: Option<u64>,
    #[allow(missing_docs)]
    pub domain: Option<String>,
    #[allow(missing_docs)]
    pub path: Option<String>,
    #[allow(missing_docs)]
    pub secure: bool,
    #[allow(missing_docs)]
    pub httponly: bool,
    #[allow(missing_docs)]
    pub custom: BTreeMap<String, String>,
}

/// Crate-level error type used to indicate a problem with parsing
#[derive(Debug)]
pub struct Error;

impl Cookie {
    /// Creates a new `Cookie` instance from key and value strings
    ///
    /// # Example
    ///
    /// ```
    /// use cookie::Cookie;
    ///
    /// let c = Cookie::new("foo".into(), "bar".into());
    /// assert_eq!(c.name, "foo");
    /// assert_eq!(c.value, "bar");
    /// ```
    pub fn new(name: String, value: String) -> Cookie {
        Cookie {
            name: name,
            value: value,
            expires: None,
            max_age: None,
            domain: None,
            path: None,
            secure: false,
            httponly: false,
            custom: BTreeMap::new(),
        }
    }

    /// Attempts to parse a string into a `Cookie` instance
    ///
    /// # Example
    ///
    /// ```
    /// use cookie::Cookie;
    ///
    /// let c = Cookie::parse("foo=bar; httponly").expect("Failed to parse cookie");
    /// assert_eq!(c.name, "foo");
    /// assert_eq!(c.value, "bar");
    /// assert!(c.httponly);
    /// ```
    pub fn parse(s: &str) -> Result<Cookie, Error> {
        macro_rules! unwrap_or_skip{ ($e:expr) => (
            match $e { Some(s) => s, None => continue, }
        ) }

        let mut c = Cookie::new(String::new(), String::new());
        let mut pairs = s.trim().split(';');
        let keyval = match pairs.next() {
            Some(s) => s,
            _ => {
                return Err(Error);
            }
        };
        let (name, value) = try!(split(keyval));
        c.name = name.into();
        if c.name.is_empty() {
            return Err(Error);
        }
        c.value = value.into();

        for attr in pairs {
            let (k, v) = attr_split(attr);
            match (&k.to_ascii_lowercase()[..], v) {
                ("secure", _) => c.secure = true,
                ("httponly", _) => c.httponly = true,
                ("max-age", Some(v)) => {
                    // See RFC 6265 Section 5.2.2, negative values
                    // indicate that the earliest possible expiration
                    // time should be used, so set the max age as 0
                    // seconds.
                    let max_age: i64 = unwrap_or_skip!(v.parse().ok());
                    c.max_age = Some(if max_age < 0 {
                        0
                    } else {
                        max_age as u64
                    });
                },
                ("domain", Some(v)) => {
                    if v.is_empty() {
                        continue;
                    }

                    let domain = if v.chars().next() == Some('.') {
                        &v[1..]
                    } else {
                        v
                    };
                    c.domain = Some(domain.to_ascii_lowercase());
                }
                ("path", Some(v)) => c.path = Some(v.to_string()),
                ("expires", Some(v)) => {
                    // Try strptime with three date formats according to
                    // http://tools.ietf.org/html/rfc2616#section-3.3.1
                    // Try additional ones as encountered in the real world.
                    let tm = time::strptime(v, "%a, %d %b %Y %H:%M:%S %Z").or_else(|_| {
                        time::strptime(v, "%A, %d-%b-%y %H:%M:%S %Z")
                    }).or_else(|_| {
                        time::strptime(v, "%a, %d-%b-%Y %H:%M:%S %Z")
                    }).or_else(|_| {
                        time::strptime(v, "%a %b %d %H:%M:%S %Y")
                    });
                    let tm = unwrap_or_skip!(tm.ok());
                    c.expires = Some(tm);
                }
                (_, Some(v)) => {c.custom.insert(k.to_string(), v.to_string());}
                (_, _) => {}
            }
        }

        return Ok(c);

        fn attr_split<'a>(s: &'a str) -> (&'a str, Option<&'a str>) {
            match s.find("=") {
                Some(pos) => {
                    let parts = s.split_at(pos);
                    let value = parts.1[1..].trim();
                    (parts.0.trim(), Some(value))
                }
                None => (s.trim(), None)
            }
        }

        fn split<'a>(s: &'a str) -> Result<(&'a str, &'a str), Error> {
            macro_rules! try {
                ($e:expr) => (match $e {
                    Some(s) => s,
                    None => return Err(Error)
                })
            }
            let mut parts = s.trim().splitn(2, '=');
            let first = try!(parts.next()).trim();
            let second = try!(parts.next()).trim();
            Ok((first, second))
        }
    }

    /// Returns the (name, value) pair for this `Cookie` instance
    pub fn pair(&self) -> AttrVal {
        AttrVal(&self.name, &self.value)
    }
}

#[cfg(feature = "serialize-serde")]
impl Serialize for Cookie {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: serde::Serializer
    {
        serializer.serialize_str(&*self.to_string())
    }
}

#[cfg(feature = "serialize-serde")]
impl Deserialize for Cookie {
    fn deserialize<D>(deserializer: &mut D) -> Result<Cookie, D::Error>
        where D: serde::Deserializer
    {
        deserializer.deserialize_string(CookieVisitor)
    }
}
#[cfg(feature = "serialize-serde")]
struct CookieVisitor;

#[cfg(feature = "serialize-serde")]
impl serde::de::Visitor for CookieVisitor {
    type Value = Cookie;

    fn visit_str<E>(&mut self, v: &str) -> Result<Cookie, E>
        where E: serde::de::Error
    {
        match Cookie::parse(v) {
            Ok(cookie) => Ok(cookie),
            Err(_) => Err(serde::de::Error::custom("Could not parse serialized cookie!"))
        }
    }
}

/// Represents a key/value pair
pub struct AttrVal<'a>(pub &'a str, pub &'a str);

impl<'a> fmt::Display for AttrVal<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let AttrVal(ref attr, ref val) = *self;
        write!(f, "{}={}", attr, val)
    }
}

impl fmt::Display for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(AttrVal(&self.name, &self.value).fmt(f));
        if self.httponly { try!(write!(f, "; HttpOnly")); }
        if self.secure { try!(write!(f, "; Secure")); }
        match self.path {
            Some(ref s) => try!(write!(f, "; Path={}", s)),
            None => {}
        }
        match self.domain {
            Some(ref s) => try!(write!(f, "; Domain={}", s)),
            None => {}
        }
        match self.max_age {
            Some(n) => try!(write!(f, "; Max-Age={}", n)),
            None => {}
        }
        match self.expires {
            Some(ref t) => try!(write!(f, "; Expires={}", t.rfc822())),
            None => {}
        }

        for (k, v) in self.custom.iter() {
            try!(write!(f, "; {}", AttrVal(&k, &v)));
        }
        Ok(())
    }
}

impl FromStr for Cookie {
    type Err = Error;
    fn from_str(s: &str) -> Result<Cookie, Error> {
        Cookie::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::Cookie;

    #[test]
    fn parse() {
        assert!(Cookie::parse("bar").is_err());
        assert!(Cookie::parse("=bar").is_err());
        assert!(Cookie::parse(" =bar").is_err());
        assert!(Cookie::parse("foo=").is_ok());
        let mut expected = Cookie::new("foo".to_string(), "bar".to_string());
        assert_eq!(Cookie::parse("foo=bar").ok().unwrap(), expected);
        assert_eq!(Cookie::parse("foo = bar").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;Domain=").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;Domain= ").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;Ignored").ok().unwrap(), expected);
        expected.httponly = true;
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;httponly").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HTTPONLY=whatever").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ; sekure; HTTPONLY").ok().unwrap(), expected);
        expected.secure = true;
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure=aaaa").ok().unwrap(), expected);
        expected.max_age = Some(0);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=0").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age = 0 ").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=-1").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age = -1 ").ok().unwrap(), expected);
        expected.max_age = Some(4);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=4").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age = 4 ").ok().unwrap(), expected);
        expected.path = Some("/foo".to_string());
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=4; Path=/foo").ok().unwrap(), expected);
        expected.domain = Some("foo.com".to_string());
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=4; Path=/foo; \
                                  Domain=foo.com").ok().unwrap(), expected);
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=4; Path=/foo; \
                                  Domain=FOO.COM").ok().unwrap(), expected);
        expected.custom.insert("wut".to_string(), "lol".to_string());
        assert_eq!(Cookie::parse(" foo=bar ;HttpOnly; Secure; \
                                  Max-Age=4; Path=/foo; \
                                  Domain=foo.com; wut=lol").ok().unwrap(), expected);

        assert_eq!(expected.to_string(),
                   "foo=bar; HttpOnly; Secure; Path=/foo; Domain=foo.com; \
                    Max-Age=4; wut=lol");
    }

    #[test]
    fn cookie_parse_error() {
        match Cookie::parse("bar") {
            Ok(_) => assert!(false),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn odd_characters() {
        let expected = Cookie::new("foo".to_string(), "b%2Fr".to_string());
        assert_eq!(Cookie::parse("foo=b%2Fr").ok().unwrap(), expected);
    }

    #[test]
    fn pair() {
        let cookie = Cookie::new("foo".to_string(), "bar".to_string());
        assert_eq!(cookie.pair().to_string(), "foo=bar".to_string());
    }

    #[cfg(feature = "serialize-serde")]
    #[test]
    fn test_serialize() {
        #[cfg(feature = "serialize-serde")] extern crate serde_json;

        use super::Cookie;
        use time;
        use std::collections::BTreeMap;

        let mut custom = BTreeMap::new();
        custom.insert("x86".to_string(), "rdi".to_string());
        custom.insert("arm".to_string(), "x0".to_string());
        let original = Cookie {
            name: "Hello".to_owned(),
            value: "World!".to_owned(),
            expires: Some(time::strptime("Sun, 23 Nov 2014 20:00:00 UTC",
                                         "%a, %d %b %Y %H:%M:%S %Z").unwrap()),
            max_age: Some(42),
            domain: Some("servo.org".to_owned()),
            path: Some("/".to_owned()),
            secure: true,
            httponly: false,
            custom: custom
        };

        let serialized = serde_json::to_string(&original).unwrap();

        let roundtrip: Cookie = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, roundtrip);
    }

    #[cfg(feature = "serialize-serde")]
    #[test]
    fn test_serialize_odd_characters() {
        #[cfg(feature = "serialize-serde")] extern crate serde_json;

        use super::Cookie;
        use time;
        use std::collections::BTreeMap;

        let mut custom = BTreeMap::new();
        custom.insert("x86".to_string(), "rdi".to_string());
        custom.insert("arm".to_string(), "x0".to_string());
        let original = Cookie {
            name: "test".to_owned(),
            value: "^start/foo=bar\\s,name@place:[test]|hello%3Bworld".to_owned(),
            expires: Some(time::strptime("Tue, 15 Jun 2016 20:00:00 UTC",
                                         "%a, %d %b %Y %H:%M:%S %Z").unwrap()),
            max_age: Some(42),
            domain: Some("example.com".to_owned()),
            path: Some("/".to_owned()),
            secure: true,
            httponly: false,
            custom: custom
        };

        let serialized = serde_json::to_string(&original).unwrap();

        let roundtrip: Cookie = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, roundtrip);
    }
}
