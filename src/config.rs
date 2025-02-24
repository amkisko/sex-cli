use anyhow::{Context, Result};
use base64::Engine;
use keyring::Entry;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::secretbox;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "sex-cli";
const KEYRING_USERNAME: &str = "project-encryption-key";
const PROJECT_KEY_LENGTH: usize = 32;
const APP_NAME: &str = "sex-cli";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct EncryptedProject {
    #[serde(with = "encrypted_data")]
    pub name: Vec<u8>,
    pub slug: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Organization {
    pub name: String,
    pub slug: String,
    #[serde(skip)]
    keyring: Option<Entry>,
    #[serde(default)]
    #[serde(with = "encrypted_projects")]
    pub(crate) projects: HashMap<String, EncryptedProject>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub organizations: HashMap<String, Organization>,
}

mod encrypted_data {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let b64 = BASE64.encode(data);
        serializer.serialize_str(&b64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b64: String = String::deserialize(deserializer)?;
        BASE64
            .decode(b64.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

mod encrypted_projects {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        projects: &HashMap<String, EncryptedProject>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::Serialize;
        projects.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<String, EncryptedProject>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::Deserialize;
        HashMap::deserialize(deserializer)
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_path()?;
        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))
    }

    pub fn add_organization(&mut self, name: String, slug: String) {
        self.organizations.insert(
            name.clone(),
            Organization {
                name,
                slug,
                keyring: None,
                projects: HashMap::new(),
            },
        );
    }

    pub fn get_organization(&self, name: &str) -> Option<&Organization> {
        self.organizations.get(name)
    }

    pub fn get_organization_mut(&mut self, name: &str) -> Option<&mut Organization> {
        self.organizations.get_mut(name)
    }

    fn get_project_key() -> Result<[u8; PROJECT_KEY_LENGTH]> {
        let keyring = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)?;

        match keyring.get_password() {
            Ok(key_str) => {
                let key_bytes = base64::engine::general_purpose::STANDARD
                    .decode(key_str)
                    .context("Failed to decode project key")?;
                let mut key = [0u8; PROJECT_KEY_LENGTH];
                key.copy_from_slice(&key_bytes);
                Ok(key)
            }
            Err(_) => {
                // Generate new key if not exists
                let mut key = [0u8; PROJECT_KEY_LENGTH];
                rand::thread_rng().fill_bytes(&mut key);
                let key_str = base64::engine::general_purpose::STANDARD.encode(key);
                keyring.set_password(&key_str)?;
                Ok(key)
            }
        }
    }

    #[allow(dead_code)]
    pub fn find_project(&self, project_slug: &str) -> Vec<(&Organization, bool)> {
        let mut matches = Vec::new();

        // First, check cached projects
        for org in self.organizations.values() {
            if org.projects.contains_key(project_slug) {
                matches.push((org, true)); // true indicates it's from cache
            }
        }

        // If no matches in cache, return all orgs for live check
        if matches.is_empty() {
            matches = self
                .organizations
                .values()
                .map(|org| (org, false)) // false indicates it needs live check
                .collect();
        }

        matches
    }

    pub fn cache_project(
        &mut self,
        org_name: &str,
        project_slug: String,
        project_name: String,
    ) -> Result<()> {
        if let Some(org) = self.organizations.get_mut(org_name) {
            let key = Self::get_project_key()?;
            let nonce = secretbox::gen_nonce();
            let encrypted_name =
                secretbox::seal(project_name.as_bytes(), &nonce, &secretbox::Key(key));

            let mut combined = nonce.as_ref().to_vec();
            combined.extend(encrypted_name);

            org.projects.insert(
                project_slug.clone(),
                EncryptedProject {
                    name: combined,
                    slug: project_slug,
                },
            );
            self.save()?;
        }
        Ok(())
    }
}

impl Organization {
    pub fn new(name: String, slug: String) -> Self {
        let keyring = Entry::new(&format!("{}-{}", APP_NAME, name), "auth-token").ok();
        Self {
            name,
            slug,
            keyring,
            projects: HashMap::new(),
        }
    }

    pub fn get_auth_token(&self) -> Result<Option<String>> {
        Ok(self.keyring.as_ref().and_then(|k| k.get_password().ok()))
    }

    pub fn set_auth_token(&mut self, token: String) -> Result<()> {
        if let Some(keyring) = &self.keyring {
            keyring.set_password(&token)?;
        }
        Ok(())
    }

    pub fn get_project(&self, slug: &str) -> Option<Result<String>> {
        self.projects.get(slug).map(|project| {
            let key = Config::get_project_key()?;
            let combined = &project.name;
            if combined.len() < secretbox::NONCEBYTES {
                return Err(anyhow::anyhow!("Invalid encrypted project data"));
            }

            let (nonce_bytes, encrypted) = combined.split_at(secretbox::NONCEBYTES);
            let nonce =
                secretbox::Nonce::from_slice(nonce_bytes).context("Invalid nonce length")?;

            let decrypted = secretbox::open(encrypted, &nonce, &secretbox::Key(key))
                .map_err(|_| anyhow::anyhow!("Failed to decrypt project name"))?;

            String::from_utf8(decrypted).context("Invalid UTF-8 in decrypted project name")
        })
    }

    pub fn has_project(&self, slug: &str) -> bool {
        self.projects.contains_key(slug)
    }

    #[allow(dead_code)]
    pub fn add_project(&mut self, project_slug: String) {
        self.projects.insert(
            project_slug.clone(),
            EncryptedProject {
                name: Vec::new(),
                slug: project_slug,
            },
        );
    }
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to determine config directory")?
        .join("sex-cli");
    Ok(config_dir.join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;

    #[test]
    fn test_add_organization() {
        let mut config = Config::default();
        config.add_organization("test".to_string(), "test-slug".to_string());

        let org = config.get_organization("test").unwrap();
        assert_eq!(org.name, "test");
        assert_eq!(org.slug, "test-slug");
        assert!(org.keyring.is_none());
    }

    #[test]
    fn test_organization_auth_token() -> Result<()> {
        let mut config = Config::default();
        config.add_organization("test".to_string(), "test-slug".to_string());

        let org = config.get_organization_mut("test").unwrap();
        org.set_auth_token("secret-token".to_string())?;

        let token = org.get_auth_token()?.unwrap();
        assert_eq!(token, "secret-token");
        Ok(())
    }

    #[test]
    fn test_save_and_load() -> Result<()> {
        let temp = assert_fs::TempDir::new()?;
        let config_file = temp.child("config.json");

        let mut config = Config::default();
        config.add_organization("test".to_string(), "test-slug".to_string());

        // Save config
        let content = serde_json::to_string_pretty(&config)?;
        config_file.write_str(&content)?;

        // Load config
        let loaded: Config = serde_json::from_str(&fs::read_to_string(config_file.path())?)?;
        assert_eq!(config, loaded);

        Ok(())
    }

    #[test]
    fn test_load_nonexistent() -> Result<()> {
        let temp = assert_fs::TempDir::new()?;
        let config_file = temp.child("config.json");

        assert!(!config_file.exists());
        let config = Config::default();
        assert_eq!(config.organizations.len(), 0);

        Ok(())
    }
}
