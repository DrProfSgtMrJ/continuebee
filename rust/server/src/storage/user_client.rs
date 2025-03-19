use axum::http::Uri;
use sessionless::{secp256k1::PublicKey, Sessionless};

use super::{Client, PubKeys, StorageClient, User};


static USER_STRING: &str = "user";
static KEYS_STRING: &str = "keys";

#[derive(Debug, Clone)]
pub struct UserCLient {
    pub client: Client
}

impl UserCLient {
    pub fn new(storage_uri: Uri) -> Self {
        Self { client: Client::new(storage_uri) }
    }

    fn key(uuid: &str) -> String {
        format!("{}:{}", USER_STRING, uuid)
    }

    pub async fn get_user_uuid(self, pub_key: &PublicKey) -> Option<String> {
        match self.get_keys().await {
            Ok(pub_keys) => pub_keys.get_user_uuid(&pub_key.to_string()).cloned(),
            Err(_) => None
        }
    }

    pub async fn get_user(self, uuid: impl AsRef<str>) -> Option<User> {
        match self.client.get(UserCLient::key(uuid.as_ref()).as_str()).await {
            Some(value) => {
                match serde_json::from_value(value) {
                    Ok(user) => Some(user),
                    Err(_) => None
                }
            },
            None => None
        }
    }

    // Will put a new user with the given pub_key and hash
    // will return the newly put user
    pub async fn put_user(&self, pub_key: &str, hash: &str) -> anyhow::Result<User> {
        let uuid = Sessionless::generate_uuid().to_string();
        let user = User::new(Some(uuid), pub_key.to_string(), hash.to_string());
        if let Ok(value) = serde_json::to_value(user.clone()) {
            match self.client.set(&UserCLient::key(&user.uuid).as_str(), value).await {
                Ok(_) => {
                    return Ok(user.clone());
                },
                Err(e) => Err(e.into()),
            }
        } else {
            Err(anyhow::Error::msg("Failed to serialize user"))
        }
    }


    // TODO
    /* pub async fn update_hash(self, existing_user: &User, new_hash: String) -> anyhow::Result<User> {
        if let Some(mut user) = self.clone().get_user(&existing_user.uuid).await {
            user.hash = new_hash;
            self.clone().put_user(&user).await
        } else {
            Err(anyhow::Error::msg("Failed to retrieve existing user"))
        }
    }*/

    pub async fn delete_user(self, uuid: &str) -> bool {
        self.client.delete(UserCLient::key(uuid).as_str()).await
    }

    pub async fn save_pub_keys(&self, keys: PubKeys) -> anyhow::Result<()> {
        if let Ok(value) = serde_json::to_value(keys) {
            self.client.set(KEYS_STRING, value).await?;
            Ok(())
        } else {
            Err(anyhow::Error::msg("Failed to set keys"))
        }
    }

    pub async fn get_keys(&self) -> anyhow::Result<PubKeys> {
        match self.client.get(KEYS_STRING).await {
            Some(value) => {
                match serde_json::from_value(value) {
                    Ok(result) => Ok(result),
                    Err(_) => Ok(PubKeys::default())
                }
            },
            None => Ok(PubKeys::default())
        }
    }

