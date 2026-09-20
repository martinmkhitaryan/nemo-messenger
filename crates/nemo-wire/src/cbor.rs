//! Minimal canonical CBOR (RFC 8949 definite lengths) for Nemo maps.
//! Map keys are unsigned integers in increasing order (also encoded-byte order for small ints).

use crate::error::{Result, WireError};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Uint(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Value>),
    Map(Vec<(u64, Value)>),
}

pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_value(&mut out, value);
    out
}

fn write_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Uint(n) => write_head(out, 0, *n),
        Value::Bytes(b) => {
            write_head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(s) => {
            write_head(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::Array(items) => {
            write_head(out, 4, items.len() as u64);
            for v in items {
                write_value(out, v);
            }
        }
        Value::Map(pairs) => {
            let mut sorted = pairs.clone();
            sorted.sort_by_key(|a| a.0);
            write_head(out, 5, sorted.len() as u64);
            for (k, v) in sorted {
                write_head(out, 0, k);
                write_value(out, &v);
            }
        }
    }
}

fn write_head(out: &mut Vec<u8>, major: u8, n: u64) {
    if n < 24 {
        out.push((major << 5) | (n as u8));
    } else if n <= u8::MAX as u64 {
        out.push((major << 5) | 24);
        out.push(n as u8);
    } else if n <= u16::MAX as u64 {
        out.push((major << 5) | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= u32::MAX as u64 {
        out.push((major << 5) | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push((major << 5) | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

pub fn decode(bytes: &[u8]) -> Result<Value> {
    let (v, rest) = read_value(bytes)?;
    if !rest.is_empty() {
        return Err(WireError::Cbor("trailing bytes"));
    }
    Ok(v)
}

fn read_value(input: &[u8]) -> Result<(Value, &[u8])> {
    let (major, n, rest) = read_head(input)?;
    match major {
        0 => Ok((Value::Uint(n), rest)),
        2 => {
            let n = usize::try_from(n).map_err(|_| WireError::Cbor("bstr too large"))?;
            if rest.len() < n {
                return Err(WireError::Cbor("truncated bstr"));
            }
            Ok((Value::Bytes(rest[..n].to_vec()), &rest[n..]))
        }
        3 => {
            let n = usize::try_from(n).map_err(|_| WireError::Cbor("tstr too large"))?;
            if rest.len() < n {
                return Err(WireError::Cbor("truncated tstr"));
            }
            let s =
                std::str::from_utf8(&rest[..n]).map_err(|_| WireError::Cbor("tstr not utf-8"))?;
            Ok((Value::Text(s.to_owned()), &rest[n..]))
        }
        4 => {
            let count = usize::try_from(n).map_err(|_| WireError::Cbor("array too large"))?;
            let mut rest = rest;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                let (v, r) = read_value(rest)?;
                items.push(v);
                rest = r;
            }
            Ok((Value::Array(items), rest))
        }
        5 => {
            let count = usize::try_from(n).map_err(|_| WireError::Cbor("map too large"))?;
            let mut rest = rest;
            let mut pairs = Vec::with_capacity(count);
            let mut last_key: Option<u64> = None;
            for _ in 0..count {
                let (k, r) = read_value(rest)?;
                let Value::Uint(key) = k else {
                    return Err(WireError::Cbor("map key must be uint"));
                };
                if let Some(prev) = last_key {
                    if key <= prev {
                        return Err(WireError::Cbor("map keys not strictly increasing"));
                    }
                }
                last_key = Some(key);
                let (v, r) = read_value(r)?;
                pairs.push((key, v));
                rest = r;
            }
            Ok((Value::Map(pairs), rest))
        }
        _ => Err(WireError::Cbor("unsupported major type")),
    }
}

fn read_head(input: &[u8]) -> Result<(u8, u64, &[u8])> {
    let first = *input.first().ok_or(WireError::Cbor("empty"))?;
    let major = first >> 5;
    let ai = first & 0x1f;
    let rest = &input[1..];
    match ai {
        n @ 0..=23 => Ok((major, n as u64, rest)),
        24 => {
            let b = *rest.first().ok_or(WireError::Cbor("truncated u8"))?;
            Ok((major, b as u64, &rest[1..]))
        }
        25 => {
            if rest.len() < 2 {
                return Err(WireError::Cbor("truncated u16"));
            }
            let n = u16::from_be_bytes([rest[0], rest[1]]) as u64;
            Ok((major, n, &rest[2..]))
        }
        26 => {
            if rest.len() < 4 {
                return Err(WireError::Cbor("truncated u32"));
            }
            let n = u32::from_be_bytes(rest[..4].try_into().unwrap()) as u64;
            Ok((major, n, &rest[4..]))
        }
        27 => {
            if rest.len() < 8 {
                return Err(WireError::Cbor("truncated u64"));
            }
            let n = u64::from_be_bytes(rest[..8].try_into().unwrap());
            Ok((major, n, &rest[8..]))
        }
        _ => Err(WireError::Cbor("indefinite or reserved additional info")),
    }
}

pub fn map_get(map: &[(u64, Value)], key: u64) -> Result<&Value> {
    map_get_opt(map, key).ok_or(WireError::Cbor("missing map key"))
}

pub fn map_get_opt(map: &[(u64, Value)], key: u64) -> Option<&Value> {
    map.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

pub fn expect_bytes(v: &Value) -> Result<&[u8]> {
    match v {
        Value::Bytes(b) => Ok(b),
        _ => Err(WireError::Cbor("expected bstr")),
    }
}

pub fn expect_text(v: &Value) -> Result<&str> {
    match v {
        Value::Text(s) => Ok(s),
        _ => Err(WireError::Cbor("expected tstr")),
    }
}

pub fn expect_uint(v: &Value) -> Result<u64> {
    match v {
        Value::Uint(n) => Ok(*n),
        _ => Err(WireError::Cbor("expected uint")),
    }
}

pub fn expect_map(v: &Value) -> Result<&[(u64, Value)]> {
    match v {
        Value::Map(m) => Ok(m),
        _ => Err(WireError::Cbor("expected map")),
    }
}

pub fn expect_array(v: &Value) -> Result<&[Value]> {
    match v {
        Value::Array(a) => Ok(a),
        _ => Err(WireError::Cbor("expected array")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_map() {
        let v = Value::Map(vec![
            (0, Value::Uint(1)),
            (1, Value::Bytes(vec![1, 2, 3])),
            (2, Value::Text("ab".into())),
        ]);
        let bytes = encode(&v);
        assert_eq!(decode(&bytes).unwrap(), v);
        // definite map of 3, keys 0,1,2
        assert_eq!(bytes[0], (5 << 5) | 3);
    }

    #[test]
    fn roundtrip_array() {
        let v = Value::Array(vec![Value::Text("relay".into()), Value::Uint(1)]);
        let bytes = encode(&v);
        assert_eq!(decode(&bytes).unwrap(), v);
        assert_eq!(bytes[0], (4 << 5) | 2);
    }
}
