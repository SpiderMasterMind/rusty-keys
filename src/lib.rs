use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_jsonlines::{append_json_lines, json_lines};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{
    env,
    fs::{self},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

struct Config {
    wal_path: PathBuf,
}

fn config() -> Config {
    Config {
        wal_path: default_wal_path(),
    }
}

fn default_wal_path() -> PathBuf {
    let mut path = env::current_dir().expect("cannot get current dir");
    path.push("wal.log");
    path
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    crc: String,
    timestamp: String,
    keysize: String,
    valuesize: String,
    key: String,
    value: String,
}

impl Entry {
    pub fn new(key: String, value: String) -> Self {
        Self {
            key: key,
            value: value,
            crc: String::from("CRC"),
            keysize: String::from("1"),
            valuesize: String::from("1"),
            timestamp: String::from("123"),
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("key not found: {0}")]
    NotFound(String),

    #[error("failed to read WAL: {0}")]
    WalReadFailed(#[from] anyhow::Error),
}

pub fn add<P: AsRef<Path>>(
    key: String,
    value: String,
    wal_path: Option<P>,
) -> Result<(String, String)> {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("time backwards");

    let key_size = String::len(&key);
    let value_size = String::len(&value);

    let values = vec![Entry {
        crc: "CRC".to_string(),
        timestamp: duration.as_millis().to_string(),
        keysize: key_size.to_string(),
        valuesize: value_size.to_string(),
        key: key.clone(),
        value: value.clone(),
    }];

    let path = match wal_path {
        Some(p) => p.as_ref().to_path_buf(),
        None => config().wal_path,
    };

    append_json_lines(&path, &values)?;
    Ok((key, value))
}

pub fn replay_into_memory<P: AsRef<Path>>(wal_path: Option<P>) -> Result<Vec<Entry>> {
    let path = match wal_path {
        Some(p) => p.as_ref().to_path_buf(),
        None => config().wal_path,
    };
    let entries = json_lines(&path)?
        .map(|r| r.map_err(anyhow::Error::msg))
        .collect::<Result<Vec<Entry>>>()?;
    Ok(entries)
}

pub fn read_from_memory<P: AsRef<Path>>(
    key: String,
    wal_path: Option<P>,
) -> Result<(String, String), StoreError> {
    let entries = replay_into_memory(wal_path)?;
    let mut result_hash = HashMap::new();

    for entry in entries {
        result_hash.insert(entry.key, entry.value);
    }

    let value = result_hash
        .get(&key)
        .cloned()
        .ok_or_else(|| StoreError::NotFound(key.clone()))?;

    Ok((key, value))
}

pub fn all<P: AsRef<Path>>(wal_path: Option<P>) -> Result<String> {
    if let Some(path) = wal_path {
        Ok(fs::read_to_string(path)?)
    } else {
        Ok(fs::read_to_string(Path::new(&config().wal_path))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches_regex::assert_matches_regex;
    use std::fs::File;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::tempdir;

    #[test]
    fn all_returns_kvs_in_wal_log() -> () {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("temp_wal.log");
        let mut file = File::create(&file_path).unwrap();

        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).expect("time backwards");

        let key = "a";
        let key_size = size_of_val(key);
        let value_string = "Hello World!";
        let value_size = size_of_val(value_string);

        let contents = format!(
            "{},{:?},{},{},{},{}",
            "CRC".to_string(),
            duration.as_millis(),
            key_size.to_string(),
            value_size.to_string(),
            key,
            value_string
        );
        let contents2 = format!(
            "{},{:?},{},{},{},{}",
            "CRC".to_string(),
            duration.as_millis(),
            key_size.to_string(),
            value_size.to_string(),
            key,
            value_string
        );
        writeln!(file, "{}\n{}", contents, contents2).unwrap();

        assert_matches_regex!(
            all(Some(file_path)).unwrap(),
            r"CRC,\d+,1,12,a,Hello World!"
        )
    }

    #[test]
    fn add_adds_and_returns_kv_added() -> () {
        let tmp_dir = tempdir().unwrap();
        let file_path = tmp_dir.path().join("my-temporary-note.txt");
        File::create(&file_path).unwrap();

        let add_result = add("A".to_string(), "Egg".to_string(), Some(&file_path)).unwrap();

        assert_eq!(add_result, (String::from("A"), String::from("Egg")))
    }

    #[test]
    fn read_from_memory_returns_the_keyed_value_when_it_exists() -> () {
        let tmp_dir = tempdir().unwrap();
        let file_path = tmp_dir.path().join("my-temporary-note.txt");
        File::create(&file_path).unwrap();

        add("A".into(), "Egg".into(), Some(&file_path)).unwrap();
        add("B".into(), "Man".into(), Some(&file_path)).unwrap();

        let result = read_from_memory("B".to_string(), Some(&file_path));

        assert_eq!(
            result.unwrap(),
            (String::from("B"), String::from("Man")),
            "incorrect value"
        )
    }

    #[test]
    fn read_from_memory_returns_an_err_when_it_doesnt_exist() -> () {
        let tmp_dir = tempdir().unwrap();
        let file_path = tmp_dir.path().join("my-temporary-note.txt");
        File::create(&file_path).unwrap();

        add("A".into(), "Egg".into(), Some(&file_path)).unwrap();

        let result = read_from_memory("B".to_string(), Some(&file_path));

        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    #[test]
    fn replay_into_memory_collects_jsonl_entries_into_entry_vec() -> () {
        let tmp_dir = tempdir().unwrap();
        let file_path = tmp_dir.path().join("my-temporary-note.txt");
        File::create(&file_path).unwrap();

        add("A".into(), "Egg".into(), Some(&file_path)).unwrap();
        add("B".into(), "Man".into(), Some(&file_path)).unwrap();
        add("C".into(), "Walrus".into(), Some(&file_path)).unwrap();

        let result = replay_into_memory(Some(&file_path)).unwrap();

        let _expected: Vec<Entry> = vec![("A", "Egg"), ("B", "Man")]
            .iter()
            .cloned()
            .map(|(k, v)| Entry::new(k.to_string(), v.to_string()))
            .collect();

        assert_eq!(result.len(), 3);

        let Entry { key, value, .. } = &result[2];
        assert_eq!(key, "C");
        assert_eq!(value, "Walrus");

        let Entry { key, value, .. } = &result[1];
        assert_eq!(key, "B");
        assert_eq!(value, "Man");

        let Entry { key, value, .. } = &result[0];
        assert_eq!(key, "A");
        assert_eq!(value, "Egg");
    }
}

// crc,32bittimestamp,keysize,valuesize,key,value
