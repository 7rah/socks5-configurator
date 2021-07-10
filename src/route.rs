use crate::common::Address;
use anyhow::Result;
use flit::BloomFilter;
use ipnet::{Ipv4Net, Ipv6Net};
use iprange::IpRange;
use serde_derive::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

pub enum Tag {
    Proxy,
    Direct,
}

impl Tag {
    // true -> Direct
    // false -> Proxy
    fn from_bool(b: bool) -> Self {
        match b {
            true => Tag::Direct,
            false => Tag::Proxy,
        }
    }
}

pub struct Router {
    ipv4_filter: IpRange<Ipv4Net>,
    ipv6_filter: IpRange<Ipv6Net>,
    domain_filter: BloomFilter<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Source {
    Url { url: String },
    Path { path: String },
}

#[derive(Debug, Deserialize)]
pub struct RouterConfig {
    pub cidr4: Source,
    pub cidr6: Source,
    pub domain: Source,
}

impl Source {
    fn lines(&self) -> Result<Vec<String>> {
        let mut result: Vec<String> = Vec::new();
        match self {
            Source::Url { url } => {
                let reader = ureq::get(url).call()?.into_reader();
                let reader = BufReader::new(reader);
                for line in reader.lines() {
                    let line = line?;
                    result.push(line);
                }
            }
            Source::Path { path } => {
                let file = File::open(path)?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = line?;
                    result.push(line);
                }
            }
        }
        Ok(result)
    }
}

impl Router {
    pub fn init(config: &RouterConfig) -> Result<Self> {
        let ipv4_filter: IpRange<Ipv4Net> = config
            .cidr4
            .lines()?
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();

        let ipv6_filter: IpRange<Ipv6Net> = config
            .cidr6
            .lines()?
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();

        let domains = config.domain.lines()?;
        let mut domain_filter = BloomFilter::new(0.01, domains.len());
        for domain in domains {
            domain_filter.add(&domain);
        }

        Ok(Router {
            ipv4_filter,
            ipv6_filter,
            domain_filter,
        })
    }

    #[inline]
    pub fn match_tag(&self, addr: &Address) -> Tag {
        match addr {
            Address::Domain(domain, _) => {
                let len = domain.len();
                for (i, &item) in domain.as_bytes().iter().enumerate() {
                    if item == b'.' {
                        let str = &domain[i + 1..len];
                        if self.domain_filter.might_contain(&str.to_string()) {
                            return Tag::from_bool(true);
                        }
                    }
                }
                Tag::from_bool(false)
            }
            Address::SocketAddr(addr) => match addr.ip() {
                std::net::IpAddr::V4(ipv4) => Tag::from_bool(self.ipv4_filter.contains(&ipv4)),
                std::net::IpAddr::V6(ipv6) => Tag::from_bool(self.ipv6_filter.contains(&ipv6)),
            },
        }
    }
}