    // will add a new key
    pub async fn update_keys(&self, pub_key: &PublicKey, user_uuid: &str) -> anyhow::Result<()> {
        match self.get_keys().await {
            Ok(mut pub_keys) => {
                let pub_keys = pub_keys.add_user_uuid(user_uuid, &pub_key.to_string());
                self.save_pub_keys(pub_keys.clone()).await
            },
            Err(e) => Err(e)
        }

    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use axum::http::Uri;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_get_user() {
        let current_directory = std::env::current_dir().expect("Failed to get current directory"); 
        let dir_path = format!("{}/get_user", current_directory.display());
        let uri = Uri::builder().path_and_query(dir_path.clone()).build().unwrap();

        let initial_uuid = "uuid";
        let file_path = format!("{}/user:{}", dir_path, initial_uuid);
        let user_client = UserCLient::new(uri);

        match user_client.clone().client {
            Client::FileStorageClient { storage_client } => {
                storage_client.create_storage_dir().await.expect("Failed to create storage directory");
            },
            _ => assert!(false)
        }

        // confirm file doesn't exist before
        let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
        assert!(!file_exists);

        let user = User::new(Some(initial_uuid.to_string()), "pub_key".to_string(), "hash".to_string());

        let data= serde_json::to_value(user.clone()).expect("Failed to serialize");

        // write user to file with fs::write
        let mut file = match tokio::fs::File::create_new(file_path).await {
            Ok(file) => file,
            Err(e) => panic!("Failed to write to file: {}", e),
        };

        assert!(file.write_all(serde_json::to_string(&data).expect("Failed to serialize to string").as_bytes()).await.is_ok());

        match user_client.clone().get_user(initial_uuid).await {
            Some(result) => assert_eq!(result, user.clone()),
            None => assert!(false)
        };

        // clean up
        tokio::fs::remove_dir_all(dir_path.clone()).await.expect("Failed to remove directory");
    }


    #[tokio::test]
    async fn test_put_user() {
        let current_directory = std::env::current_dir().expect("Failed to get current directory"); 
        let dir_path = format!("{}/put_user", current_directory.display());
        let uri = Uri::builder().path_and_query(dir_path.clone()).build().unwrap();

        let user_client = UserCLient::new(uri);

        // check that dir_path doesn't exist
        let dir_exists = tokio::fs::metadata(dir_path.clone()).await.is_ok();
        assert!(!dir_exists);

        let pub_key = "pub_key";
        let hash = "hash";
        match user_client.put_user(pub_key, hash).await {
            Ok(result) => {
                // the set user should be a new uuid
                assert!(!result.uuid.is_empty());
                assert_eq!(result.pub_key.to_string(), pub_key.to_string());
                assert_eq!(result.hash, hash);
                let file_path = format!("{}/user:{}", dir_path.clone(), result.uuid);
                let file_exists = tokio::fs::metadata(file_path).await.is_ok();
                assert!(file_exists);
            },
            Err(_) => assert!(false)
        }

        // clean up
        tokio::fs::remove_dir_all(dir_path.clone()).await.expect("Failed to remove directory");
    }

    #[tokio::test]
    async fn test_delete_user() {
        let current_directory = std::env::current_dir().expect("Failed to get current directory"); 
        let dir_path = format!("{}/delete_user", current_directory.display());
        let uri = Uri::builder().path_and_query(dir_path.clone()).build().unwrap();

        let initial_uuid = "uuid";
        let file_path = format!("{}/user:{}", dir_path, initial_uuid);
        let user_client = UserCLient::new(uri);

        match user_client.clone().client {
            Client::FileStorageClient { storage_client } => {
                storage_client.create_storage_dir().await.expect("Failed to create storage directory");
            },
            _ => assert!(false)
        }

        // confirm the file doesn't exist before
        let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
        assert!(!file_exists);

        let user = User::new(Some(initial_uuid.to_string()), "pub_key".to_string(), "hash".to_string());
        let data = serde_json::to_value(user.clone()).expect("Failed to serialize");

        // write user to file with fs::write
        let mut file = match tokio::fs::File::create_new(file_path.clone()).await {
            Ok(file) => file,
            Err(e) => panic!("Failed to write to file: {}", e),
        };

        assert!(file.write_all(serde_json::to_string(&data).expect("Failed to serialize to string").as_bytes()).await.is_ok());

        // confirm the file exists
        let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
        assert!(file_exists);

        // delete the user: should be true as the file should be deleted
        assert!(user_client.clone().delete_user(initial_uuid).await);

        // confirm the file doesn't exist after
        let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
        assert!(!file_exists);

        // try to delete the user again: should be false as the file doesn't exist
        assert!(!user_client.clone().delete_user(initial_uuid).await);

        // clean up
        tokio::fs::remove_dir_all(dir_path.clone()).await.expect("Failed to remove directory");
    }

    #[tokio::test]
    async fn test_get_keys() {
        let current_directory = std::env::current_dir().expect("Failed to get current directory"); 
        let dir_path = format!("{}/get_keys", current_directory.display());
        let uri = Uri::builder().path_and_query(dir_path.clone()).build().unwrap();

        let file_path = format!("{}/{}", dir_path, KEYS_STRING);
        let user_client = UserCLient::new(uri);

        // confirm file doesn't exist before
        let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
        assert!(!file_exists);

        // Keys are default when the file doesn't exist
        match user_client.get_keys().await {
            Ok(result) => {
                assert_eq!(result, PubKeys::default());
            },
            Err(_) => assert!(false)
        }

        // create directory
        match user_client.clone().client {
            Client::FileStorageClient { storage_client } => {
                storage_client.create_storage_dir().await.expect("Failed to create storage directory");
            },
            _ => assert!(false)
        }

        let user_uuid = "test_user_uuid";
        let pub_key = "test_pub_key";

        let mut pub_keys = PubKeys::default();
        let pub_keys = pub_keys.add_user_uuid(user_uuid, pub_key);
        let data = serde_json::to_value(pub_keys.clone()).expect("Failed to serialize");

        // write pub_keys to file with fs::write
        let mut file = match tokio::fs::File::create_new(file_path).await {
            Ok(file) => file,
            Err(e) => panic!("Failed to write to file: {}", e),
        };

        assert!(file.write_all(serde_json::to_string(&data).expect("Failed to serialize to string").as_bytes()).await.is_ok());

        match user_client.clone().get_keys().await {
            Ok(result) => {
                let result_user_uuid = result.get_user_uuid(pub_key);
                assert!(result_user_uuid.is_some());
                assert_eq!(user_uuid, result_user_uuid.unwrap().as_str());
            },
            Err(_) => assert!(false)
        };

        // clean up
        tokio::fs::remove_dir_all(dir_path.clone()).await.expect("Failed to remove directory");
    }

    #[tokio::test]
    async fn test_save_pub_keys() {
        let current_directory = std::env::current_dir().expect("Failed to get current directory"); 
        let dir_path = format!("{}/save_pub_keys", current_directory.display());
        let uri = Uri::builder().path_and_query(dir_path.clone()).build().unwrap();

        let file_path = format!("{}/{}", dir_path, KEYS_STRING);
        let user_client = UserCLient::new(uri);

        // confirm file doesn't exist before
        let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
        assert!(!file_exists);

        // create directory
        match user_client.clone().client {
            Client::FileStorageClient { storage_client } => {
                storage_client.create_storage_dir().await.expect("Failed to create storage directory");
            },
            _ => assert!(false)
        }

        let mut pub_keys = PubKeys::default();
        let pub_keys = pub_keys.add_user_uuid("test_user_uuid", "test_pub_key");

        match user_client.clone().save_pub_keys(pub_keys.clone()).await {
            Ok(_) => {
                let file_exists = tokio::fs::metadata(file_path.clone()).await.is_ok();
                assert!(file_exists);
                // read the file and check the contents
                match tokio::fs::read(file_path.clone()).await {
                    Ok(data) => {
                        let result: PubKeys = serde_json::from_slice(data.as_slice()).expect("Failed to deserialize");
                        assert_eq!(result, *pub_keys);
                    },
                    Err(e) => panic!("Failed to read file: {}", e)
                }
            },
            Err(_) => assert!(false)
        }

        // clean up
        tokio::fs::remove_dir_all(dir_path.clone()).await.expect("Failed to remove directory");
    }
}